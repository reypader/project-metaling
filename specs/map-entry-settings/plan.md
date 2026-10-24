# Implementation Plan: Map-entry settings

Spec: `specs/map-entry-settings/spec.md`. Planned 2026-10-24. Scope chosen with the user: defaults only (no
persistence).

## Milestones
- [x] **M1 ro-core + ro-packets** (2026-10-24)
  - `ClientIntent::BlockingPlayCancel` and the `ServerEvent` variants.
  - `0x0447` decoder.
  - New `outgoing/settings.rs` encoders.
  - `table.rs` lengths, `opcode.rs` names, `is_implemented_outbound_opcode`.
  - Scenarios: PKT-SET-01..03.
- [x] **M2 ro-engine** (2026-10-24)
  - `connect_new` / `changemap` on `PlayerInfo`.
  - `actor_init` settings group.
  - `BlockingPlayCancel` → `LoadConfirmed`.
  - Scenarios: ENG-SET-01..04.
- [x] **M3 map-server** (2026-10-24)
  - `FriendsList` after `0x02eb` on a fresh login.
  - Scenario: BIN-SET-01.
- [x] **Finish** (2026-10-24; manual client check pending, tracked in `DEFERRED.md`)
  - `DEFERRED.md` section.
  - `ro-engine/DESIGN.md` entry-burst note.
  - `./verify.sh --smoke`.
  - Reviewer over the feature diff, fixes, commit.
  - User: manual client check (no settings error, chat works, `0x0447` → `0x0b1b` in the proxy log), then resume
    the items check.

- [x] **Fix 1: NV_BASIC + status block** (2026-10-24; manual client check pending) (spec §3; `metaling-20261024-2.txt` showed the group alone isn't enough)
  - ro-core `CharacterStatus`, `SkillInfo`, `StatusWindow`, new events; persistence loads the status.
  - ro-packets encoders + tests (PKT-SET-04); engine block (ENG-SET-06); BIN-SET-01 update.
  - `./verify.sh`, self-review (small fix: no reviewer run), commit.
  - User: manual client check again.

## Implementation notes
- Research changed two things from the planning session: hotkeys are `0x0b20` (271 bytes), not `0x0a00`, at
  20260219; and `0x0b8d` is not a per-load trigger (same-map warps send no settings and the client still sends
  `0x0447`). So the whole default group goes out, and the manual check decides.
- `0x0b20` was named `ZC_SHORTCUT_KEY_LIST_V4` because `ZC_SHORTCUT_KEY_LIST` already names `0x02b9`.
- The skill list is empty (4 bytes), not rAthena's NV_BASIC level-0 entry.
- `connect_new` / `changemap` live on `PlayerInfo`; no map-server flag was needed, because login and warp
  already use different engine entry points (`spawn_character` / `enter_by_warp`).
- Table lengths for the rAthena status block (`0x0acb`, `0x00bd`, `0x0141`, `0x013a`) were added so the proxy
  frames rAthena captures; metaling doesn't send those yet.
- `bins/map-server/tests/item_tests.rs` `enter` now reads the 4-byte friends list after the 23-byte burst.
- Review (2026-10-24): no high-severity findings. Fixed:
  - BIN-WARP-01/02 now check the changemap settings on cross-map arrival and their absence on a same-map
    warp, and login-only frames are in the "not after a warp" list.
  - `MapInstance::spawn_player` takes an explicit `Arrival` (`Login` / `MapChange`), so a future spawn path
    must choose; `World::spawn_player` defaults both flags off.
  - ENG-SET-05: logging in on a warp cell sends the settings before the `0x0091`, and none after it.
  - The test helper `is_dynamic` knows `0x0201`/`0x010f`/`0x0b8d`; `is_entry` no longer reuses it.
  - DEFERRED gained the same-map-warp-cell `0x0447` case and the NV_BASIC fallback.
  - Docs: capture line-number convention, the `0x0b1b` name alias, the README's `0x0a00` misframe.
- Fix 1 (2026-10-24):
  - `CharacterStatus` replaced the lone `str` on `CharacterSpawn`, `PlayState` and `PlayerInfo`. A cross-map
    warp carries it in the hand-off spawn; nothing writes it back yet, since nothing changes it.
  - The status block lives in `ro-engine/src/status.rs` (stat cost formula included), next to
    `inventory::max_weight`, rather than starting `ro-mechanics` for one function.
  - NEXT*EXP uses the max-level constants, not 0, so the client can't divide by 0.
  - `ServerEvent::SkillList` now carries its entries; the engine builds NV_BASIC.
  - Small fix inside a reviewed feature: `./verify.sh` + self-review, no reviewer run.
