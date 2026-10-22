# Deferred work

Items consciously left out of finished work, with enough context to pick them up later.
Remove an item when it lands. Newest feature first.

## npc-spawning (branch `claude`, commits `1111051..a16d629`)

Spec: `specs/npc-spawning/spec.md` (§4 lists these too).

### Warp map change (next feature) prerequisites
- **Script portals stay inert.** About 970 positioned `script` NPCs with warp sprites (45/139) do `warp` from
  `OnTouch`; only plain `warp`/`warp2` entries carry a `WarpTrigger`. They need scripting.
- Same-map warp branch of `pc_setpos` (`pc.cpp:~7040-7080`) is not in the catalog yet.
- Warp destination `(0,0)` (random cell) is stored as-is; map change must handle it.
- **No cell->warp index.** Warps live in a `SparseSet<WarpTrigger>`; build a per-instance cell bitmap or
  cell->entity map of trigger areas in `spawn_npcs` (rAthena marks `CELL_NPC`) instead of scanning every warp
  per step.
- **Skip `Disabled` and `Cloaked` warps** at trigger time: `NpcInfo.state` keeps the state (rAthena
  `npc_touch_areanpc`).
- **Map name normalisation.** `WarpDest.map` is bare (`geffen`); worlds are keyed `<map>.gat` and
  `is_valid_map_name` rejects `@`, so `1@...` instance destinations fail. Add one helper for bare <-> `.gat`.

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
- Mobs, `ZC_NPCACK_MAPMOVE` on warps, status, combat, and rayon parallelism are future phases (`DESIGN.md`).
- `.rsw` water-level promotion of type-0 cells (`ro-db`).

### Performance (hot path; CLAUDE.md map-server rule)

- A* output is copied twice (scratch → `PathCells` → `WalkingPath`); build `WalkingPath` from the slice.
- The actor encodes an event before checking that any recipient is still joined.
- Spatial blocks are `SmallVec<[Entity; 8]>` (~295 KB for a 512x512 map); an intrusive list (`u32` head
  plus a `next` column) would shrink this if many maps run.
