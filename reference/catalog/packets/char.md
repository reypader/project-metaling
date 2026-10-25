# Character Server Packets (`CH_*` / `HC_*`)

Target client version: `PACKETVER = 20260219`. Packet entries are sorted by ascending hexadecimal opcode.

## Raw 4-Byte `AccountId` Echo Policy

* **Provenance:** Workspace Architecture Policy (`./AGENTS.md`, Section 5) & `rathena` (`src/char/char_clif.cpp`, `chclif_parse_reqtoconnect`, `PACKETVER = 20260219`).
* **Handshake Framing:** Immediately after accepting `CH_ENTER` (`0x0065`), the character server writes the 4-byte little-endian `AccountId` directly to the TCP stream without a 2-byte packet opcode header.
* **Client Framing Behavior:** The official client reads these raw 4 bytes as an out-of-band stream prefix before switching back to standard 2-byte opcode framing for `HC_ACCEPT_ENTER` and character slot page packets.
* **Socket Handoff Policy:** Upon sending `HC_NOTIFY_ZONESVR` (`0x0ac5`), the character server keeps the TCP connection open until the client closes the socket (`Ok(0)` EOF).

## Packet Specifications

### `0x0ac5` `HC_NOTIFY_ZONESVR`

* **Opcode:** `0x0ac5`
* **Canonical Name:** `HC_NOTIFY_ZONESVR`
* **Direction:** Server to Client (Character Server)
* **Frame Length:** Fixed `156` bytes
* **Provenance:** `rathena` (`src/char/packets.hpp`, `src/char/char_clif.cpp`), `PACKETVER = 20260219` (`PACKETVER >= 20170315`).

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x0ac5` (little-endian) |
| `2` | `4` | `u32` | `char_id` | Selected character ID (`CID`) |
| `6` | `16` | `[u8; 16]` | `map_name` | Null-terminated `.gat` map name (for example, `prontera.gat`) |
| `22` | `4` | `u32` | `ip` | Map server IPv4 address in network byte order |
| `26` | `2` | `u16` | `port` | Map server TCP port in little-endian byte order |
| `28` | `128` | `[u8; 128]` | `domain` | Zero-padded map server domain string |

## Per-Character Look Fields (`CHARACTER_INFO`, `0x006b` / `0x0b72` / `0x0b6f`)

* **Provenance:** `rathena` @ `d4b8e7b8f160` (`$RATHENA_PATH`): struct `src/common/packets.hpp:31-117`, builder `char_mmo_char_tobuf` `src/char/char.cpp:1774-1870`. Resolved for `PACKETVER = 20260219` (applies; `PACKETVER_MAIN_NUM >= 20220330` so hp/sp are 64-bit, `PACKETVER_RE` undefined).
* **Entry size:** `175` bytes. Cross-check: `0x0b72` = `4 + n*175` (879 for 5 chars) and `0x006b` = `27 + n*175` (902 for 5) in `captures/login_sequence-rathena.txt` and `captures/metaling-20261024-3.txt`. `0x006b` header: opcode, length, `total u8`, `premium_start u8`, `premium_end u8`, `extension[20]`, then entries (`packets.hpp:~245-258`). `0x0b6f` (`HC_ACCEPT_MAKECHAR`) carries one entry after the opcode; `0x0b72` is opcode, length, entries (`packets.hpp:~255-281, 610-620`).

| Offset | Size | Field | Notes |
| :--- | :--- | :--- | :--- |
| 0 | 4 | `GID` | char id |
| 4 | 8 | `exp` (i64) | |
| 12 | 4 | `money` | |
| 16 | 8 | `jobexp` (i64) | |
| 24 | 4 | `joblevel` | |
| 28 / 32 | 4 / 4 | `bodystate` / `healthstate` | both always 0 |
| 36 | 4 | `effectstate` | `option` |
| 40 / 44 | 4 / 4 | `virtue` / `honor` | karma / manner |
| 48 | 2 | `jobpoint` | status points, capped at i16 max |
| 50 / 58 / 66 / 74 | 8 each | `hp`, `maxhp`, `sp`, `maxsp` (i64) | |
| 82 | 2 | `speed` | always `DEFAULT_WALK_SPEED` |
| 84 | 2 | `job` | class id |
| 86 | 2 | `head` | hair style |
| 88 | 2 | `body` | `status.body` (>= 20231220). **Captured: always equals `job`** (Novice 0, Archer 3, Assassin 12, Bard 19; captures `equip_sequence_2..4.txt`). The client draws the body sprite from it, so 0 shows a Novice body while the job name says otherwise. |
| 90 | 2 | `weapon` (**i16**) | see below |
| 92 | 2 | `level` | base level |
| 94 | 2 | `sppoint` | skill points |
| 96 | 2 | `accessory` | `status.head_bottom` |
| 98 | 2 | `shield` (**i16**) | `status.shield` |
| 100 | 2 | `accessory2` | `status.head_top` |
| 102 | 2 | `accessory3` | `status.head_mid` |
| 104 | 2 | `headpalette` | `status.hair_color` |
| 106 | 2 | `bodypalette` | `status.clothes_color` |
| 108 | 24 | `name` | |
| 132 | 6 | Str Agi Vit Int Dex Luk | u8 each |
| 138 / 139 | 1 / 1 | `CharNum` / `hairColor` | slot / hair color capped at u8 |
| 140 | 2 | `bIsChangedCharName` | 0 if rename pending (`rename > 0`), else 1 |
| 142 | 16 | `mapName` | last map |
| 158 | 4 | `DelRevDate` | seconds until deletion (`PACKETVER_CHAR_DELETEDATE`) else raw `delete_date` |
| 162 | 4 | `robePalette` | **receives `status.robe`** (the garment view id, not a palette) |
| 166 | 4 | `chr_slot_changeCnt` | |
| 170 | 4 | `chr_name_changeCnt` | |
| 174 | 1 | `sex` | |

* **`weapon` (offset 90):** `p.weapon` straight from the `char` table column, i.e. `status.weapon`, which is the **weapon type** (`W_*`, or a dual-wield constant), not a sprite or item id. It is forced to `0` when `option` has any of `0x20 | 0x80000 | 0x100000 | 0x200000 | 0x400000 | 0x800000 | 0x1000000 | 0x2000000 | 0x4000000 | 0x8000000` (riding/dragon/wug/mado-type options); the source comment says the client crashes at login otherwise (`char.cpp:1817-1818`).
* **`shield` (98):** `status.shield` as stored: the **shield view** (`item.look` of the worn armor-type left-hand item) or `0`. A left-hand *weapon* stores `W_FIST` (= 0) here (`pc.cpp:12181-12196`).
* **`accessory` / `accessory2` / `accessory3` (96/100/102):** head bottom / top / mid **view ids** (`status.head_*`, i.e. the item `look`), already costume-resolved when saved. **`robePalette` (162):** `status.robe` view id.
* **`bodypalette` (106):** `clothes_color`; `pc_makesavestatus` zeroes it when `save_clothcolor` is off, and zeroes `status.body` when `save_body_style` is off (`pc.cpp:1352-1359`). `body` (88) is the full i16 `status.body`.
* **Widths:** weapon/shield are 16-bit here even though the map entry packets use 32-bit at this packetver. Item ids above 32767 would not fit; the char-select field is a type/view, not an item id.
* **Character-select weapon sprite (settled 2026-10-25):** rAthena sends the `W_*` type here, never an item id. [`captures/equip_sequence_2.txt`](../captures/equip_sequence_2.txt) lines 1157 and 1309: a Knife gives `weapon` = 1 (`W_DAGGER`); Goggles (top + mid) give `accessory2` = 1 and `accessory3` = 0; a Bravery Bag gives `robePalette` = 2. The client doesn't draw the weapon or shield on the select screen; it draws them only in combat stance (user report), so a missing weapon there is expected. Headgear does show. More in [equipment.md](./equipment.md#char-select-capture-2-lines-1157-1309).

### Where the `char` table columns come from and when they are written

* Columns `weapon`, `shield`, `head_top`, `head_mid`, `head_bottom`, `robe` (`smallint unsigned`) are written by the char-server UPDATE in `char_mmo_char_tosql` (`char.cpp:~280-330`, columns at 310 and 323), only when the incoming `mmo_charstatus` differs from the cached copy in any compared field (the condition includes each of these six, `char.cpp:293-296`).
* Source values are the map-server `sd->status.weapon/shield/head_top/head_mid/head_bottom/robe` copied whole in the `0x2b01` char-save packet by `chrif_save` (`chrif.cpp:277-320`, which first runs `pc_makesavestatus`; none of the six is altered there).
* Triggers: `chrif_save` runs on logout/map-server change/disconnect (`CSAVE_QUIT`, `map.cpp:2329,5113`), the periodic autosave (`pc_autosave`, `pc.cpp:13002-13040`, one player per call, interval `autosave_time` 300 s divided by users+1, floored at `minsave_time` 100 ms; each call also saves inventory and cart), plus the `save_settings` events (trade, vending, storage, pet, mail, auction, quest, bank, attendance), job change (`pc.cpp:11076`), inventory expansion (`clif.cpp:23183`), and a few others.
* **An equip or unequip does not trigger a save.** `pc_equipitem` (`pc.cpp:12015`) and `pc_unequipitem` (`pc.cpp:12398`) contain no `chrif_save`; the new `inventory.equip` mask and the six columns reach SQL at the next autosave or logout. A crash in between loses the equip change (the char row and the inventory row are saved in the same `chrif_save` call, so they stay consistent with each other).

### Map-server values: `status.*` versus `vd.look[]`

| Value | Holds | Set by |
| :--- | :--- | :--- |
| `status.weapon` | weapon type `W_*` (right/left combined, dual constants) | `pc_calcweapontype` `pc.cpp:1520-1556`, called by `pc_setequipindex` at login and by equip/unequip |
| `status.shield` | shield `look` (armor in left hand) or `W_FIST`=0 (weapon in left hand or empty) | `pc_equipitem` `pc.cpp:12181-12196`, `pc_unequipitem` `12443-12448` |
| `status.head_bottom/mid/top`, `status.robe` | headgear/garment `look` (view ids), multi-slot items kept on the highest slot only, costume overrides | `pc_set_costume_view` `pc.cpp:15490-15540` (called on equip/unequip, `12229`, `12455`, and at login) |
| `vd.look[LOOK_WEAPON]` | right-hand item `view_id`, else `nameid`; 0 if empty or `OPTION_COSTUME` | `map_session_data::update_look` `pc.cpp:623-680` |
| `vd.look[LOOK_SHIELD]` | left-hand item `view_id`, else `nameid`; 0 if empty, `OPTION_COSTUME`, or same slot as the right hand (two-hander) | same |
| `vd.look[LOOK_HEAD_*]`, `[LOOK_ROBE]` | copied from `status.head_*` / `status.robe` | `status_set_viewdata` `status.cpp:9423-9435` |

* **Entry packets** (`clif_set_unit_idle`, `clif.cpp:1041-1170`) read everything from `vd->look[]`: `weapon` = `LOOK_WEAPON`, `shield` = `LOOK_SHIELD` (>= 20181121), `accessory` = `LOOK_HEAD_BOTTOM`, `accessory2` = `LOOK_HEAD_TOP`, `accessory3` = `LOOK_HEAD_MID`, `robe` = `LOOK_ROBE`. So the map entry packet sends the item id (alias if any) for weapon/shield and view ids for headgear/robe, never the `W_*` type; this matches the capture that shows a worn Knife as `b1 04 00 00` (1201 = `nameid`). The same `LOOK_WEAPON`/`LOOK_SHIELD` pair is sent by `0x01d7` look changes (`clif_changelook` refreshes both from `update_look`, `clif.cpp:3965-3990`).
* **Consequence for Metaling:** the `char` table `weapon` column should hold the `W_*` type (it is what `0x006b`/`0x0b72` emit and what `pc_calcweapontype` derives), not an item id. On login rAthena recomputes it from the equipped inventory rows (`pc_setequipindex`, `intif.cpp:3486`), so the column is only authoritative for the select screen. Note the catalog's `db/sql_schemas.md` comment "Equipped weapon view ID" for `weapon` is inaccurate: it is the type.
