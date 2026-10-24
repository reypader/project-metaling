# First-login settings burst (`clif_parse_LoadEndAck` tail)

Target `PACKETVER = 20260219` (`PACKETVER_MAIN_NUM = PACKETVER`, no `RE`/`ZERO`; `src/config/packets.hpp:19-28`, so every `PACKETVER_MAIN_NUM >= X` gate below is TRUE and `PACKETVER_RE_NUM` gates are FALSE). Provenance: rathena `d4b8e7b8f`, `src/map/{clif.cpp,pc.cpp,packets.hpp,packets_struct.hpp,clif.hpp}`, `src/common/mmo.hpp`, `db/re/reputation.yml`. Cross-checked against [packet_rathena_item_2026-10-24.txt](../captures/packet_rathena_item_2026-10-24.txt) (first login lines 37-114; second login ~252-360). All little-endian, packed, Map to Client unless noted.

## Capture misframing warning (important when reading the capture)

The proxy has no length for `0x010f` (it is variable) and a wrong one for `0x0b20`, so everything from capture line 78 to 97 is mis-split. Real boundaries, proven by byte arithmetic (sum of the misframed frames 78-97 = 1080 bytes; the packets below add up exactly):

| Capture shows | Really is |
| :--- | :--- |
| line 78 `0x010f` 634 bytes | `0x010f` 41 (payload begins `29 00` = length 41) + `0x0b20` 271 + `0x0b20` 271 + 4x `0x0acb` (12 each) + `0x00b0` SP_SKILLPOINT (8) + first 5 bytes of `0x00bd` |
| lines 79-85 (`0x0200`, `0x0102`, `0x0100`, `0x0a00` 269, bogus `0x0000`) | rest of `0x00bd` (44) + 18x `0x0141`/`0x013a`/`0x00b0` (252 bytes total, see order below). No `0x0a00` is ever sent for 20260219. |
| line 92 `0x02c9` 6 bytes, payload `00 da 02 00` | `0x02c9` 3 bytes (`c9 02 00`) + `0x02da` 3 bytes (`da 02 00`) |
| line 97 `0x0b8d` 69 | correct (variable, length field = 0x45) |

The second login (line 331) shows `0x010f` as 644 bytes; same cause, only the swallowed tail differs (not decoded further). Correct `table.rs` lengths: `0x010f` Variable (u16 len at offset 2), `0x0b20` Fixed(271), `0x0acb` Fixed(12), `0x0141` Fixed(14), `0x013a` Fixed(4), `0x02c9` Fixed(3), `0x02da` Fixed(3), `0x02d9` Fixed(10, already), `0x0b8d` Variable (len at offset 2), `0x0b1b` Fixed(2), `0x0b1c` Fixed(2), `0x0b1d` Fixed(2), `0x09e7` Fixed(3), `0x0a24` Fixed(66), `0x0a23` Variable, `0x0ade` Fixed(6), `0x00bd` Fixed(44), `0x00be` Fixed(5) (already), `0x01d7` Fixed(15) (already). `0x0a00` and `0x0200`(18) are not used by this flow.

## Packet reference

| Opcode | Name | Frame | Layout after opcode | Source |
| :--- | :--- | :--- | :--- | :--- |
| `0x010f` | `ZC_SKILLINFO_LIST` | 4 + 37*n | `len:u16`, then n x skill entry (below) | `packets_struct.hpp:4271-4280`, `clif.cpp:5696-5750` |
| `0x0b20` | `ZC_SHORTCUT_KEY_LIST` (V4) | 271 | `rotate:u8, tab:u16, 38 x {isSkill:u8, id:u32, count:u16}` | `packets_struct.hpp:1575-1590`, `clif.cpp:11217-11245` |
| `0x02c9` | `ZC_PARTY_CONFIG` | 3 | `denyPartyInvites:u8` (0 allow, 1 auto-deny) | `packets_struct.hpp:4038-4043`, `clif.cpp:7914-7924` |
| `0x02da` | `ZC_CONFIG_NOTIFY` | 3 | `showEquip:u8` (0 disabled, 1 allow others to view equipment) | `packets.hpp:1955-1959`, `clif.cpp:10301-10311` |
| `0x02d9` | `ZC_CONFIG` | 10 | `type:u32, value:u32` | `clif.cpp:10287-10298`, `clif_packetdb.hpp:920` |
| `0x0b8d` | `ZC_REPUTE_INFO` | 5 + 16*n | `len:u16, success:u8, n x {type:u64, points:i64}` | `packets.hpp:249-261`, `clif.cpp:24272-24290` |
| `0x00be` | `ZC_STATUS_CHANGE` | 5 | `statusID:u16, value:u8` | `packets_struct.hpp:366-373`, `clif.cpp:3745-3752,3865-3875` |
| `0x0447` | `CZ_BLOCKING_PLAY_CANCEL` (C to Map) | 2 | none | `clif_packetdb.hpp:1244,1607`, `clif.cpp:20016-20024` |
| `0x0b1b` | `ZC_NOTIFY_ACTORINIT` ("load confirm"; metaling names it `ZC_LOAD_CONFIRM`) | 2 | none | `packets_struct.hpp:4029-4033`, `clif.cpp:20000-20012` |

