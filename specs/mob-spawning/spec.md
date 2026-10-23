# Specification: Mob spawning (permanent spawns, placeholder stats)

Puts rAthena's permanent mob spawns (`monster` / `boss_monster` lines) on our maps as visible, named,
inert units. They exist so combat has something to hit. They have placeholder stats (1 HP), no
movement, no AI, no death, no respawn, and no mob_db stats. Combat comes next.

Ground truth is rAthena commit `d4b8e7b` at `PACKETVER = 20260219`, as recorded in the catalog:
- `reference/catalog/mechanics/mobs.md` §1.1 (spawn line), §1.4 (placement), §1.5 (wire fields),
  §1.6 (importer facts and counts), §1.7 (movement/AI notes, not used here)
- `reference/catalog/db/npc_files.md` §1-§3: manifest and lexing, shared with NPCs
- `reference/catalog/db/mob_db.md`: id ranges, `Name` / `JapaneseName`

When this spec and the catalog disagree, the catalog wins and this spec gets fixed.

---

## 0. Data pipeline

```text
$RATHENA_PATH/npc/re/scripts_main.conf ──► tools/npc-import (offline, MIT, clean-room)
                                              │ writes
                        ┌─────────────────────┴──────────────────────┐
                        ▼                                            ▼
   crates/ro-db/data/npc/<map>.yml (unchanged)    crates/ro-db/data/mob_spawn/<map>.yml (GPL-3.0)
                                                                     │ ro_db::MapMobSpawns::load_yaml
                                                                     ▼
                              bins/map-server MapDataLoader ──► ro_engine::MapInstance::spawn_mobs
```

- The same importer handles both datasets. The manifest that lists NPC files also lists the mob spawn
  files, and the lexer already yields `monster` lines as records.
- `ro-db` never parses rAthena `.txt`.
- The YAML keeps the spawn line's values. Rules that depend on map cells or on config (the rectangle
  rules, map-wide forcing, respawn delays) run at runtime.
- rAthena's `dynamic_mobs: yes` creates a map's mobs when its first player enters. Our map worlds
  already start on first entry, so spawning at world start matches.

## 1. YAML schema (`MOB_SPAWN_DB`, version 1)

```yaml
# SPDX-License-Identifier: GPL-3.0-only
# Derived from rAthena (https://github.com/rathena/rathena) commit d4b8e7b..., files under npc/.
# Modified: converted from tab-delimited .txt to YAML by tools/npc-import. Do not edit by hand.
# See crates/ro-db/data/LICENSE and crates/ro-db/data/NOTICE.md.
Header:
  Type: MOB_SPAWN_DB
  Version: 1
Body:
  - Name: Poring                # spawn-line name (<= 23 chars); --en--/--ja-- already resolved
    MobId: 1002                 # AegisName refs already resolved
    X: 0
    Y: 0
    Span: { X: 10, Y: 10 }      # omitted when both are 0; the raw xs, ys
    Amount: 20                  # 1..=1000
    Delay1: 5000                # ms; 5000 when the line has none
    Delay2: 180000              # ms; omitted when 0
    Level: 5                    # omitted when the line has none (no current data has one)
    Boss: true                  # omitted when false; the line used boss_monster
    Event: "ant_d02_i_boss::OnMobDead"   # omitted when absent or ignored (see TOOL-MOB-04)
    Source: npc/re/mobs/fields/prontera.txt:12
```

- `Boss` is the spawn keyword. It is not mob_db's `Class: Boss` or `Mvp`.
- Entries in a file are in rAthena load order (manifest file order, then line). Output is
  byte-identical across runs.
- `MobId` must be in rAthena's valid ranges: `1001..=3998` or `20021..=31998` (`1000 < id < 3999` or
  `20020 < id < 31999`).

## 2. Scenarios

### TOOL-MOB-01: Spawn lines are converted
- `monster` and `boss_monster` (case-insensitive) become spawn entries instead of skip-report entries.
- Each field is trimmed, so ` 20255` (with a leading space) is valid.
- A trailing `\t//comment` after w4 is ignored.
- Missing optional fields take the defaults: `Delay1` 5000, `Delay2` 0, no level, no event.

### TOOL-MOB-02: Location
- `w1` is `map`, `map,x,y` or `map,x,y,xs,ys`. Missing numbers are 0, so a bare map name is map-wide.
- A negative `x` or `y` is a skip (`negative coordinates`).
- A negative `xs`/`ys` becomes 0, with a warning.

