# Specification: NPC spawning (static NPCs and warps)

Puts rAthena's positioned NPCs on our maps as visible, named, inert units: `warp`/`warp2`, `script`,
`duplicate(...)` and the shop family. They have no behavior: no click, no dialog, no OnTouch, no
shop catalog. The next feature is warp map change, so warps carry their destination and trigger
area now.

Ground truth is rAthena commit `d4b8e7b` at `PACKETVER = 20260219`, as recorded in the catalog:
- `reference/catalog/db/npc_files.md`: manifest, lexing, header grammar, names, sprites, counts
- `reference/catalog/packets/map_units.md#npc-units-in-entry-packets-static-scriptshopwarp-npcs`: `0x09ff` field values
- `reference/catalog/packets/map_units.md#0x0adf-zc_ack_reqnameall_npc-reply-to-cz_reqname-for-an-npc`: name reply
- `reference/catalog/mechanics/npcs.md`: which packet when, never-sent rules, ids, vanish, warp trigger

When this spec and the catalog disagree, the catalog wins and this spec gets fixed.

---

## 0. Data pipeline

```text
$RATHENA_PATH/npc/re/scripts_main.conf ──► tools/npc-import (offline, MIT, clean-room)
                                              │ writes
                                              ▼
                              crates/ro-db/data/npc/<map>.yml  (GPL-3.0, committed)
                                              │ ro_db::MapNpcs::load_yaml (serde, runtime)
                                              ▼
                         bins/map-server MapDataLoader ──► ro_engine::MapInstance::spawn_npcs
```

- `ro-db` never parses rAthena `.txt`. All of that grammar lives in `tools/npc-import`.
- Map names are bare rAthena names (`prontera`) in YAML and in `ro-db`. The map-server strips `.gat` before lookup.
- Script bodies are not converted. Each entry records `Source: <path>:<line>` so the scripting phase can find its body.

## 1. YAML schema (`NPC_SPAWN_DB`, version 1)

```yaml
# SPDX-License-Identifier: GPL-3.0-only
# Derived from rAthena (https://github.com/rathena/rathena) commit d4b8e7b, files under npc/.
# Modified: converted from tab-delimited .txt to YAML by tools/npc-import. Do not edit by hand.
Header:
  Type: NPC_SPAWN_DB
  Version: 1
Body:
  - Name: "Kafra Employee#prt1"   # display part (before "::"), as rAthena's nd->name
    UniqueName: kafra_prt1        # only when the header used "::"; otherwise equal to Name
    X: 152
    Y: 326
    Dir: 4                        # raw value, not validated (data has 8, 15, 55)
    Sprite: 117                   # i16; -1 = fake (never sent); 32767 = invisible (never sent)
    SpriteIsMob: true             # omitted when false; sprite resolves to a mob id (object_type 0x0C)
    State: Hidden                 # omitted when normal; Hidden | Cloaked | Disabled
    Trigger: { X: 1, Y: 1 }       # half-extents; warps always, scripts/duplicates when given
    DuplicateOf: "Kafra Employee" # unique name of the source, when declared via duplicate(...)
    Kind: Script                  # Script | Shop | Warp
    Warp: { Map: prt_fild08, X: 170, Y: 375, TriggerWhileHidden: false }  # Kind: Warp only
    Source: npc/re/merchants/kafra.txt:42
```

- `Kind` is the resolved kind. A duplicate of a warp is a `Warp` that takes its destination and `warp2` flag from the source and its trigger from its own line. A duplicate of a shop is a `Shop`.
- Every warp's sprite is `45` (`WARPNPC`).
- Entries in a file are in rAthena load order (manifest file order, then line). Output must be byte-identical across runs.

## 2. Scenarios

### DB-NPC-01: Load a map file
A valid file loads into `MapNpcs`. Every field round-trips: `MapNpcs` → YAML → `MapNpcs` is equal.

### DB-NPC-02: Reject bad files
A wrong `Header.Type`, an unsupported `Version`, a missing required field, `Kind: Warp` without `Warp`,
or a YAML syntax error each give an `NpcDataError` naming the path. YAML errors also give the line.
A missing file is `Ok(None)`, meaning the map has no NPCs.

### DB-NPC-03: Shipped data loads
Every `crates/ro-db/data/npc/*.yml` loads without error and starts with the SPDX + attribution header.
`LICENSE` and `NOTICE.md` exist in that directory.

### TOOL-NPC-01: Manifest
Follows `npc:`, `delnpc:` and `import:` (case-insensitive), plus `npc: clear` / `delnpc: all`, and strips `//` comments.
- `npc:` dedupes by exact path string.
- A `delnpc:` only removes an entry already present.
- `import:` recurses in place.
- Load order is list order.

