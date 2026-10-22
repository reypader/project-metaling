# Project Metaling Reference Knowledge Catalog Index

This master index maps extracted protocol opcodes, bit-packing algorithms, Aegis mechanics, script commands, and database schemas to their curated specification files under `./reference/catalog/`. All packet specifications target `PACKETVER = 20260219`.

## 1. Packets & Framing (`./packets/`)

| Identifier / Opcode | Canonical Name | Direction | Frame Length | Target File & Section |
| :--- | :--- | :--- | :--- | :--- |
| Policy | Client-Initiated TCP Socket Lifecycle | Bidirectional | N/A | [packets/login.md#tcp-socket-lifecycle-and-handoff-policy](./packets/login.md#tcp-socket-lifecycle-and-handoff-policy) |
| Policy | Raw 4-Byte AccountId Echo | Server to Client | Fixed 4 bytes | [packets/char.md#raw-4-byte-accountid-echo-policy](./packets/char.md#raw-4-byte-accountid-echo-policy) |
| Policy | `clif_shuffle.hpp` Precedence Rules | Client to Server | N/A | [packets/map.md#packet-shuffling-precedence-and-macro-offset-rules](./packets/map.md#packet-shuffling-precedence-and-macro-offset-rules) |
| `0x007d` | `CZ_NOTIFY_ACTORINIT` | Client to Map | Fixed 2 bytes | [packets/map.md#0x007d-cz_notify_actorinit](./packets/map.md#0x007d-cz_notify_actorinit) |
| `0x0091` | `ZC_NPCACK_MAPMOVE` | Map to Client | Fixed 22 bytes | [packets/map.md#0x0091-zc_npcack_mapmove](./packets/map.md#0x0091-zc_npcack_mapmove) |
| `0x0283` | `ZC_AID` | Map to Client | Fixed 6 bytes | [packets/map.md#0x0283-zc_aid](./packets/map.md#0x0283-zc_aid) |
| `0x02eb` | `ZC_ACCEPT_ENTER` | Map to Client | Fixed 13 bytes | [packets/map.md#0x02eb-zc_accept_enter](./packets/map.md#0x02eb-zc_accept_enter) |
| `0x035f` | `CZ_REQUEST_MOVE` | Client to Map | Fixed 5 bytes | [packets/map.md#0x035f-cz_request_move](./packets/map.md#0x035f-cz_request_move) |
| `0x0360` | `CZ_REQUEST_TIME` | Client to Map | Fixed 6 bytes | [packets/map.md#0x0360-cz_request_time](./packets/map.md#0x0360-cz_request_time) |
| `0x0361` | `CZ_CHANGE_DIRECTION` | Client to Map | Fixed 5 bytes | [packets/map.md#0x0361-cz_change_direction](./packets/map.md#0x0361-cz_change_direction) |
| `0x0368` | `CZ_REQNAME` | Client to Map | Fixed 6 bytes | [packets/map.md#0x0368-cz_reqname](./packets/map.md#0x0368-cz_reqname) |
| `0x0436` | `CZ_ENTER` | Client to Map | Fixed 23 bytes | [packets/map.md#0x0436-cz_enter](./packets/map.md#0x0436-cz_enter) |
| `0x0437` | `CZ_REQUEST_ACT` | Client to Map | Fixed 7 bytes | [packets/map.md#0x0437-cz_request_act](./packets/map.md#0x0437-cz_request_act) |
| Policy | Player wire identifiers (`bl.id` = account_id; char_id only in `GID` of 0x09fx) | Map to Client | N/A | [packets/map.md#player-wire-identifiers-policy](./packets/map.md#player-wire-identifiers-policy) |
| `0x0080` | `ZC_NOTIFY_VANISH` | Map to Client | Fixed 7 bytes | [packets/map_units.md#0x0080-zc_notify_vanish](./packets/map_units.md#0x0080-zc_notify_vanish) |
| `0x0086` | `ZC_NOTIFY_MOVE` (unused; observers get 0x09fd) | Map to Client | n/a (16 if used) | [packets/map_units.md#0x0086-zc_notify_move-unused](./packets/map_units.md#0x0086-zc_notify_move-unused) |
| `0x0087` | `ZC_NOTIFY_PLAYERMOVE` | Map to Client | Fixed 12 bytes | [packets/map_units.md#0x0087-zc_notify_playermove](./packets/map_units.md#0x0087-zc_notify_playermove) |
| `0x009c` | `ZC_CHANGE_DIRECTION` | Map to Client | Fixed 9 bytes | [packets/map_units.md#0x009c-zc_change_direction](./packets/map_units.md#0x009c-zc_change_direction) |
| `0x00b0` | `ZC_PAR_CHANGE` | Map to Client | Fixed 8 bytes | [packets/map_units.md#0x00b0-zc_par_change](./packets/map_units.md#0x00b0-zc_par_change) |
| `0x08c8` | `ZC_NOTIFY_ACT` (sit/stand/damage) | Map to Client | Fixed 34 bytes | [packets/map_units.md#0x08c8-zc_notify_act-aka-zc_notify_act3](./packets/map_units.md#0x08c8-zc_notify_act-aka-zc_notify_act3) |
| `0x09fd` | `ZC_NOTIFY_MOVEENTRY11` (walking unit) | Map to Client | Dynamic (114) | [packets/map_units.md#0x09fd-zc_notify_moveentry11-walking-unit-new-or-changed-walk](./packets/map_units.md#0x09fd-zc_notify_moveentry11-walking-unit-new-or-changed-walk) |
| `0x09fe` | `ZC_NOTIFY_NEWENTRY11` (spawn) | Map to Client | Dynamic (107) | [packets/map_units.md#0x09fe-zc_notify_newentry11-unit-spawns-in-view](./packets/map_units.md#0x09fe-zc_notify_newentry11-unit-spawns-in-view) |
| `0x09ff` | `ZC_NOTIFY_STANDENTRY11` (idle unit) | Map to Client | Dynamic (108) | [packets/map_units.md#0x09ff-zc_notify_standentry11-idle-unit-comes-into-view](./packets/map_units.md#0x09ff-zc_notify_standentry11-idle-unit-comes-into-view) |
| `0x0a30` | `ZC_ACK_REQNAMEALL` (CZ_REQNAME reply, players) | Map to Client | Fixed 106 bytes | [packets/map_units.md#0x0a30-zc_ack_reqnameall-reply-to-cz_reqname-for-a-player](./packets/map_units.md#0x0a30-zc_ack_reqnameall-reply-to-cz_reqname-for-a-player) |
| `0x0af7` | `ZC_ACK_REQNAME_BYGID` (char_id lookup) | Map to Client | Fixed 32 bytes | [packets/map_units.md#0x0af7-zc_ack_reqname_bygid-not-the-cz_reqname-reply](./packets/map_units.md#0x0af7-zc_ack_reqname_bygid-not-the-cz_reqname-reply) |
| `0x0ac4` | `AC_ACCEPT_LOGIN` | Login to Client | Dynamic | [packets/login.md#0x0ac4-ac_accept_login](./packets/login.md#0x0ac4-ac_accept_login) |
| `0x0ac5` | `HC_NOTIFY_ZONESVR` | Char to Client | Fixed 156 bytes | [packets/char.md#0x0ac5-hc_notify_zonesvr](./packets/char.md#0x0ac5-hc_notify_zonesvr) |
| `0x0b18` | `ZC_INVENTORY_EXPANSION_INFO` | Map to Client | Fixed 4 bytes | [packets/map.md#0x0b18-zc_inventory_expansion_info](./packets/map.md#0x0b18-zc_inventory_expansion_info) |
| `WBUFPOS` | 3-Byte Coordinate & Direction Packing | Bidirectional | Fixed 3 bytes | [packets/bitpacking.md#wbufpos-3-byte-coordinate-and-direction-packing](./packets/bitpacking.md#wbufpos-3-byte-coordinate-and-direction-packing) |
| `WBUFPOS2` | 6-Byte Movement Pair Packing | Server to Client | Fixed 6 bytes | [packets/bitpacking.md#wbufpos2-6-byte-movement-pair-packing](./packets/bitpacking.md#wbufpos2-6-byte-movement-pair-packing) |

## 2. Mechanics & Lifecycles (`./mechanics/`)

| Topic / Identifier | Summary | Target File & Section |
| :--- | :--- | :--- |
| Derived Stats | Renewal base and derived stat formulas (`ATK`, `DEF`, `ASPD`, `HIT`, `FLEE`) | [mechanics/stats.md#derived-stat-formulas](./mechanics/stats.md#derived-stat-formulas) |
| Combat Formulas | Physical and magical damage calculation and modifier tables | [mechanics/combat.md#combat-damage-pipeline](./mechanics/combat.md#combat-damage-pipeline) |
| Pathfinding (A*) | `MAX_WALKPATH` 32, `max_walk_path` 17 reject rule, 10/14 costs, Manhattan heuristic, no corner cutting, last row/col blocked | [mechanics/movement_aoi.md#1-pathfinding-a](./mechanics/movement_aoi.md#1-pathfinding-a) |
| Step Timing | Walk speed 150 ms, diagonal x14/10, position updates at step end, start packets | [mechanics/movement_aoi.md#2-step-timing](./mechanics/movement_aoi.md#2-step-timing) |
| Mid-walk Redirect | `change_walk_target`, repath at end of in-flight step, click on own cell | [mechanics/movement_aoi.md#3-mid-walk-redirect-new-cz_request_move-while-walking](./mechanics/movement_aoi.md#3-mid-walk-redirect-new-cz_request_move-while-walking) |
| Area of Interest | `AREA_SIZE` 14, `BLOCK_SIZE` 8, square range, per-step outsight/insight strips, `CLR_OUTSIGHT` | [mechanics/movement_aoi.md#4-area-of-interest](./mechanics/movement_aoi.md#4-area-of-interest) |
| Map Entry (`LoadEndAck`) | Order of ops, `map_addblock`, spawn to others, nearby to self, self packets (no `VAR_SPEED`) | [mechanics/map_entry_actions.md#map-entry-cz_notify_actorinit-clif_parse_loadendack](./mechanics/map_entry_actions.md#map-entry-cz_notify_actorinit-clif_parse_loadendack) |
| Sit / Stand | `ZC_NOTIFY_ACT` type 2/3 to `AREA` incl. self, stop-walk first, idle-entry state=2 | [mechanics/map_entry_actions.md#sit--stand-cz_request_act-action-2--3](./mechanics/map_entry_actions.md#sit--stand-cz_request_act-action-2--3) |
| Direction / Name / Time | `ZC_CHANGE_DIRECTION` to `AREA_WOS`, `CZ_REQNAME` range 14 self-only reply, `ZC_NOTIFY_TIME` = server tick | [mechanics/map_entry_actions.md#direction-change-cz_change_direction](./mechanics/map_entry_actions.md#direction-change-cz_change_direction) |
| Status Effects | Status change lifecycles, timers, and stat modifiers | [mechanics/status.md#status-change-lifecycle](./mechanics/status.md#status-change-lifecycle) |

## 3. Scripting VM (`./scripting/`)

| Topic / Command | Summary | Target File & Section |
| :--- | :--- | :--- |
| NPC Dialog Commands | Non-blocking dialog suspension and navigation builtins (`mes`, `next`, `close`, `select`, `warp`) | [scripting/npc.md#npc-dialog-and-navigation-builtins](./scripting/npc.md#npc-dialog-and-navigation-builtins) |
| Skill Script Hooks | Skill execution hooks and ratio calculation bindings | [scripting/skills.md#skill-script-hooks](./scripting/skills.md#skill-script-hooks) |
| Item Bonus Scripts | Equipment and consumable item script evaluation rules | [scripting/items.md#item-bonus-script-evaluation](./scripting/items.md#item-bonus-script-evaluation) |

## 4. Databases & Map Formats (`./db/`)

| Schema / Format | Summary | Target File & Section |
| :--- | :--- | :--- |
| Renewal YAML Schemas | Static database schemas (`item_db.yml`, `mob_db.yml`, `skill_db.yml`, `job_stats.yml`) | [db/yaml_schemas.md#renewal-yaml-database-schemas](./db/yaml_schemas.md#renewal-yaml-database-schemas) |
| Relational SQL: `login` | Account credentials, administrative groups, and inter-server authentication seed | [db/sql_schemas.md#login-table-schema](./db/sql_schemas.md#login-table-schema) |
| Relational SQL: `char` | Character slots, base and trait attributes, vitals, appearance, and coordinates | [db/sql_schemas.md#char-table-schema](./db/sql_schemas.md#char-table-schema) |
| `.gat` / `.rsw` & Spawns | Map cell walkability grid binary layout and mob/warp script definitions | [db/map_gat.md#map-walkability-and-spawn-formats](./db/map_gat.md#map-walkability-and-spawn-formats) |
| `.gat` Cell Types | Raw type 0-6 to walkable/shootable/water, unknown = wall, water promotion, last row/col quirk | [db/map_gat.md#gat-cell-type-mapping](./db/map_gat.md#gat-cell-type-mapping) |
