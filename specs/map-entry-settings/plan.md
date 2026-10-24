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
  - User: manual client check again. **Failed** (`metaling-20261024-3.txt`): the new packets arrive correctly
    framed (lines 48–92) and the client still sends no `0x0447`.
- [ ] **Fix 2: the remaining login-only differences, in one batch** (handoff below; new session)
  - Spec §5 first (same pattern as §3), then code, `./verify.sh`, self-review, commit, one client check.

## Handoff for Fix 2 (written 2026-10-24, end of the Fix 1 session)

**Goal:** the client sends `0x0447` after the first map load (then we answer `0x0b1b`), the "account settings"
error goes away, and the chat box takes input. Until then the items manual check (DEFERRED.md) is blocked.

**Why one batch:** every attempt costs the user a client run and a capture. Two single-suspect attempts failed.
Send every remaining login-only difference at once. If that works, bisect afterwards: drop halves behind a
temporary flag, or just keep them all, since rAthena sends them anyway.

**Ruled out (sent, and the client still waits):** `0x0201` friends; `0x010f` with NV_BASIC; `0x0b20` ×2;
`0x0acb` ×4 + SKILLPOINT; `0x00bd` + `0x0141` ×12 + `0x013a` + ASPD + traits + `0x00be` f7..fc; `0x02c9`,
`0x02da`, `0x02d9` ×4, `0x0b8d`. Our `0x0b1b` reply works in tests but has never run against the real client.

**Still different, rAthena first login vs metaling** (`packet_rathena_item_2026-10-24.txt` file lines 37–114
vs `metaling-20261024-3.txt` lines 33–93; file line = proxy `#` + 10 in both):
1. **Before the client's first `0x007d`** (rAthena lines 41–70; metaling sends only `0x0283`, `0x0b18`,
   `0x02eb`, `0x0201`):
   - `0x008e` ×2 (MOTD and version line). The text is redacted and is rAthena's; send our own line.
   - **`0x0091` with the character's own map and cell (PF-20).** rAthena `pc_authok` → `clif_changemap` on a
     fresh login. The client then reloads the map and sends `0x007d` **twice** (rAthena lines 71 and 110).
     Strongest remaining suspect: the client may tie its "settings loaded" step to that second load.
     Engine hazard: the arrival-on-warp-cell path already emits `PlayerMapMove` after actor init (ENG-SET-05),
     and the entry settings consume `connect_new`/`changemap` on the first `0x007d`. Decide whether the settings
     go on the first or the second `0x007d`; rAthena sends them on the first (lines 88–107, before the second
     `0x007d` at 110).
   - The status diff push from `status_calc_pc` (PF-12): `0x00b0` MAXWEIGHT, `0x0141` ×6, then HIT, FLEE1,
     ASPD, ATK1, DEF1/DEF2, ATK2, FLEE2, CRITICAL, MATK, MDEF, `0x013a`, MAXHP, MAXSP, HP, SP, WEIGHT. The proxy
     misframed part of this (`0x0141` shows as one 84-byte frame = 6 × 14). Values: HP/SP/MaxHP/MaxSP are in
     `characters` (not yet in `CharacterStatus`); derived stats stay 0.
   - `0x09e7` (unread mail = 0), `0x0a24` (achievement update) and `0x0a23` (achievement list), `0x0ade`
     (overweight percent, 70). Layouts: `packets/settings.md` "Brief entries". With no achievements, `0x0a23`
     is still sent in the capture with n = 2; with n = 0 rAthena sends nothing. Send `0x0a24` with zeros, plus
     `0x0a23` with n = 0 if the client needs it.
2. **After `0x007d`, before the settings** (rAthena lines 73–87):
   - `0x01d7` look/weapon sprite (15 bytes, PF-01).
   - `0x099b` `ZC_NOTIFY_MAPPROPERTY2` (8 bytes, `00 00 00 06 00 00` on `prt_fild08`; catalog
     `packets/map.md` if present, else ask `metaling-explorer`).
   - `0x0adf` name replies pushed for each mob entry (rAthena sends them unasked; metaling answers `0x0368`).
     Unlikely to matter.
3. **Char server** (low suspicion, but cheap to diff): metaling `0x0b72` is 879 bytes per page and rAthena's is
   179. The login `0x0ac4` and char `0x082d`/`0x006b` aren't compared yet.

**Suggested order in the new session:**
1. Read this handoff, spec §3, `map_entry_actions.md#client-input-unblock--account-settings-0x0447---0x0b1b`,
   `packets/settings.md`, `packets/status.md`, PF-12 and PF-20 in `PACKET_FINDINGS.md`.
2. Use `metaling-explorer` for whatever is missing in the catalog: `0x099b` layout and flags, `0x0a24`/`0x0a23`
   zero forms, `0x09e7`, `0x0ade`, `0x01d7` at login, and rAthena's `pc_authok` → `clif_changemap` order
   relative to the MOTD and the status push.
3. Optional but valuable: the proxy cuts payloads after 32 bytes. Ask the user for a fresh rAthena login
   capture of a novice on `prt_fild08`, ideally with full payloads (a proxy flag would need adding), so the
   comparison is apples to apples.
4. Write spec §5 (scenarios `PKT-SET-05+`, `ENG-SET-07+`, `BIN-SET-02`), implement, verify, commit, then ask for
   one client check.

**Gotchas from this session:**
- `CharacterStatus` (ro-core) is loaded by `load_play_state` and carried through warps in `CharacterSpawn`;
  nothing writes it back. Add hp/sp/max_hp/max_sp there if the push needs them, and extend the PER-INV-04 test.
- `ENG-09` in `ro-engine/tests/map_instance_tests.rs` counts every actor-init event (52 now); update it.
- `bins/map-server/tests/settings_tests.rs` BIN-SET-01 and `item_tests.rs` `enter` read exact byte counts after
  `0x02eb` (23-byte burst + 4-byte `0x0201`). Anything new sent before `0x007d` shifts them.
- `./verify.sh --smoke` fails with "Port 6900 already in use" while the user's own servers run. Ask before
  stopping them.
- A `0x035f` with no reply in the metaling captures is expected: the click was ≥ 21 cells away, and rAthena
  `max_walk_path` = 17 also drops it silently.

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
