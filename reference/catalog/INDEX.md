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
| Map Handshake & AoI | Map entry sequence, walking ticks, sitting transitions, and Area of Interest ranges | [mechanics/movement_aoi.md#map-entry-and-movement-lifecycle](./mechanics/movement_aoi.md#map-entry-and-movement-lifecycle) |
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
| Relational SQL Schemas | Account, character, and inventory table layouts for PostgreSQL | [db/sql_schemas.md#relational-persistence-schemas](./db/sql_schemas.md#relational-persistence-schemas) |
| `.gat` / `.rsw` & Spawns | Map cell walkability grid binary layout and mob/warp script definitions | [db/map_gat.md#map-walkability-and-spawn-formats](./db/map_gat.md#map-walkability-and-spawn-formats) |