### `0x010f` skill entry (37 bytes, name form)
`id:u16, inf:i32 (skill_get_inf), level:u16, sp:u16, range2:u16, name:char[24] (skill name, NUL padded), upFlag:u8`. The 22-byte `level2` form with opcode `0x0b32` is gated `PACKETVER_RE_NUM >= 20190807 || ZERO_NUM >= 20190918`, FALSE here. `upFlag = 1` when the skill is permanent (not quest/item-granted) and level < max for the class. `WE_CALLPARTNER` is skipped and sent separately; packet is capped at 8192 bytes, extra skills go one by one via `0x0111` (`ZC_ADD_SKILL`, 39 bytes) + `0x07e1`. Fresh novice capture: one entry, `01 00 | 00 00 00 00 | 00 00 | 00 00 | 00 00 | "NV_BASIC"+NUL pad | 00`, i.e. frame `0x010f 29 00 ...` = 41 bytes (NV_BASIC level 0, `upFlag` byte not visible in the truncated capture; NV_BASIC is level 0).

### `0x0b20` hotkeys
`hotkey_data` = 7 bytes: `isSkill:u8` (0 item, 1 skill), `id:u32` (item/skill id), `count:u16` (item qty / skill level). `MAX_HOTKEYS = 38` (`mmo.hpp:~28-33`, 38 for `PACKETVER >= 20090617`), packet carries 38 slots; DB keeps 76 (`MAX_HOTKEYS_DB`, `mmo.hpp:36`) as two banks. `rotate` = `hotkey_rowshift` (tab 0) or `hotkey_rowshift2` (tab 1), the row-shift of the visible bar; `tab` = bank index 0 or 1. Empty bar = 271 bytes: `0x0b20`, `rotate 00`, `tab 00 00` (resp. `01 00` for the second), then 266 zero bytes. The `0x0a00` form (no `tab`, 269 bytes) is only for `PACKETVER_MAIN_NUM < 20190522`; do not send it. Client change packets use `PACKET_CZ_SHORTCUT_KEY_CHANGE*` / `HotkeyRowShift` (`clif.cpp:11248+`, not extracted here).

### `0x02d9` ZC_CONFIG types (`enum e_config_type`, `clif.hpp:719-726`)
0 `OPEN_EQUIPMENT_WINDOW` (client-side show-equip toggle reply; not sent at login, `0x02da` carries it), 1 `CALL` (disable `Call` partner request; value = `status.disable_call`), 2 `PET_AUTOFEED`, 3 `HOMUNCULUS_AUTOFEED`, 5 `DISABLE_SHOWCOSTUMES` (`PACKETVER >= 20230419`; value = `status.disable_showcostumes`). 4 is unused. Defaults sent for a fresh char: all four with value 0, in the order 2, 1, 3, 5 (capture lines 93-96). Type 2 is sent always because `pet_autofeed_always` defaults yes (`conf/battle/pet.conf:101`), type 3 always because `homunculus_autofeed_always` defaults yes (`conf/battle/homunc.conf:71`; if no, only sent when on).

