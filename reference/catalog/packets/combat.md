# Combat Packets (attack range, attack-distance failure, lock-on cancel)

Target client version: `PACKETVER = 20260219`. Citations are `$RATHENA_PATH/src/...`. The damage packet `ZC_NOTIFY_ACT` (`0x08c8`) lives in [map_units.md](./map_units.md#0x08c8-zc_notify_act-aka-zc_notify_act3) with its attack-specific rules; the request `CZ_REQUEST_ACT` (`0x0437`) lives in [map.md](./map.md#0x0437-cz_request_act). Behaviour: [../mechanics/auto_attack.md](../mechanics/auto_attack.md). No proxy capture of any packet below exists; layouts are from rAthena structs only.

## `0x0139` `ZC_ATTACK_FAILURE_FOR_DISTANCE`

> Open findings: [PF-13](../../../PACKET_FINDINGS.md#pf-13).

* **Direction:** Map to Client. **Frame:** Fixed `16`. **Provenance:** `map/packets_struct.hpp:5419-5428` (`PACKET_ZC_ATTACK_FAILURE_FOR_DISTANCE`, packed, no `PACKETVER` branch), sender `clif_movetoattack` `map/clif.cpp:8174-8186`. Sent `SELF` only. Not in `clif_packetdb.hpp` (server-to-client, length from the struct).

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | `0x0139` |
| `2` | `4` | `u32` | `target_id` | target `bl.id` (mob id) |
| `6` | `2` | `i16` | `target_x` | target cell X at send time |
| `8` | `2` | `i16` | `target_y` | target cell Y |
| `10` | `2` | `i16` | `x` | player's current cell X (`sd.x`) |
| `12` | `2` | `i16` | `y` | player's current cell Y |
| `14` | `2` | `i16` | `attack_range` | player's weapon range (`battle_status.rhw.range`, minimum 1) |

* **When:** the attack timer fires and the player fails `check_distance_client_bl(src, target, range)` (`unit.cpp:3261-3263`). It is a request to the client to walk to the target; the server does not walk the player. The attack loop ends after sending it (no re-arm); the client must send `CZ_REQUEST_MOVE` and a fresh `CZ_REQUEST_ACT`. Range semantics: [auto_attack.md section 2](../mechanics/auto_attack.md#2-range-check-semantics).

## `0x013a` `ZC_ATTACK_RANGE`

* **Direction:** Map to Client. **Frame:** Fixed `4`. **Provenance:** `map/packets_struct.hpp:384-388` (`PACKET_ZC_ATTACK_RANGE`), sender `clif_attackrange` `map/clif.cpp:3599-3607`, reached from `clif_updatestatus(SP_ATTACKRANGE)` (`clif.cpp:3777-3779`); also sent in `clif_initialstatus` on the first login (see [map_entry_actions.md](../mechanics/map_entry_actions.md#map-entry-cz_notify_actorinit-clif_parse_loadendack) step 8).

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | `0x013a` |
| `2` | `2` | `u16` | `attack_range` | weapon range (`rhw.range`; 1 bare-handed or melee) |

* Sent `SELF` whenever the attack range stat changes (weapon equip/unequip, range-modifying skills). A client uses it to decide locally how close to walk before attacking; the server's authoritative check is separate. Wire value is the plain range (not range+1).

## `0x0118` `CZ_CANCEL_LOCKON` (stop chasing / attacking)

* **Direction:** Client to Map. **Frame:** Fixed `2`. **Provenance:** `map/clif_packetdb.hpp:115` (`parseable_packet(0x0118,2,clif_parse_StopAttack,0)`); not overridden by `clif_shuffle.hpp` for `PACKETVER > 20180307`, so the opcode is `0x0118` at `20260219` (opcode not re-verified against a client capture). Handler `clif_parse_StopAttack` `map/clif.cpp:12579-12585`: `unit_stop_attack(sd)` then `sd->ud.state.attack_continue = 0`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | `0x0118`; no payload |

* No reply packet. Server just clears target and attack timer. Sent by the client when the user cancels a locked-on attack (e.g. clicking the ground or pressing a stop key).

## Related: attack outcome packet

The reply to a swing is `ZC_NOTIFY_ACT` `0x08c8` (34 bytes) to `AREA` around the target, sent at swing time. `src_speed` = attacker `amotion`, `dmg_speed` = target `dmotion` (0 on miss), `type` 0 normal / 10 crit / 8 double attack / 11 lucky dodge / 13 double+crit. See [map_units.md](./map_units.md#0x08c8-zc_notify_act-aka-zc_notify_act3) and [../mechanics/combat.md section 6](../mechanics/combat.md#6-result-struct---zc_notify_act-mapping).
