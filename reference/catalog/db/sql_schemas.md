# Relational Persistence Schemas

Target client version: `PACKETVER = 20260219`.

## Relational Persistence Schemas

* **Provenance:** Workspace Architecture Policy (`./AGENTS.md`, Section 5) & `rathena` (`sql-files/main.sql`, lines 209 to 296 and 770 to 799), `PACKETVER = 20260219`.
* **Scope:** Defines relational persistence schemas for player authentication, inter-server daemon credentials, and character state. PostgreSQL migrations map these structures to strongly typed schemas in `crates/ro-persistence`.

## login Table Schema

The `login` table stores user credentials, authorization groups, account state flags, and inter-server communication accounts.

* **Provenance:** `rathena` (`sql-files/main.sql`, lines 770 to 794).
* **Primary Key:** `account_id` (`INT(11) UNSIGNED`).
* **Sequence Start:** `AUTO_INCREMENT = 2000000`. Player accounts begin at 2000000. Account identifiers below 2000000 are reserved for internal servers and administrative entities.

### login Table Columns

| Field | Upstream Type | Default / Constraints | Rust Contract Type | Purpose |
| :--- | :--- | :--- | :--- | :--- |
| `account_id` | `INT(11) UNSIGNED` | `NOT NULL AUTO_INCREMENT PRIMARY KEY` | `AccountId` (`u32`) | Unique account identifier. Player accounts start at 2000000. |
| `userid` | `VARCHAR(23)` | `NOT NULL DEFAULT ''` | `String` | Username for authentication. |
| `user_pass` | `VARCHAR(32)` | `NOT NULL DEFAULT ''` | `String` | Password or hash used to verify identity. |
| `sex` | `ENUM('M','F','S')` | `NOT NULL DEFAULT 'M'` | `Sex` (`enum`) | Account gender. `'M'` is Male, `'F'` is Female, and `'S'` is Server daemon. |
| `email` | `VARCHAR(39)` | `NOT NULL DEFAULT ''` | `String` | Account contact email address. |
| `group_id` | `TINYINT(3)` | `NOT NULL DEFAULT 0` | `i32` | GM level and permission group. |
| `state` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `AccountState` (`enum`) | Status flags indicating active, banned, or locked state. |
| `unban_time` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `Option<u64>` | UNIX timestamp marking when a ban expires. Zero indicates no ban. |
| `expiration_time` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `Option<u64>` | UNIX timestamp marking account expiration. Zero indicates no expiration. |
| `logincount` | `MEDIUMINT(9) UNSIGNED` | `NOT NULL DEFAULT 0` | `u32` | Total count of successful logins. |
| `lastlogin` | `DATETIME` | `DEFAULT NULL` | `Option<DateTime>` | Timestamp of the most recent login. |
| `last_ip` | `VARCHAR(100)` | `NOT NULL DEFAULT ''` | `String` | Remote IP address of the most recent session. |
| `character_slots` | `TINYINT(3) UNSIGNED` | `NOT NULL DEFAULT 0` | `u8` | Number of character slots allowed. Zero defaults to global configuration. |
| `pincode` | `VARCHAR(4)` | `NOT NULL DEFAULT ''` | `String` | Four digit security PIN for character selection. |
| `vip_time` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `Option<u64>` | UNIX timestamp when VIP status expires. Zero indicates no VIP subscription. |

### Default Seeded Inter-Server Credentials

* **Provenance:** `rathena` (`sql-files/main.sql`, line 799).
* **Seeded Record:** `account_id = 1`, `userid = 's1'`, `user_pass = 'p1'`, `sex = 'S'`, `email = 'athena@athena.com'`.
* **Usage:** Provides default credentials for trusted communication between `char-server`, `map-server`, and `login-server`. The `'S'` sex code designates a server connection.

## char Table Schema

The `char` table stores persistent character state including slot indexing, appearance, base attributes, Renewal 4th job traits, vitals, and map coordinates.

* **Provenance:** `rathena` (`sql-files/main.sql`, lines 209 to 296).
* **Primary Key:** `char_id` (`INT(11) UNSIGNED`).
* **Sequence Start:** `AUTO_INCREMENT = 150000`. Player characters start at 150000. Identifiers below 150000 are reserved.
* **Foreign Key:** `account_id` references `login.account_id` with cascading delete constraints.

### char Table Columns