### `0x0b8d` reputation list
Gate `PACKETVER_MAIN_NUM >= 20220330`: TRUE. One entry per `reputation_db` row (`db/re/reputation.yml`: ids 1 Orc Village, 2 Goblin Village, 3 Grey Wolf Village, 4 Isgard), `points` = the player's permanent variable `RepPoints*` (0 fresh). `success` always 1. Fresh novice = 5 + 4x16 = 69 bytes: `8d 0b 45 00 01` then 4 entries with points 0. Capture entry order is 4, 3, ... (`reputation_db` iteration order, not sorted); the client does not care. Sent only in the `changemap` block.

### `0x00be` f7..fc
`statusID` 247..252 = `SP_UPOW, SP_USTA, SP_UWIS, SP_USPL, SP_UCON, SP_UCRT` (`map.hpp:533`): "points needed to raise this 4th-job trait stat by one" for POW/STA/WIS/SPL/CON/CRT (the `SP_U*` counterpart of the STR..LUK cost fields). value = `pc_need_trait_point(sd, stat, 1)` as u8 (0 in the capture). Gated `PACKETVER_MAIN_NUM >= 20200916` (TRUE). Same function also emits `SP_USTR..SP_ULUK` the same way (0x00be 0x0020..0x0025 range), which the base-stat flow already covers through `0x00bd`.

### `0x0447` / `0x0b1b`
`CZ_BLOCKING_PLAY_CANCEL` is the client's "loading screen finished" signal (the client sends it ~300 ms after the first `0x007d`/`0x0360`, capture line 103). rAthena answers by calling `clif_loadConfirm`: if `PACKETVER_MAIN_NUM >= 20190403` (TRUE) it sends `0x0b1b` (2 bytes, opcode only), then `clif_instance_info` (memorial-dungeon info for own/party instance; nothing if none), then if the map has `MF_SPECIALPOPUP > 0` sends `clif_specialpopup`. The same `0x0b1b` is also sent at the end of `clif_refresh` (`@refresh`, `clif.cpp:9911`). It is the only server reply to `0x0447`; without it the client stays in the "account settings" load state (metaling capture `metaling-20261024.txt`). `0x0447` has no field; client opcode not shuffled (`clif_shuffle.hpp` has no entry; `clif_packetdb.hpp:1244/1607` apply).

