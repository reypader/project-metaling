# Specification: Map-entry settings and the load-confirm handshake

On metaling, right after the map loads, the client shows an "account settings" load error and the chat box takes
no input. So `@item` can't be typed and the items manual check is blocked. On rAthena the same client works.

Captures: [`metaling-20261024.txt`](../../reference/catalog/captures/metaling-20261024.txt) (metaling) and
[`packet_rathena_item_2026-10-24.txt`](../../reference/catalog/captures/packet_rathena_item_2026-10-24.txt) lines
71–114 (rAthena first login). Packet layouts: [`packets/settings.md`](../../reference/catalog/packets/settings.md).
Analysis: [`map_entry_actions.md#client-input-unblock--account-settings-0x0447---0x0b1b`](../../reference/catalog/mechanics/map_entry_actions.md#client-input-unblock--account-settings-0x0447---0x0b1b).

## 0. What the captures settled
- After every map load the rAthena client sends `0x0447 CZ_BLOCKING_PLAY_CANCEL` and the server replies `0x0b1b`
  (2 bytes, opcode only). The metaling client never sends `0x0447`.
- On same-map warps rAthena sends no settings packets and the client still sends `0x0447`. So the client waits for
  something only once per session, at first login. The captures can't tell which packet. This feature sends the
  whole default settings group, which is cheap, and the manual check settles whether it is enough.
- rAthena order after `0x007d`: inventory, weight, `0x099b`, unit entries, then
  - first login only (`connect_new`): `0x010f` skill list, `0x0b20` hotkeys tab 0 and tab 1, exp/status block;
  - first login and cross-map change (`changemap`): `0x02c9`, `0x02da`, `0x02d9` types 2, 1, 3, 5, `0x0b8d`;
  - then `0x007f` (the reply to the client's `0x0360`).
- At first login rAthena also sends an empty `0x0201` friends list right after `0x02eb`.

## 1. Design (defaults only)

### `ro-core`
- `ClientIntent::BlockingPlayCancel`.
- `ServerEvent` variants:
  - `FriendsList`: empty list.
  - `SkillList`: empty list.
  - `ShortcutKeyList { tab: u8 }`: empty bar.
  - `PartyConfig { deny_invites: bool }`.
  - `EquipWindowConfig { show_equip: bool }`.
  - `ConfigChanged { config_type: u32, value: u32 }`.
  - `ReputationList`: the four reputation types, 0 points each.
  - `LoadConfirmed`.

### `ro-packets`
- Decode `0x0447` (2 bytes) to `BlockingPlayCancel`.
- Encoders (all little-endian):

  | Packet | Bytes |
  | :--- | :--- |
  | `0x0201` | `01 02 04 00` |
  | `0x010f` | `0f 01 04 00` |
  | `0x0b20` | `20 0b 00 <tab> 00` + 266 zero bytes (271 total) |
  | `0x02c9` | `c9 02 <deny>` |
  | `0x02da` | `da 02 <show>` |
  | `0x02d9` | `d9 02 <type:u32> <value:u32>` |
  | `0x0b8d` | `8d 0b 45 00 01`, then `type:u64 points:i64` for types 1–4 (69 bytes) |
  | `0x0b1b` | `1b 0b` |
- `table.rs` lengths: `0x010f` Dynamic, `0x0b20` 271, `0x02c9` 3, `0x02da` 3, `0x0b8d` Dynamic, `0x0b1b` 2,
  `0x0b1d` 2. Also, so the proxy frames the rAthena burst: `0x0acb` 12, `0x00bd` 44, `0x0141` 14, `0x013a` 4.
- All new outbound opcodes are listed in `is_implemented_outbound_opcode`.

### `ro-engine`
- `PlayerInfo` gains `connect_new` and `changemap` flags:
  - `spawn_character` (login) sets both.
  - `enter_by_warp` (cross-map) sets `changemap` only.
  - A same-map warp sets neither.
  - `actor_init` clears both after use.
- `actor_init`, on the entry that puts the player into the grid, after the unit entries and before the
  arrival-cell warp check:
  - if `connect_new`: `SkillList`, `ShortcutKeyList { tab: 0 }`, `ShortcutKeyList { tab: 1 }`;
  - if `changemap`: `PartyConfig { false }`, `EquipWindowConfig { false }`, `ConfigChanged` for types 2, 1, 3, 5
    with value 0, then `ReputationList`.
- `BlockingPlayCancel` → `LoadConfirmed` to self. Like other gameplay intents it is ignored before actor init.
  This matches rAthena, where the dispatcher drops it while the player isn't on the map.

### `map-server`
- `FriendsList` is sent right after `ZC_ACCEPT_ENTER` on a fresh login (not on a warp hand-off).
- The `0x0447` intent reaches the engine through the existing catch-all forward.

## 2. Scenarios

### PKT-SET-01: Decode `CZ_BLOCKING_PLAY_CANCEL`
`47 04` decodes, via `decode_frame` and the intent parser, to `BlockingPlayCancel`.

### PKT-SET-02: Encode the settings packets
Each new `ServerEvent` encodes to the bytes in the §1 table. `0x02c9 00`, `0x02da 00`, `0x02d9` types 2/1/3/5 and
the 69-byte `0x0b8d` header match the capture (file lines 102–107, proxy `#` 92–97, after the misframe
correction in `settings.md`).

### PKT-SET-03: Table lengths
`lookup_packet_length` returns the §1 lengths, and a stream of the corrected rAthena burst
(`0x02c9`, `0x02da`, `0x02d9`, `0x0b8d`) frames without bogus frames.

### ENG-SET-01: First login sends the settings group once
After `spawn_character` and `NotifyActorInit`, the player gets, after its unit entries and in this order:
`SkillList`, `ShortcutKeyList` tab 0 and tab 1, `PartyConfig`, `EquipWindowConfig`, `ConfigChanged` ×4 (types
2, 1, 3, 5), `ReputationList`. A second `NotifyActorInit` sends none of them again.

### ENG-SET-02: Cross-map warp sends only the changemap part
A player arriving by `enter_by_warp` gets the `0x02c9`…`0x0b8d` part after `NotifyActorInit`, and no skill list or
hotkeys.

### ENG-SET-03: Same-map warp sends none
After a same-map warp and the following `NotifyActorInit`, no settings events are sent.

### ENG-SET-04: Load confirm
`BlockingPlayCancel` after actor init → exactly one `LoadConfirmed` to the sender. Before actor init it is ignored.

### ENG-SET-05: Login on a warp cell
Logging in on a same-map warp cell sends the settings group before the `PlayerMapMove`, and the next
`NotifyActorInit` sends none.

### BIN-SET-01: Login over TCP
A client that logs in and sends `0x007d` then `0x0447` receives `0x0201` after `0x02eb`, the settings frames, and
then `0x0b1b`.

## 3. Fix 1 (2026-10-24): NV_BASIC entry and the first-login status block
The settings group alone did not unblock input: in
[`metaling-20261024-2.txt`](../../reference/catalog/captures/metaling-20261024-2.txt) the client gets every
settings packet and still never sends `0x0447`. Next try, chosen with the user: the two remaining first-login
(`connect_new`) differences after `0x007d`.

### Design
- `ro-core`:
  - `CharacterStatus` (allocated STR..LUK, status and skill points, base and job exp). Default is a fresh
    character: stats 1, 48 status points, 0 skill points, 0 exp. It replaces the lone `str` on `CharacterSpawn`
    and `PlayState`. Persistence loads it from `characters`.
  - `SkillInfo` and `SkillList { skills }`.
  - `StatusWindow` (the `0x00bd` fields) and the events `LongLongParameterChanged` (`0x0acb`), `StatusWindow`
    (`0x00bd`), `CoupleStatus` (`0x0141`), `AttackRange` (`0x013a`), `StatusCost` (`0x00be`).
- `ro-engine`, the `connect_new` part of the entry settings, in rAthena order:
  1. `SkillList` with one entry: NV_BASIC (id 1), inf 0, level 0, sp 0, range 0, upgradable.
  2. `0x0b20` tab 0 and tab 1.
  3. `0x0acb` BASEEXP (1), NEXTBASEEXP (22), JOBEXP (2), NEXTJOBEXP (23); then `0x00b0` SKILLPOINT (12).
  4. `0x00bd`; `0x0141` STR..LUK (13..18, bonus 0); `0x013a` 1; `0x00b0` ASPD (53); `0x0141` POW..CRT
     (219..224); `0x00b0` PATK..CRATE (225..230), TRAITPOINT (231), AP (232), MAXAP (233); `0x00be` UPOW..UCRT
     (247..252).
- Values:
  - Allocated stats, status and skill points and exp come from `CharacterStatus`.
  - Stat-raise costs (renewal): `2 + (s-1)/10` below 100, `16 + 4*((s-100)/5)` from 100, 0 at the cap. The cap
    is 99 for every job until job classes are modelled.
  - There are no exp tables yet, so NEXT*EXP is rAthena's max-level value (99999999 base, 999999999 job). The
    client never divides by 0.
  - Derived stats (ATK, MATK, DEF, MDEF, HIT, FLEE, CRIT, ASPD) are 0 until `ro-mechanics` exists. Attack range
    is 1 (bare hands). Trait stats, their costs and AP are 0.

### Scenarios
- **PKT-SET-04:** the new events encode to their layouts: `0x010f` with NV_BASIC is 41 bytes and starts
  `0f 01 29 00 01 00`; `0x0acb` 12, `0x00bd` 44, `0x0141` 14, `0x013a` 4, `0x00be` 5 bytes. Every new opcode is
  marked implemented.
- **ENG-SET-06:** after a login and `NotifyActorInit`, the `connect_new` events arrive in the order above, with the
  loaded stats and their costs. A cross-map arrival sends none of them.
- **BIN-SET-01** (updated): `0x010f` is 41 bytes, and `0x00bd` (44) arrives before `0x02c9`.
- **DB-PERS** (`load_play_state`): the stored stats, points and exp load into `CharacterStatus`.

## 4. Deferred
- Saving hotkeys (`CZ_SHORTCUT_KEY_CHANGE*`, row shift) and config toggles; loading them at entry.
- A real skill list (NV_BASIC and job skills) with the skill tree.
- The exp/status block (`0x0acb`, `0x00bd`, `0x0141`, `0x013a`, trait `0x00be`); see PF-12.
- If this group doesn't unblock input, the next suspects are login-time `0x0091` (PF-20), then the status block.

## 5. Fix 2 (2026-10-24): the remaining login-only differences, in one batch
Fix 1 failed too ([`metaling-20261024-3.txt`](../../reference/catalog/captures/metaling-20261024-3.txt)). Every
attempt costs a client run, so this one sends every remaining login difference at once (plan handoff).

What the captures show: in both rAthena logins of
[`packet_rathena_item_2026-10-24.txt`](../../reference/catalog/captures/packet_rathena_item_2026-10-24.txt)
(proxy `#` 33 → 61 → 100 → 103 and 249 → 303 → 306 → 349), the server sends `0x0091` with the character's own
cell before the first `0x007d`. The client sends `0x007d` twice, and `0x0447` follows the second one. On warps,
too, `0x0447` follows the `0x007d` of a `0x0091` load. So the client most likely sends `0x0447` only after a load
that a `0x0091` started, and metaling never sends `0x0091` at login. That is the main suspect; the other items
below come along in the same batch.

### Design
- `ro-core`:
  - `CharacterStatus` gains `hp`, `max_hp`, `sp`, `max_sp` (default 40/40/11/11, the schema defaults);
    persistence loads them.
  - `AchievementSummary { total_score, level, exp, exp_to_next }`.
  - `ServerEvent`: `SpriteChanged` (`0x01d7`), `MapProperty` (`0x099b`), `UnreadMail` (`0x09e7`),
    `AchievementUpdate` (`0x0a24`), `OverweightPercent` (`0x0ade`).
- `ro-packets` encoders (lengths already in `table.rs`; all marked implemented):

  | Packet | Bytes |
  | :--- | :--- |
  | `0x01d7` | `d7 01 <gid:u32> <type:u8> <val:u32> <val2:u32>` (15) |
  | `0x099b` | `9b 09 <property:u16> <flags:u32>` (8) |
  | `0x09e7` | `e7 09 <unread:u8>` (3) |
  | `0x0a24` | `24 0a <score:u32> <level:u16> <exp:u32> <exp_to_next:u32>` + 50 zero bytes (66) |
  | `0x0ade` | `de 0a <percent:u32>` (6) |
- `ro-engine`:
  - `spawn_character` returns the login burst (all to self) instead of an empty list:
    1. `0x008e` "Welcome to Metaling." (rAthena sends its version line and MOTD here).
    2. `0x0091` with the map's `.gat` name and the spawn cell.
    3. The status push of rAthena's first `status_calc_pc`, every field once, in captured order: `0x00b0`
       MAXWEIGHT; `0x0141` STR..LUK (bonus 0); `0x00b0` HIT, FLEE1, ASPD, ATK1, DEF1, DEF2, ATK2, FLEE2,
       CRITICAL, MATK2, MATK1, MDEF2, MDEF1 (all 0); `0x013a` 1; `0x00b0` MAXHP, MAXSP, HP, SP (loaded), WEIGHT.
    4. `0x09e7` 0; `0x0a24` score 0, level 0, exp 0, exp_to_next 18 (`achievement_level_db` level 1); `0x0ade`
       70. No `0x0a23`: rAthena skips it when there are no achievements.
  - The actor init that shows the player (login, both warp kinds) starts with `0x01d7` `LOOK_WEAPON` with
    the right-hand and left-hand client ids (0 when empty; a two-handed weapon gives val2 0), to self only.
    After the spawn to others it sends `0x099b` property 0, flags `0x0600` (USECART and SUNMOONSTAR, rAthena's
    plain-map default), before the unit entries.
  - A repeated `0x007d` (the login reload) still only re-sends the inventory (ENG-INV-04), with no `0x01d7` or
    `0x099b`. The settings stay on the first `0x007d`, as in rAthena.
- `map-server`: on a login, sends `0x02eb`, `0x0201`, then the returned burst, in that order.

Left out (`DEFERRED.md`): `0x01d7` to observers, the unasked `0x0adf` name pushes, `0x0a23`, a configurable
MOTD, and the char-server `0x0b72` size difference.

If this works, bisecting is optional: the batch matches what rAthena sends.

### Scenarios
- **PKT-SET-05:** the five new events encode to the bytes above (`0x01d7`, `0x099b`, `0x09e7`, `0x0ade` and the
  `0x0a24` header match the capture), with lengths 15/8/3/66/6 in `table.rs`, and are marked implemented.
- **ENG-SET-07:** `spawn_character` returns exactly the burst above, in order, all to self, with the loaded STR,
  AGI and HP/SP and max weight from STR. Nothing is queued for the next tick. `enter_by_warp` returns no burst.
- **ENG-SET-08:** a login's actor init starts with `0x01d7` (gid = account id, type 2, 0/0 with empty hands).
  `0x099b` follows SP_MAXWEIGHT and the spawn to others and comes before the first unit entry. A second `0x007d`
  sends neither. A same-map warp and a cross-map arrival send both again. Unit test: `hand_looks` for empty
  hands, an unworn weapon, a weapon plus shield, and a two-handed weapon.
- **BIN-SET-01** (updated): after `0x0201` the client gets `0x008e`, `0x0091` (own map and cell), the status
  push, then `0x09e7`, `0x0a24`, `0x0ade`. After `0x007d` it gets `0x01d7` (15 bytes) and `0x099b`. A second
  `0x007d` gets only the inventory frames, and `0x0447` gets `0x0b1b`.
- **DB-PERS** (PER-INV-04): HP, max HP, SP and max SP load into `CharacterStatus`.
