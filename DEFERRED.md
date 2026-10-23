# Deferred work

Items consciously left out of finished work, with enough context to pick them up later.
Remove an item when it lands. Newest feature first.

## mob-spawning (branch `claude`, commits `262f83a..2093eaf`)

Spec: `specs/mob-spawning/spec.md` (§4 lists these too). Mobs are visible, named, inert units.

### Still to verify by hand
- **Real-client check** (plan Finish, optional): walk `prt_fild08` against metaling and rAthena through
  `tools/proxy`. Compare the `0x09ff` fields and the `0x0adf` reply. The wire values come from source only
  (catalog `mechanics/mobs.md` §1.5).

### mob_db stats
- Real `Hp`, `Level`, `WalkSpeed` and `is_boss` (1 for `Class: Boss`, 2 for `Mvp`). Today every mob has
  1 HP, level 1 (or the line's level) and speed 150, and `is_boss` is always 0.
- `GroupId` / `Title` in `0x0adf`. Today both are empty.

### Death and respawn (need combat)
- **Respawn delays:** `delay2` counts only for `boss_monster` lines (`mob_spawn_variance: 1`); a given
  `delay1 < 1000` becomes 5000; rate scaling; a 1000 ms floor. `SpawnGroup.line` keeps the raw values.
- **On (re)spawn while players are present:** `0x09fe` to `AREA_WOS`, then the name to `AREA`.
- **Event labels** are stored in the YAML (`Event`) but nothing runs them. 139 lines have an event field,
  and 44 of those carry a real label such as `"ant_d02_i_boss::OnMobDead"` or
  `"illusion_mob#moonlight::OnKill"`. When the mob dies, rAthena runs that NPC label with the killer
  attached (catalog `mechanics/mobs.md` §3 step 9). This needs mob death (combat) and the scripting VM.
  Quest and instance scripts that count kills this way won't progress until then.

### Spawning gaps
- The 1000 ms retry after a failed placement (ENG-MOB-03): the mob is simply not created.
- `dynamic_mobs` removal 300 s after the last player leaves, and `MAX_MOB_LIST_PER_MAP` (128 lines). Map
  worlds start on first entry, so spawning at world start already matches rAthena's first-entry spawn.
- The spawn line's `size`/`ai`, and the `EF_GIANTBODY2` / `EF_BABYBODY2` effects (to the area on spawn,
  to that player only on view-enter). No current data sets them; the importer warns and drops them.
- The config knobs `mob_count_rate`, `force_random_spawn`, `no_spawn_on_player` and `override_mob_names`
  (all at rAthena defaults).
- Once-spawns: the script `monster` command and `@monster`.
- Mob gids are per map instance (rAthena: one global pool shared with NPCs).

### Movement and AI
- No random walk, chase or AI timers. Catalog `mechanics/mobs.md` §1.7 has the rAthena rules: the
  4000 ms + `rnd()%1000` walk timer, hard AI (100 ms) only within 18 cells of a player, lazy AI (1000 ms)
  otherwise, and `mob_randomwalk`'s ±7 window.

## exit-game (branch `claude`, commit `d666b53`)

Spec: `specs/exit-game/spec.md` (§3 lists these too).

### Still to verify by hand
- **Real-client check** (plan M5). "Exit to Windows" through `tools/proxy` should close the client, and
  the observer should see `0x0080` type 2. The character should then log back in at the cell where it quit.

### Robustness (`bins/map-server/src/{service,session}.rs`)
- **Exit or restart racing a cross-map handoff.** If map A hands the session off just as `0x018a`/`0x00b2`
  arrives, the session task can still forward it to A, which drops it. A now logs this at `warn`.
  - Effect: no ack. The client stays in game, loads the destination map and plays on. Pressing Exit
    again works. Server state stays consistent: the claim moves to B as in any warp.
  - The same window applies to every intent. Fix it in the handoff, e.g. A keeps handed-off session ids
    for one tick and forwards their intents.
  - Test: warp and quit in the same tick, then assert `0x018b` arrives.

### Needs combat
- **Refusing the exit (`0x018b` result 1, "wait 10 seconds").** rAthena refuses while the character is
  hidden or cloaked, or within `prevent_logout` (10 s) of attacking, casting or taking damage. The same
  check refuses `CZ_RESTART`.
- **Delayed removal.** A client that closes inside the `prevent_logout` window stays in the world for 10 s.

## warp-map-change (branch `claude`, commits from `2a7904f`)

Spec: `specs/warp-map-change/spec.md` (§3 lists the scope cuts).

### Out of scope
- **Script portals stay inert.** About 970 positioned `script` NPCs with warp sprites (45/139) do `warp` from
  `OnTouch`; only plain `warp`/`warp2` entries carry a `WarpTrigger`. They need scripting. Warp skills
  (Teleport, Warp Portal) and `@warp`/`@go` too.