## Brief entries for the rest of the burst
| Opcode | Name | Frame | One-line purpose |
| :--- | :--- | :--- | :--- |
| `0x09e7` | `ZC_NOTIFY_UNREADMAIL` (`rodexicon`, `packets_struct.hpp:253,1865`) | 3 | `result:u8`, 1 if unread/unchecked RODEX mail (`clif_Mail_new`, `clif.cpp:16029`); 0 fresh |
| `0x0a24` | `ZC_ACH_UPDATE` | 66 | `total_score:u32, level:u16, exp:u32, exp_tnl:u32, ach_id:u32, completed:u8, count[10]:u32, completed_time:u32, rewarded:u8`; zero achievement part when `ach` is null (`clif.cpp:21862-21885`); capture: score 10, level 0, exp 10, tnl 18 |
| `0x0a23` | `ZC_ALL_ACH_LIST` | 22 + 50*n | `len:u16, count:u32, total_score:u32, level:u16, exp:u32, exp_tnl:u32, n x {id:u32, completed:u8, count[10]:u32, completed_time:u32, rewarded:u8}`; nothing sent when n = 0 (`clif.cpp:21820-21858`); capture n = 2 (122 bytes) |
| `0x0ade` | `ZC_OVERWEIGHT_PERCENT` | 6 | `u32 natural_heal_weight_rate` (70), `clif_weight_limit`, from `pc_scdata_received` (`pc.cpp:14794`); gate `PACKETVER >= 20171025` |
| `0x0141` | `ZC_COUPLESTATUS` | 14 | `type:u32 (SP_*), base:i32, bonus:i32` (STR..LUK = 13..18, POW..CRT = 219..224); [status.md](./status.md#sp_-var-ids) |
| `0x013a` | `ZC_ATTACK_RANGE` | 4 | `range:u16` (1 for a bare-handed novice) |
| `0x0b1c` | `CZ_PING_LIVE` (C to Map) | 2 | client keep-alive; handler is `clif_parse_dull`, no reply (`clif_packetdb.hpp:1945`) |
| `0x0b1d` | `ZC_PING_LIVE` | 2 | server keep-alive, sent per `ping_timer_interval` (`clif.cpp:22479-22492`), gate `>= 20190213`; not part of the login burst |
| `0x0acb` | `ZC_LONGLONGPAR_CHANGE` | 12 | `var:u16, value:i64`; used for SP_BASEEXP/NEXTBASEEXP/JOBEXP/NEXTJOBEXP (`PACKETVER >= 20170830`) |

## Send conditions and order

Gates inside `clif_parse_LoadEndAck` (`clif.cpp:10748-11183`):
* `connect_new` is set by `pc_authok` (`pc.cpp:2119`, first map-server entry of this session) and cleared at the end of `LoadEndAck` (`clif.cpp:11182`). Block A below runs only when `connect_new`.
* `changemap` is set by `pc_setpos` (`pc.cpp:6951`) when the new `mapindex` differs from the current one, which is also true at login. Block B runs when `changemap` (login AND every cross-map warp; not same-map warps).
* `0x0ade`, `0x09e7`, `0x0a23`, `0x0a24` and the stat dump before `0x007d` come from `pc_authok`/`pc_scdata_received`/achievement init, before the client's `0x007d`.

Order after `0x007d` (first login of a fresh novice, matches capture lines 63-98 once re-framed):
1. `0x01d7` look, `0x0b08`/`0x0b09`/`0x0b39`/`0x0b0b`/`0x0a9b` inventory, weight/max weight `0x00b0`, spawn entries, map property `0x099b`, area unit entries ([inventory.md](./inventory.md), [map.md](./map.md)).
2. Block A (`connect_new` only):
   1. `0x010f` skill list
   2. `0x0b20` tab 0, then `0x0b20` tab 1 (second send gated `PACKETVER_MAIN_NUM >= 20190522`)
   3. `0x0acb` x4 (base exp, next base exp, job exp, next job exp), `0x00b0` SP_SKILLPOINT
   4. `clif_initialstatus`: `0x00bd`; `0x0141` x6 (STR..LUK); `0x013a`; `0x00b0` ASPD; then renewal block: `0x0141` x6 (POW..CRT), `0x00b0` x6 (PATK, SMATK, RES, MRES, HPLUS, CRATE), `0x00b0` x3 (TRAITPOINT, AP, MAXAP), `0x00be` x6 (SP_UPOW..SP_UCRT = f7..fc)
   5. status-load icons for falcon/riding, SC_NOCHAT, friend-online notices, `NPCE_LOGIN` script event
3. Block B (`changemap`): `0x02c9` party config; `0x02da` equip checkbox (gate `PACKETVER >= 20070918`); `0x02d9` type 2 (pet autofeed, `>= 20141008`); `0x02d9` type 1; `0x02d9` type 3 (`>= 20170920`); `0x02d9` type 5 (`>= 20230419`); `0x0b8d` reputation list. Then guild notice, map-dependent effects, `pc_calc` etc.
4. Tail: mail icon `0x09e7` re-send only if unread (`>= 20150513`), `0x007f` is the answer to the client's `0x0360`, not part of this flow.

Defaults for a fresh novice (no party, no pet, no homunculus, nothing toggled), hex incl. opcode:
* `0x02c9`: `c9 02 00`. `0x02da`: `da 02 00`.
* `0x02d9` x4: `d9 02 02 00 00 00 00 00 00 00`, same with type `01`, `03`, `05` (values 0).
* `0x0b8d`: `8d 0b 45 00 01` + 4 x (u64 id, u64 0).
* `0x00be` x6: `be 00 f7 00 00` ... `be 00 fc 00 00`.
* `0x0b20` x2: `20 0b 00 00 00` / `20 0b 00 01 00` + 266 zero bytes each (rotate 0 unless previously changed).
* Client then sends `0x0447` and gets `0x0b1b`: `1b 0b`.

Open items: the exact byte contents of the two `0x0b20` frames and of `0x010f` upFlag are inferred (hotkey bytes are not visible in the capture because the proxy swallowed them); the arithmetic of frame sizes confirms 271 and 41. Re-capture after fixing `table.rs` to verify.
