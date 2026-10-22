# Deferred work

Items consciously left out of finished work, with enough context to pick them up later.
Remove an item when it lands. Newest feature first.

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
- Players only: `UnitAppearance` has no `object_type`; NPCs, mobs, portals, `ZC_ACK_REQNAMEALL_NPC`,
  `ZC_NPCACK_MAPMOVE` on warps, status, combat, and rayon parallelism are future phases (`DESIGN.md`).
- `.rsw` water-level promotion of type-0 cells (`ro-db`).

### Performance (hot path; CLAUDE.md map-server rule)

- Player name `String` is cloned into every entry/walk event; switch `UnitAppearance.name` to `Arc<str>`.
- A* output is copied twice (scratch → `PathCells` → `WalkingPath`); build `WalkingPath` from the slice.
- The actor encodes an event before checking that any recipient is still joined.
- Spatial blocks are `SmallVec<[Entity; 8]>` (~295 KB for a 512x512 map); an intrusive list (`u32` head
  plus a `next` column) would shrink this if many maps run.
