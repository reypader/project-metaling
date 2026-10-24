# Status-change icon packets (`clif_status_change`), PACKETVER 20260219

Provenance: `rathena` @ `d4b8e7b8f`, `src/map/clif.cpp:6472-6560` (`clif_status_change_sub`, `clif_status_change`), `clif_packetdb.hpp:182,1157,1572` (lengths), `status.hpp:1456-2143` (EFST enum), `status.cpp:1045-1049`, `battle.cpp:8636` + `conf/battle/client.conf:108` (`display_status_timers`). Applies to `20260219`: yes (`PACKETVER >= 20120618` branch). Server-to-client only, so `clif_shuffle.hpp` does not apply.

## Packet choice

`clif_status_change(bl, type, flag, tick, val1, val2, val3)` builds one of:

| Condition | Opcode | Name | Len |
| :--- | :--- | :--- | :--- |
| `flag != 0` and `display_status_timers` (default 1, `client.conf: yes`) | `0x0983` | `ZC_MSG_STATE_CHANGE3` | 29 |
| otherwise (always when `flag == 0`) | `0x0196` | `ZC_MSG_STATE_CHANGE` | 9 |

(`0x043f` is the `>= 20090121 < 20120618` variant; not used at 20260219.)

### `0x0983` (29 bytes)

| Offset | Size | Type | Field |
| :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | opcode `0x0983` |
| 2 | 2 | u16 | `type` (EFST id) |
| 4 | 4 | u32 | target GID (`bl->id`) |
| 8 | 1 | u8 | `state` (the `flag` argument: 1 = active) |
| 9 | 4 | u32 | total duration, ms |
| 13 | 4 | u32 | remaining duration, ms (rAthena writes the same value as total) |
| 17 | 4 | u32 | `val1` |
| 21 | 4 | u32 | `val2` |
| 25 | 4 | u32 | `val3` |

Duration: if `tick <= 0` (includes `INFINITE_TICK = -1`, `timer.hpp:21`) it is replaced by `9999`; the value is truncated to u32 (`client_tick`).

### `0x0196` (9 bytes): status end (or timers disabled)

`u16 opcode 0x0196; u16 type (EFST id); u32 target GID; u8 flag` (flag 0 = end, 1 = start without timer fields).

## Send target

`clif_status_change` sends with `AREA` for a visible PC (`SELF` if the PC is invisible, `AREA_WOS` for non-PC), so for a player it reaches the player AND every observer in view. It is dropped when the EFST's relevant-BL-type mask excludes the unit type; the default mask is `BL_PC` (`status.cpp:16370`), so a PC always passes. `EFST_BLANK (-1)` and `EFST_POSTDELAY` with tick 0 are never sent.

## `EFST_CLIENT_ONLY_EQUIP_ARROW` = 695

Computed from the enum: `EFST_PROVOKE` is 0 (after `EFST_BLANK = -1`), the only explicit assignment is `EFST_SILVERVEIN_RUSH_POSTDELAY = 596` (`status.hpp`), and `EFST_CLIENT_ONLY_EQUIP_ARROW` is declared at `status.hpp:2143`. Counting from there gives 695. Not verified against a capture; if the icon does not appear, check this value first.

Ammo equip call (`clif_arrowequip`, `clif.cpp:4204-4217`): `clif_status_change(&sd, EFST_CLIENT_ONLY_EQUIP_ARROW, 1, INFINITE_TICK, 0, 0, 0)`. So the wire packet is `0x0983`, `state = 1`, total = remain = 9999, `val1 = val2 = val3 = 0` (the "1" is the state byte, not val1). Immediately followed by `0x013c`.

* Sent from exactly two places: ammo equip (`pc.cpp:12164`) and `clif_inventorylist` when `equip_index[EQI_AMMO] >= 0` (`clif.cpp:3125-3126`, so at every login / `LoadEndAck` inventory send, between the normal list and the equip list). Both therefore emit `0x0983` + `0x013c`.
* **Nothing ever ends this EFST.** grep of `src/map` finds no other reference. Unequipping ammo (directly, because the weapon was removed/changed, or by consumption/drop of the whole stack) sends only `0x099a` (flag 0); no `0x0196`. Metaling can mirror that or send `0x0196 {695, GID, 0}` itself; rAthena does not.
* Because of `AREA`, observers also receive the `0x0983` for the ammo EFST of a player who equips ammo; it is harmless client-side but is a deviation to decide on.