- **Entry content:** rAthena's login-time `0x0091` ([PF-20](PACKET_FINDINGS.md#pf-20)), `0x01d7` to observers
  at entry ([PF-01](PACKET_FINDINGS.md#pf-01)), and the inventory re-send at entry.
- **Hiding.** There is no hiding state, so the "hidden players don't warp unless `warp2`" skip is missing;
  `WarpTrigger.trigger_while_hidden` is stored but unused.
- `pc_job_can_entermap` (job/group restrictions on the destination).
- **Multi-map-server handoff** (char-server reconnect) and instance (`1@...`) destinations: instance
  warps are inert at load.

### Robustness
- **Blocking map load on the session task.** `get_or_spawn_world` reads the `.gat` and NPC files on the
  calling task; a warp to a map not yet running now does this on the session task.
- **Re-login on a warp cell after a disconnect in transit** (spec §1 deviation): `rewarp_count` resets,
  and a warp disabled later leaves the player standing on it.
- With `MAP_DATA_DIR` set, a map whose own `.gat` is missing (fallback grid) treats its same-map warps as
  inert too.
- **Packets while loading.** rAthena drops everything but `0x007d` while the player is off the map. We
  still answer `0x0360` (as the spec allows) and act on Restart `0x00b2` on the destination before
  `0x007d`.

## npc-spawning (branch `claude`, commits `1111051..a16d629`)

Spec: `specs/npc-spawning/spec.md` (§4 lists these too).

### Data not converted (tools/npc-import)
- Script bodies (and where the scripting phase reads them from: `.txt` via `Source:`, or extracted files).
- Floating (`-`) NPCs and `function`s, mapflags, shop catalogs.
- Barter NPCs defined in `npc/**/barters.yml`.
- rAthena drops shops with zero valid items; we keep every shop (no item_db yet).

### Fidelity
- NPC names go on the wire as UTF-8; the client expects CP949 (6 headers in `quests_17_2.txt` /
  `enchan_sage_legacy_17_2.txt` use Greek letters).
- PC-job-sprite NPCs: rAthena also sets `body = class` and clamps hair; guild-flag NPCs send emblem data.
- `Dir` values above 7 (8, 15, 55 in data) are sent masked to 4 bits; the client's handling is unverified.
- NPC ids are per map instance (rAthena: one global pool shared with mobs/pets).
- Name requests answer only NPCs in the spatial grid (rAthena also answers disabled NPCs).
- The engine treats every sprite except `-1`/`32767` as visible. rAthena hides classes outside its view
  ranges (`mechanics/npcs.md`); the current data has none, but new data could.
- Real-client proxy capture of NPC entry (`0x09ff` object_type 6) and `0x0adf`: everything is source-derived.

## engine-foundation (branch `claude`, commits `ed7c0d9..af3445d`)

Spec: `specs/engine-foundation/spec.md` (§6 lists the scope cuts below as well).

### Still to verify by hand

- **Real-client proxy check** (plan "Final checks"): run two real clients through `tools/proxy` against
  metaling and against rAthena (walk in and out of view, sit, turn, name request) and diff the captures in
  `tools/proxy/packet_captures/`. Not done: needs the game client.

### Robustness (map-server, `bins/map-server/src/{service,session}.rs`)

- **Pre-auth handshake timeout.** A truncated `CZ_ENTER` with no EOF keeps the socket open forever
  (slowloris). BIN-01 was reworded to "truncated `CZ_ENTER` then EOF".
- **Duplicate-login policy and idle timeout.** If a half-open socket still holds a character, a reconnect
  is rejected (spec BIN-01) until the old TCP connection dies. rAthena kicks the old session or sends a
  duplicate-login notice instead.
- **World spawn cap on entry.** `resolve_entry` starts a world for any valid stored `last_map` without the
  `MAX_HOSTED_WORLDS` check that `accept_transfer` does.
- **Map-side auth.** `login_id1` is not checked against a char-server ticket (needs a handoff that doesn't
  exist yet). Until then anyone can send `CZ_ENTER` for any visible account/char id pair.
- **Slow-client eviction scenario and tick-budget assertion** (old BIN-SRV-05). The `try_send` eviction
  exists but has no dedicated test.

### Engine fidelity (`crates/ro-engine`)

- `OFFICIAL_WALKPATH` straight-line rule for 15–17 step paths.
- rAthena's 32x32 A* node-table collision quirk and equal-cost tie-breaking parity.
- Sub-cell offsets other than (8, 8) in `0x09fd` (rAthena uses them when a walk is cut short mid-cell).
- Walk speed clamp: speed 0 from the DB makes every step complete in one tick (rAthena clamps speed).
- Weight / max weight are placeholders (`0` / `20300`) until inventory and stats exist.
- Mobs, status, combat, and rayon parallelism are future phases (`DESIGN.md`).
- `.rsw` water-level promotion of type-0 cells (`ro-db`).

### Performance (hot path; CLAUDE.md map-server rule)

- A* output is copied twice (scratch → `PathCells` → `WalkingPath`); build `WalkingPath` from the slice.
- The actor encodes an event before checking that any recipient is still joined.
- Spatial blocks are `SmallVec<[Entity; 8]>` (~295 KB for a 512x512 map); an intrusive list (`u32` head
  plus a `next` column) would shrink this if many maps run.