### TOOL-NPC-02: Lexing
- Skips whitespace and `//` / `/* */` comments between records; block comments may span lines.
- A header line splits on tab into exactly 4 fields: the 4th takes the rest of the line.
- A `{...}` body is skipped by brace counting that ignores braces inside `"..."` (with `\` escapes) and inside comments. Single quotes are not special.
- A record may follow a body's `}` on the same line.
- CRLF works. A non-UTF-8 header decodes as CP949 (6 headers in `quests_17_2.txt` and `enchan_sage_legacy_17_2.txt`).

### TOOL-NPC-03: Headers
- `warp` / `warp2`: `map,x,y[,dir]` and `xs,ys,dest,dx,dy`.
- `script`: `sprite,{` or `sprite,xs,ys,{`.
- `duplicate(src)`: `sprite[,xs,ys]`.
- The shop family (`shop`, `cashshop`, `itemshop`, `pointshop`, `marketshop`): first token of `w4` is the sprite.
- Optional state suffix `(HIDDEN|CLOAKED|DISABLED)` on `script`, `warp` and `warp2`.
- Extra trailing `w1` values are ignored (`1@mcd,144,212,5,0`). Leading spaces before numbers are allowed (`rgsr_in,135,186, 7`).
- Floating (`-`) entries, `function`, `monster`, `boss_monster`, `mapflag` and unknown keywords are skipped and counted in the skip report.

### TOOL-NPC-04: Names
- `A::B` → `Name: A`, `UniqueName: B`. Otherwise both are the full string, with `#` suffix.
- Names are cut to 50 characters, with a warning.
- An empty or colliding unique name gets rAthena's generated form, `<n>_<map>_<x>_<y>`.

### TOOL-NPC-05: Sprites
The first token of `w4`, ending at whitespace, `/` or `,`, resolves in this order:
1. A number in C `strtol` base 0 (`0x` hex, leading-`0` octal; an empty token is 0).
2. A `JT_` constant from `src/map/npc.hpp` `e_job_types`, case-insensitive with the prefix stripped.
3. A mob AegisName from `db/re/mob_db.yml`, which also sets `SpriteIsMob`.
4. Otherwise `32767`, with a warning.

A numeric sprite that is a `mob_db` id also sets `SpriteIsMob`.

### TOOL-NPC-06: Duplicates
`duplicate(src)` resolves `src` by unique name among NPCs already defined, case-sensitively and across files. Duplicates of duplicates work. A source that hasn't been defined yet is a skip-report error.

### TOOL-NPC-07: Output and report
- Writes one file per map with the header from §1.
- Prints counts per kind and per skip reason. Against `d4b8e7b` they must match `npc_files.md` §Counts: 3876 warp, 229 warp2, 8635 script, 10377 duplicate, 270 shop.

### PKT-NPC-01: NPC in `0x09ff`
`UnitAppearance` for an NPC encodes the field values in the catalog table:
- `object_type` `0x06` (`0x0C` if `SpriteIsMob`)
- `aid` = NPC id, `GID` 0
- speed 200
- `effect_state` 0 / `0x2` Hidden / `0x4` Cloaked
- `job` = sprite; head, palettes, sex, `clevel` all 0
- `x_size`/`y_size` 0
- hp -1/-1
- name cut to 23 bytes + NUL

A player's bytes don't change: `object_type` 0, sizes 5.

### PKT-NPC-02: `0x0adf` `ZC_ACK_REQNAMEALL_NPC`
Fixed 58 bytes: `gid`, `group_id` 0, `name`[24], `title`[24] empty. `table.rs` has `0x0adf => Fixed(58)`.

### ENG-NPC-01: Spawn
`MapInstance::spawn_npcs(&MapNpcs)` creates one entity per entry.
- Ids are a per-instance counter from `START_NPC_NUM = 110000000`. rAthena uses one global pool; per-instance is fine because a client only sees one map.
- An entry outside the grid is skipped with a warning.
- An NPC with sprite `-1` or `32767`, or `State: Disabled`, exists but is never in the spatial grid, so it is never sent.
- Sprites `45`, `111` and `139` are sent normally.
- Spawning emits no events (rAthena: no users on the map yet).

### ENG-NPC-02: Seen on entry
A player's `NotifyActorInit` sends them a `UnitStanding` for every visible NPC within `AREA_SIZE`.

### ENG-NPC-03: Seen while walking
A player who walks so an NPC enters view gets `UnitStanding` for it. When it leaves view, the player gets `UnitVanished { type 0 }`. The NPC gets nothing.

### ENG-NPC-04: Name request
`CZ_REQNAME` on a visible NPC within `AREA_SIZE` replies to the requester only with the NPC name event (`0x0adf`). Out of range, or an unknown gid, gets no reply.

### ENG-NPC-05: Warp data kept
A `Kind: Warp` entity carries `WarpTrigger { half_extents, dest_map, dest_x, dest_y, trigger_while_hidden }`, readable through a crate-internal accessor. Nothing uses it yet; map change will.

### BIN-NPC-01: Loaded per map
- With `NPC_DATA_DIR` (`--npc-data-dir`) set, a world that starts for `<map>.gat` loads `<dir>/<map>.yml`, and a player entering sees its NPCs.
- A missing file means no NPCs.
- A bad file logs a warning and means no NPCs.
- An unset `NPC_DATA_DIR` logs one warning at startup and means no NPCs.

## 3. Licensing
The repo is mixed-license. Code is MIT. `crates/ro-db/data/npc/` is a modified copy of rAthena data and stays GPL-3.0. That directory holds:
- the verbatim rAthena `LICENSE`;
- `NOTICE.md`, giving the origin, commit, copyright and modification with its date.

Other licensing changes:
- `crates/ro-db/Cargo.toml` excludes `data/` from packaging.
- The root `README.md` gains a License section and Acknowledgements for rAthena, rust-ro (MIT) and rathena-script-lang-interpreter. The interpreter has no license, so it is read-only reference.
- The root `LICENSE` is MIT, © Reynald Pader.
- SPDX tag `GPL-3.0-only`: rAthena ships the GPLv3 text without "or later" in its source headers, and `-only` is always a valid choice for a redistributor.

## 4. Deferred
- Portals that are `script` NPCs (about 970 using sprites 45/139 with OnTouch `warp`) stay inert until scripting.
- Floating NPCs, `function`, mob spawns, mapflags, shop catalogs, and the barter NPCs in `npc/**/barters.yml`.
- PC-job-sprite NPC extras (`body = class`, hair clamp) and the guild flag.
- Wire name encoding: names are UTF-8 on the wire. The client expects CP949, so the 6 Korean-suffixed NPC names render wrong.
- The client's behavior for `Dir` values above 7.
- A real-client capture of NPC entry and name packets. Everything here is derived from source.
