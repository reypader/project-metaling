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

## 3. Deferred
- Saving hotkeys (`CZ_SHORTCUT_KEY_CHANGE*`, row shift) and config toggles; loading them at entry.
- A real skill list (NV_BASIC and job skills) with the skill tree.
- The exp/status block (`0x0acb`, `0x00bd`, `0x0141`, `0x013a`, trait `0x00be`); see PF-12.
- If this group doesn't unblock input, the next suspects are login-time `0x0091` (PF-20), then the status block.