### TOOL-MOB-03: Names
- `w3` is `name[,level]`.
- The name is cut to 23 characters, with a warning.
- `--en--` becomes the mob_db `Name`.
- `--ja--` becomes the mob_db `JapaneseName`, or `Name` when the mob has none.
- The level follows rAthena's `npc_parse_mob` (catalog §1.6, `MAX_LEVEL` = 275):
  - absent, non-numeric, or any negative value (including `-1`) means no `Level`, so the db level is used
    later;
  - `1..=275` is kept as `Level`;
  - `0` or above 275 skips the line (`bad level`).

### TOOL-MOB-04: Mob reference, amount, extras
- **Mob reference** (`w4` first field): a whole-token C `strtol` base-0 number must be a mob_db id.
  Otherwise the token is a case-insensitive AegisName. Anything else is a skip (`unknown mob`).
- **Amount** outside `1..=1000` is a skip (`bad amount`).
- **Event:** strip one leading and one trailing `"`. Keep the result only if it is at least 4 characters
  (`0` and `1` are dropped).
- **`size` / `ai`:** a value that rAthena would apply (size 1..2, ai > 0) gives a warning and is dropped.
  No current data has one.

### TOOL-MOB-05: Output and report
- One file per map under `--mob-out` (default `crates/ro-db/data/mob_spawn`), with the §1 header. Stale
  `*.yml` files are removed, like the NPC output.
- The report prints converted counts per keyword, the skip reasons, and the largest per-map mob total.
- Against `d4b8e7b`, the counts must match catalog §1.6: 3814 `monster`, 75 `boss_monster`, total
  `Amount` 65252, 411 maps. The test runs only when `RATHENA_PATH` is set.
- The NPC counts are unchanged. `monster` / `boss_monster` no longer appear in the skip report.

### DB-MOB-01: Load a map file
A valid file loads into `MapMobSpawns`. Every field round-trips: `MapMobSpawns` → YAML → `MapMobSpawns`
is equal. Default fields are omitted on write.

### DB-MOB-02: Reject bad files
Each of these gives a `DbLoadError` naming the path:
- a wrong `Header.Type` or an unsupported `Version`;
- a missing required field;
- an unknown field;
- `Amount` outside `1..=1000`;
- `MobId` outside the valid ranges;
- a YAML syntax error (the error also gives the line).

A missing file is `Ok(None)`, meaning the map has no mobs.

### DB-MOB-03: Shipped data loads
- Every `crates/ro-db/data/mob_spawn/*.yml` loads and starts with the SPDX and attribution header.
- There are more than 400 files.
- `crates/ro-db/data/NOTICE.md` has a "Mob spawn lists" section.

### PKT-MOB-01: Mob in `0x09ff` / `0x09fe` / `0x09fd`
`UnitAppearance` with `UnitObjectType::Mob` encodes the catalog §1.5 values:
- `object_type` `0x05`
- `aid` = mob gid, `GID` 0
- `speed` = the unit speed
- `job` = mob id; head, palettes, sex 0
- `clevel` = level
- `x_size` / `y_size` 0
- `max_hp` / `hp` -1 / -1 (full HP)
- `is_boss` 0
- name cut to 23 bytes + NUL

The player and NPC bytes don't change.

### ENG-MOB-01: Spawn
`MapInstance::spawn_mobs(&MapMobSpawns) -> usize` creates `Amount` entities per line, in file order, and
returns how many it created.
- **Gids:** each mob takes the next value of the instance's NPC id counter, so NPCs and mobs share one
  pool from `START_NPC_ID`. The map-server spawns NPCs first. rAthena uses one global pool; per instance
  is fine because a client only sees one map.
- **Placeholder stats:** `hp = max_hp = 1`, `level` = the line's `Level` or 1, `speed` 150 (rAthena's
  default `WalkSpeed`), facing 0. The name comes from the line.
- **Spawn group:** each mob records its line (the spawn group) so respawn can reuse it later.
- **Bad lines:** a line whose `X`/`Y` lies outside the grid is skipped with one warning (rAthena skips
  it at load).
- **Capacity:** when the entity capacity runs out, spawning stops with one warning.
- **Visibility:** every created mob is in the spatial grid.
- **No events:** spawning emits nothing, because no player is on the map yet.

### ENG-MOB-02: Placement
Follows catalog §1.1 and §1.4 at default config (`randomize_center_cell: yes`,
`force_random_spawn: no`, `no_spawn_on_player: 0`). The RNG is the instance's `SplitMix64`, seeded from
the map name; `rnd[a, b]` is inclusive.

1. **Rectangle:** `xs = 1` if `xs == 0 && x > 0`, and the same for `ys`.
2. **Map-wide** when `x == 0 && y == 0`, or when `xs == ys == 1` and `(x, y)` is not walkable. Treat
   the center as off-map and `xs = ys = 0`.
3. **Per mob, the center:** `(x, y)`. When `xs > 1`, `cx = rnd[x-xs+1, x+xs-1]`; the same for `cy` when
   `ys > 1`. A center outside the grid is clamped to it.