| Field | Upstream Type | Default / Constraints | Rust Contract Type | Purpose |
| :--- | :--- | :--- | :--- | :--- |
| `char_id` | `INT(11) UNSIGNED` | `NOT NULL AUTO_INCREMENT PRIMARY KEY` | `CharId` (`u32`) | Unique character identifier starting at 150000. |
| `account_id` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `AccountId` (`u32`) | Owning account identifier. |
| `char_num` | `TINYINT(1)` | `NOT NULL DEFAULT 0` | `u8` | Slot position on the character selection screen (0 to 8). |
| `name` | `VARCHAR(30)` | `NOT NULL DEFAULT '' UNIQUE` | `String` | Character display name. Must be unique. |
| `class` | `SMALLINT(6) UNSIGNED` | `NOT NULL DEFAULT 0` | `u32` / `u16` | Job class ID. |
| `base_level` | `SMALLINT(6) UNSIGNED` | `NOT NULL DEFAULT 1` | `u32` | Character base level. |
| `job_level` | `SMALLINT(6) UNSIGNED` | `NOT NULL DEFAULT 1` | `u32` | Character job level. |
| `base_exp` | `BIGINT(20) UNSIGNED` | `NOT NULL DEFAULT 0` | `u64` | Current base experience points. |
| `job_exp` | `BIGINT(20) UNSIGNED` | `NOT NULL DEFAULT 0` | `u64` | Current job experience points. |
| `zeny` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `u32` | Current zeny currency balance. |
| `str` | `SMALLINT(4) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Base Strength attribute. |
| `agi` | `SMALLINT(4) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Base Agility attribute. |
| `vit` | `SMALLINT(4) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Base Vitality attribute. |
| `int` | `SMALLINT(4) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Base Intelligence attribute. |
| `dex` | `SMALLINT(4) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Base Dexterity attribute. |
| `luk` | `SMALLINT(4) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Base Luck attribute. |
| `pow` | `SMALLINT(4) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Renewal 4th job Power trait. |
| `sta` | `SMALLINT(4) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Renewal 4th job Stamina trait. |
| `wis` | `SMALLINT(4) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Renewal 4th job Wisdom trait. |
| `spl` | `SMALLINT(4) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Renewal 4th job Spell trait. |
| `con` | `SMALLINT(4) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Renewal 4th job Concentration trait. |
| `crt` | `SMALLINT(4) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Renewal 4th job Creative trait. |
| `hp` | `INT(11)` | `NOT NULL DEFAULT 0` | `i32` | Current hit points. |
| `max_hp` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `u32` | Maximum hit points pool. |
| `sp` | `INT(11)` | `NOT NULL DEFAULT 0` | `i32` | Current spell points. |
| `max_sp` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `u32` | Maximum spell points pool. |
| `ap` | `INT(11)` | `NOT NULL DEFAULT 0` | `i32` | Current activity points for 4th job abilities. |
| `max_ap` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `u32` | Maximum activity points pool. |
| `status_point` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `u32` | Unallocated base status points. |
| `skill_point` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `u32` | Unallocated skill points. |
| `trait_point` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `u32` | Unallocated 4th job trait points. |
| `hair` | `TINYINT(4)` | `NOT NULL DEFAULT 0` | `u16` | Hairstyle model index. |
| `hair_color` | `SMALLINT(5) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Hair color palette index. |
| `clothes_color` | `SMALLINT(5) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Clothes dye palette index. |
| `body` | `SMALLINT(5) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Body outfit variant index. |
| `weapon` | `SMALLINT(6) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Equipped weapon view ID. |
| `shield` | `SMALLINT(6) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Equipped shield view ID. |
| `head_top` | `SMALLINT(6) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Top headgear view ID. |
| `head_mid` | `SMALLINT(6) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Middle headgear view ID. |
| `head_bottom` | `SMALLINT(6) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Lower headgear view ID. |
| `robe` | `SMALLINT(6) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Garment view ID. |
| `last_map` | `VARCHAR(11)` | `NOT NULL DEFAULT ''` | `String` | Map identifier where the character logged out or transferred. |
| `last_x` | `SMALLINT(5) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Last recorded map cell X coordinate. |
| `last_y` | `SMALLINT(5) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Last recorded map cell Y coordinate. |
| `save_map` | `VARCHAR(11)` | `NOT NULL DEFAULT ''` | `String` | Respawn anchor map identifier. |
| `save_x` | `SMALLINT(5) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Respawn anchor map cell X coordinate. |
| `save_y` | `SMALLINT(5) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16` | Respawn anchor map cell Y coordinate. |
| `sex` | `ENUM('M','F')` | `NOT NULL DEFAULT 'M'` | `Sex` (`enum`) | Character gender. |
| `inventory_slots` | `SMALLINT(5) UNSIGNED` | `NOT NULL DEFAULT 100` | `u16` | Maximum inventory item capacity. Defaults to 100 slots. |

### Indexes and Key Constraints

