# Deferred work

Items consciously left out of finished work, with enough context to pick them up later.
Remove an item when it lands. Newest feature first.

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
- **Real-client check of the warp routes** (plan "Finish"): repeat one cross-map and one same-map warp with
  an observer through `tools/proxy` against metaling and diff with the rAthena captures. Needs the client.

## npc-spawning (branch `claude`, commits `1111051..a16d629`)

Spec: `specs/npc-spawning/spec.md` (§4 lists these too).

### Data not converted (tools/npc-import)
- Script bodies (and where the scripting phase reads them from: `.txt` via `Source:`, or extracted files).
- Floating (`-`) NPCs and `function`s, mob spawns (`monster`/`boss_monster`), mapflags, shop catalogs.
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