4. **Per mob, the cell.** Take the first rule that applies:
   - **Center shortcut:** take the center
     - for a `Boss` line with a fixed 1x1 rectangle (not map-wide);
     - for a non-boss line that is not map-wide, when `rnd() % (xs*ys) == 0` and the center is walkable.
   - **Free-cell search:** otherwise make 8 tries. Each picks `rnd[cx-(xs-1), cx+(xs-1)]` ×
     `rnd[cy-(ys-1), cy+(ys-1)]`; map-wide picks `rnd[15, w-16]` × `rnd[15, h-16]`. A try is rejected if
     it is the center itself, within 5 cells of the map edge (`x < 5`, `x > w-5`, same for y), or not
     walkable. The first accepted cell wins.
   - **Fallback:** if all 8 tries fail and the center is walkable, take the center. Otherwise make 50
     map-wide tries with the same rules.
   - **Failure:** if every try fails, see ENG-MOB-03.

Tests check properties, not exact cells:
- every mob is on a walkable cell;
- a fixed-rectangle mob is inside its rectangle widened by `xs-1`;
- a map-wide mob is inside `[15, w-16]`;
- a 1x1 walkable spawn sits on its cell;
- two instances of the same map place every mob identically.

### ENG-MOB-03: No cell found
If placement fails, that mob is not created, with a warning, and the remaining mobs still spawn.
rAthena retries 1000 ms later; that is deferred.

### ENG-MOB-04: Seen on entry and while walking
- A player entering gets a `UnitStanding` (`0x09ff`) for every mob within `AREA_SIZE`.
- A player whose walk brings a mob into view gets `UnitStanding`, and gets `UnitVanished { type 0 }`
  when it leaves view.
- Mobs receive nothing.

### ENG-MOB-05: Name request
`CZ_REQNAME` on a mob in the grid and within `AREA_SIZE` replies to the requester only with
`AckNpcName` (`0x0adf`) carrying the mob's name. `group_id` is 0 and the title is empty (mob_db
`GroupId`/`Title` are deferred). Out of range, or an unknown gid, gets no reply.

### BIN-MOB-01: Loaded per map
- With `MOB_SPAWN_DATA_DIR` (`--mob-spawn-data-dir`) set, a world that starts for `<map>.gat` loads
  `<dir>/<map>.yml`, and a player entering sees its mobs (`0x09ff`, `object_type` 0x05).
- A missing file means no mobs.
- A bad file logs a warning and means no mobs.
- An unset `MOB_SPAWN_DATA_DIR` logs one warning at startup and means no mobs.
- `run_metaling_servers.sh` points it at `crates/ro-db/data/mob_spawn`.

## 3. Licensing
- The data lives at `crates/ro-db/data/mob_spawn/`, under GPL-3.0 like `npc/`.
- `NOTICE.md` gets a "Mob spawn lists" section: source, commit, date, the changes, and the regenerate
  command. The NPC section stops listing "monster spawns" as dropped.
- The data commit is separate from the code commit.
- Test fixtures are small hand-written inline samples.

## 4. Deferred
These go to `DEFERRED.md` when the feature lands.

- **mob_db stats:** real `Hp`, `Level`, `WalkSpeed`, `is_boss` (from `Class: Boss`/`Mvp`), and
  `GroupId`/`Title` in `0x0adf`.
- **Death and respawn:**
  - Respawn delays: `delay2` only for boss lines, `delay1 < 1000` replaced by 5000, rate scaling, and a
    1000 ms floor.
  - On (re)spawn while players are present: `0x09fe` to `AREA_WOS`, plus the name to `AREA`.
- **Spawning gaps:**
  - The 1000 ms retry after a failed placement.
  - `dynamic_mobs` removal 300 s after the last player leaves, and `MAX_MOB_LIST_PER_MAP` (128 lines).
  - The spawn line's `size`/`ai`, and the `EF_GIANTBODY2`/`EF_BABYBODY2` effects (sent to the area on
    spawn, and to that player only on view-enter).
  - **Event labels** are stored in the YAML (`Event`) but nothing runs them. 139 lines have an event
    field, and 44 of those carry a real label such as `"ant_d02_i_boss::OnMobDead"` or
    `"illusion_mob#moonlight::OnKill"`. When the mob dies, rAthena runs that NPC label with the killer
    attached (catalog `mechanics/mobs.md` §3 step 9). This needs both mob death (combat) and the
    scripting VM. Quest and instance scripts that count kills this way won't progress until then.
  - The config knobs `mob_count_rate`, `force_random_spawn`, `no_spawn_on_player` and
    `override_mob_names`.
  - Once-spawns: the script `monster` command and `@monster`.
- **Movement and AI:** catalog §1.7.