* **Primary Key:** `PRIMARY KEY (char_id)`
* **Unique Constraints:** `UNIQUE KEY name_key (name)`
* **Lookup Indexes:** `KEY account_id (account_id)`, `KEY (account_id, char_num)`

## inventory Table Schema

* **Provenance:** `rathena` @ `d4b8e7b8f`, `sql-files/main.sql:716-752`; loader/saver `src/char/char.cpp:533-850` (`char_memitemdata_to_sql` / `char_memitemdata_from_sql`). Applies to `PACKETVER = 20260219` (`MAX_SLOTS = 4`, `MAX_ITEM_RDM_OPT = 5`). Engine is MyISAM upstream (no FKs, no transactions); Metaling uses PostgreSQL, so add a real FK to `char(char_id)` and use `ON DELETE CASCADE`.
* **Row meaning:** one row per occupied inventory slot (equipment, stackables, rentals). Order of slots is NOT stored: rows are loaded `ORDER BY id` and packed into slots `0..n-1`. Semantics: [mechanics/inventory.md](../mechanics/inventory.md#11-persistence). Similar tables with the same columns minus `favorite`/`equip_switch`: `cart_inventory` (`char_id`), `storage` (`account_id`), `guild_storage` (`guild_id`).

### inventory Table Columns

| Field | Upstream Type | Default / Constraints | Rust Contract Type | Purpose |
| :--- | :--- | :--- | :--- | :--- |
| `id` | `INT(11) UNSIGNED` | `NOT NULL AUTO_INCREMENT`, PRIMARY KEY | `u32` (row id) | Row id; defines load order. |
| `char_id` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `CharId` | Owner. |
| `nameid` | `INT(10) UNSIGNED` | `NOT NULL DEFAULT 0` | `u32` (`ItemId`) | Item id. |
| `amount` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `u16`/`i16` in memory (max 30000) | Stack size. |
| `equip` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `u32` bitmask | Worn positions (`equip_pos`). |
| `identify` | `SMALLINT(6)` | `NOT NULL DEFAULT 0` | `bool` | Identified flag. |
| `refine` | `TINYINT(3) UNSIGNED` | `NOT NULL DEFAULT 0` | `u8` | Refine level. |
| `attribute` | `TINYINT(4) UNSIGNED` | `NOT NULL DEFAULT 0` | `u8` | Non-zero = broken. |
| `card0`..`card3` | `INT(10) UNSIGNED` x4 | `NOT NULL DEFAULT 0` | `[u32; 4]` | Card / enchant ids; special markers `0x00FF` forged, `0x00FE` created, `0x0100` pet. |
| `option_id0`..`option_id4` | `SMALLINT(5)` x5 | `NOT NULL DEFAULT 0` | `i16` | Random option id. |
| `option_val0`..`option_val4` | `SMALLINT(5)` x5 | `NOT NULL DEFAULT 0` | `i16` | Random option value. |
| `option_parm0`..`option_parm4` | `TINYINT(3)` x5 | `NOT NULL DEFAULT 0` | `i8` | Random option parameter. |
| `expire_time` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `u32` unix time | Rental expiry; 0 = none. |
| `favorite` | `TINYINT(3) UNSIGNED` | `NOT NULL DEFAULT 0` | `bool` | Favorite tab flag. |
| `bound` | `TINYINT(3) UNSIGNED` | `NOT NULL DEFAULT 0` | `u8` (`bound_type` 0-4) | Account/guild/party/character bound. |
| `unique_id` | `BIGINT(20) UNSIGNED` | `NOT NULL DEFAULT 0` | `u64` | `(char_id << 32) | counter`; 0 for stackables without the UniqueId flag. |
| `equip_switch` | `INT(11) UNSIGNED` | `NOT NULL DEFAULT 0` | `u32` bitmask | Equip-switch window positions. |
| `enchantgrade` | `TINYINT UNSIGNED` | `NOT NULL DEFAULT 0` | `u8` | Item grade. |

### Indexes and Key Constraints

* **Primary Key:** `PRIMARY KEY (id)`.
* **Lookup Index:** `KEY char_id (char_id)`. The load query is `WHERE char_id = ? ORDER BY id`; save matches rows by `(nameid, card0..3, unique_id)`.
* **Insert path for new characters:** `INSERT (char_id, nameid, amount, equip, identify) ... identify=1` for each `start_items` entry (`char.cpp:1519`); all other columns take defaults.
* **Related columns elsewhere:** `char.inventory_slots` (capacity, default 100, max 200), `char.uniqueitem_counter` (`INT UNSIGNED`, next `unique_id` low word), `char.zeny` (money is not in this table).
