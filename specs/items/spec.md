# Specification: Items (inventory, ground items, pickup, player drop, mob-drop pipeline, `@item`)

**Scope:**
- Characters own a persisted inventory and see it on every map entry.
- Players can drop items and pick up items from the floor, and other players see both.
- Floor items expire after 60 s.
- The mob-drop pipeline (roll, cell placement, loot priority, boss timers) exists as a tested engine API but
  is **not wired to mob death**: there's no combat yet, so death wiring is deferred to the combat feature.
- Items for testing come from a GM `@item` command, which needs public chat parsing.

Out of scope (see §5):
- equip and unequip
- item use
- looter mobs
- autoloot
- party loot share
- rental items
- overweight statuses

**Ground truth:** rAthena commit `d4b8e7b` at `PACKETVER = 20260219`, as recorded in the catalog:
- `reference/catalog/packets/inventory.md`: list packets, `0x0b41`, `0x00af`, `0x0a9b`, `0x0ade`, client opcodes.
- `reference/catalog/packets/ground_items.md`: `0x0add`, `0x009d`, `0x00a1`, the floor item object,
  pickup and pickup gate details, player drop.
- `reference/catalog/mechanics/inventory.md`: `struct item`, `pc_additem` §3, `pc_delitem` §4,
  `unique_id` §5, drop §6, weight §8, when lists are sent §10, persistence §11.
- `reference/catalog/mechanics/mobs.md` §4: drop roll, drop cells, drop timer.
- `reference/catalog/db/item_db.md`, `reference/catalog/db/mob_db.md`.
- `reference/catalog/mechanics/atcommands.md` (`is_atcommand`, `@item`).
- `reference/catalog/packets/chat.md`.
- Captures:
  - `reference/catalog/captures/item_drop_pickup_sequence.txt`: drop, pickup and entry lists.
  - `packet_sequence-20261023_030246.txt`: the `0x00f3` `@item knife` frame.
- `PACKET_FINDINGS.md` PF-01, 02, 03, 04, 06, 07, 08, 15, 21.

When this spec and the catalog disagree, the catalog wins and this spec gets fixed.

---

## 0. Data pipeline

```text
$RATHENA_PATH/db/re/item_db_{usable,equip,etc}.yml ──► tools/item-import ──► crates/ro-db/data/item/*.yml (GPL)
$RATHENA_PATH/db/re/mob_db.yml ──► tools/mob-import ──(resolves Drops via ro_db::ItemDb)──► crates/ro-db/data/mob/mob_db.yml (GPL)
                                                                     │ ro_db::ItemDb / MobDb (loaded once at startup)
                                                                     ▼
          bins/map-server: Arc<ItemDb>, Arc<MobDb> ──► every ro_engine::MapInstance
PostgreSQL inventory table ◄──► ro_persistence::InventoryRepository ◄──► map-server claim writer
```

- Registries are global (not per map). They're loaded once at map-server start and shared through `Arc`.
- `ro-db` never parses rAthena YAML. The importers write our own schema.
- **Runtime directories:** `ITEM_DATA_DIR` / `--item-data-dir` and `MOB_DATA_DIR` / `--mob-data-dir`.
  - Unset: a warning, and empty registries. `@item` finds nothing; items already in an inventory show as unknown.
  - Set, but a file is missing or bad: startup fails.

## 1. YAML schemas

### 1.1 `ITEM_DATA` v1 (`crates/ro-db/data/item/item_{usable,equip,etc}.yml`)

```yaml
# SPDX-License-Identifier: GPL-3.0-only
# Derived from rAthena (https://github.com/rathena/rathena) commit d4b8e7b..., db/re/item_db_usable.yml.
# Modified: reduced to the fields Metaling uses and normalized by tools/item-import. Do not edit by hand.
# See crates/ro-db/data/LICENSE and crates/ro-db/data/NOTICE.md.
Header:
  Type: ITEM_DATA
  Version: 1
Body:
  - Id: 501
    AegisName: Red_Potion
    Name: Red Potion
    Type: Healing          # after load rules: DelayConsume -> Usable; equip type without Locations -> Etc
    Weight: 70             # 0.1 units
    # optional keys, omitted when default:
    Locations: 0x2         # u32 EQP_ mask (equip types and Ammo)
    View: 0
    ViewId: 0              # resolved AliasName target id; 0 = none
    Slots: 0
    StackAmount: 0         # 0 = no per-item cap
    StackInventory: false  # rAthena: false when a Stack node lacks the key
    UniqueId: false
    BindOnEquip: false
    DropEffect: 0          # e_item_drop_effect value; 0 = none, CLIENT = 1
    NoDrop: false
    NoDropOverride: 100    # Trade.Override GM level
```

- The file split mirrors the source files.
- Entries are written in id order, and every id is unique across all three files. The importer merges
  repeated ids field by field, as rAthena does.

### 1.2 `MOB_DATA` v1 (`crates/ro-db/data/mob/mob_db.yml`)

```yaml
Body:
  - Id: 1002
    AegisName: PORING
    Name: Poring
    Level: 1
    Hp: 55
    WalkSpeed: 400
    Class: Normal          # Normal | Boss | Guardian | Battlefield | Event
    Mvp: false
    Drops:                 # <= 10, db order kept, Item resolved to id
      - { Item: 909, Rate: 7000 }
    MvpDrops: []           # <= 3; imported, unused until MVP rewards
```

- `Level`, `Hp` and `WalkSpeed` are imported but not wired; that stays deferred under "mob_db stats".
- `boss_type()`: 2 if `Mvp`, else 1 if `Class: Boss`, else 0. Loot timers use boss_type != 0.

## 2. Behaviour

### 2.1 Inventory model (catalog `mechanics/inventory.md` §1-§5, §8)
- **Slots:**
  - 100 slots (`INVENTORY_SLOTS`), indexed from 0. Wire index = slot + 2.
  - Holes are allowed at runtime. A load compacts them (`ORDER BY id`).
- **Item fields:** an item keeps every `struct item` field (cards, options, expire, favorite, bound,
  unique_id, equip_switch, grade), even though this feature only produces plain items.
- **Weight:**
  - Unit is 0.1. `weight = Σ Weight × amount`.
  - `max_weight = 20000 + 300 × STR`. The per-job `MaxWeight` is deferred; it's 20000 for novices, the only
    job character creation makes. Captured: STR 1 → 20300, STR 11 → 23300.
  - Unknown item ids weigh 0.
- **`add_item`:** follows catalog §3 in check order, and the result codes go on the wire unchanged:
  1. invalid
  2. `amount > 30000` → 5
  3. stack limit → 7
  4. overweight → 2
  5. unique id assigned
  6. merge with an existing stack (stackable, `expire_time == 0`; key `nameid, bound, unique_id, cards`)
     → 5 on overflow
  7. otherwise the lowest empty slot → 4 if none
  8. update weight
- **`unique_id = (char_id << 32) | counter`:** the counter is per character, persisted, and bumped per
  assignment.
- **`remove_item(slot, amount)`:** fails on an empty slot, a bad amount or an amount over the stack. On success
  it subtracts the weight and clears the slot at 0.

### 2.2 Entry lists (catalog `packets/inventory.md`, captured order)
- **Order:** on every `CZ_NOTIFY_ACTORINIT`, including repeats and same-map warps, self gets:
  1. `0x0b08` (inv_type 0, empty name, 6 bytes)
  2. `0x0b09` (only if any stackables)
  3. `0x0b39` (only if any non-stackables)
  4. `0x0b0b` (0, 0)
  5. `0x0a9b` (empty, 4 bytes)
  6. `0x00b0` SP_WEIGHT
  7. `0x00b0` SP_MAXWEIGHT
- **Placement:** this replaces today's placeholder weight pair at the start of `actor_init`. The lists come
  *before* the unit-entry packets, as in rAthena.
- `0x01d7` and `0x0ade` are not sent (the look-change packet is deferred, PF-01).
- **Entries:** in slot order, empty slots skipped. A stackable/non-stackable split uses `is_stackable()`.
  - Fields come from the item plus the registry: `ITID` = ViewId or id, type = wire type, `WearState`
    (`0x0b09`) / `location` (`0x0b39`) = the `Locations` mask, `wItemSpriteNumber` = View when the location
    intersects `EQP_VISIBLE`.
  - Unknown ids: wire type 3, location 0, ITID = the stored id.

### 2.3 Floor items (catalog `packets/ground_items.md`)
- **Fields:** the item copy, `sub_x` / `sub_y` (each `rnd[1,4] × 3`), three owner char ids and owner
  ticks, `expire_at = now + 60000`, `mob_id`.
- **Ids:** a per-instance counter cycling through `[2, 2_000_000)` that skips taken ids. These are disjoint from
  account ids (≥ 2,000,000) and NPC/mob ids (≥ 110,000,000).
- **Visibility:**
  - At creation: `0x0add` to everyone within 14 cells of the item cell, the dropper included.
  - Coming into view (walking, or `actor_init`): `0x009d` to self.
  - Leaving view: `0x00a1` to self. Never `0x0080`.
- **Expiry:** at 60 s, `0x00a1` to the area around the item, then it's removed. No warning packet.

### 2.4 Pickup (`0x0362`, catalog Pickup and "Pickup gate details")
1. **Checks, in order:**
   1. The picker is dead: `0x0080` type 1 to the area including self, then `0x0b41` result 6.
   2. The id is not a floor item on this map: result 6.
   3. Chebyshev distance > 2 from the picker's current cell: result 6.
   4. The loot tick isn't reached:
      - no owner, or the picker is owner 1: free at once;
      - owner 2: waits for tick 1;
      - owner 3: waits for tick 2;
      - anyone else: waits for tick 3.
      If `now <` that tick: result 6.
2. **Sitting and walking:** a sitting picker is allowed and stays sitting. A walk in progress is *not*
   stopped. Walking to the item is client-side.
3. **Add:** `add_item(floor item, amount)`. On failure: `0x0b41` with that result code, and the item stays.
4. **Success, in order** (captured):
   1. `0x0b41` (slot + 2, amount added, full fields, result 0) to self.
   2. `0x00b0` SP_WEIGHT to self.
   3. `0x08c8` (src = picker gid, target = floor id, type 1, every other field 0) to the area incl. self.
   4. `0x00a1` to the area incl. self.
   5. The item is removed.
- **Failure form of `0x0b41`:** index 2, count 0, result N, every other byte 0.

### 2.5 Player drop (`0x0363`, catalog "Player drop")
1. **Gates, in order.** Any failure → `0x00af` {wire index, 0} to self and nothing else:
   - dead;
   - slot out of `[0, 100)`;
   - amount ≤ 0;
   - empty slot or amount > stack;
   - `equip_switch != 0`;
   - `NoDrop` (there are no GM levels yet, so the override never applies);
   - no free cell, which can't happen with stacking on (see 2).
2. **Cell:**
   - Always run the search, never the dropper's own cell first. 8 candidates: random mirror `mx, my ∈ {±1}`.
     Loop `i ∈ -1..1` (outer) and `j ∈ -1..1` (inner), skipping (0,0). Candidate =
     `(x + mx·i·rnd[1,2], y + my·j·rnd[1,2])`, with a fresh `rnd` per axis per candidate.
   - A candidate is free when it is walkable and ≥ 5 cells from the edge (`x ≥ 5 && x ≤ w-5`, same for y).
     Units and other floor items never block (`item_stacking: yes`).
   - If none is free, use the dropper's own cell.
3. **Floor item:** no owners, 60 s lifetime, effect 0.
4. **Equipped items:** rAthena allows dropping them. We have no equip flow yet, so `equip` is always 0. When
   equip lands, decide whether to unequip first.
5. **Success, in order:**
   1. `0x0add` to the area incl. self.
   2. `0x00b0` SP_WEIGHT to self.
   3. `0x00af` {wire index, amount} to self.

   No `0x07fa`.

### 2.6 Mob-drop API (catalog `mechanics/mobs.md` §4, not wired)
- **`roll_mob_drops(mob, roll)`:**
  - For each `Drops` entry in db order: `rate = entry.rate`. If `rate > 9000` and the base rate is
    `≤ 9000`, cap it at 9000. Clamp to `[1, 10000]`.
  - Drop one item when `roll() % 10000 < rate`.
  - Level penalty, player drop bonuses and MvpDrops are deferred.
- **`drop_mob_items(request, now)`:** `request = { mob_id, cell, owners: [Option<CharId>; 3] }`.
  - Placement: the first item goes on the mob's cell (no free check). The following items cycle
    **SE → W → N** as one-cell offsets from the death cell. An offset cell is used if free
    (`map_cell_free`: walkable, 5-cell margin, no player/mob on it). Otherwise run the 8-neighbour search of
    2.5 with distance 1 (`rnd` fixed at 1) around the offset cell. If that also fails, stay on the offset cell.
  - Loot ticks from `now`: normal +3000 / +2000 / +2000 (cumulative); boss or MVP +10000 / +10000 / +2000.
  - Effect: `showdropeffect = DropEffect > 0`, `dropeffectmode = DropEffect - 1`.
  - Sends `0x0add` per item to the area. Returns the number of items placed. Running out of capacity or ids
    stops placement cleanly.
- **Timing:** rAthena runs this on a +0 ms timer after death, in the same tick, after the exp packets
  (PF-15). The combat feature calls it at that point.

### 2.7 Chat and `@item` (catalog `packets/chat.md`, `mechanics/atcommands.md`)
- **`0x00f3` framing:**
  - The text is not NUL-terminated (captured).
  - The map-server checks the `"<name> : "` prefix. A mismatch closes the session; that's rAthena's `set_eof`.
  - Over 254 bytes of message: ignored (rAthena's 256-byte buffer holds the message, the NUL it adds, and
    rejects `length + 1 > 255`).
- **The engine gets the message without the prefix:**
  - Text starting with `@` from a `group_id == 99` sender is a command. Everything else is public chat.
    rAthena treats "no permission" as plain chat.
  - Public chat: `0x008d` (gid = speaker, full `"<name> : msg"` text) to players within ±9 cells, excluding
    the speaker, then `0x008e` echo to the speaker.
- **`@item <name|id>[:<name|id>…] [amount]`:**
  - Names may be quoted. Lookup order: lowercase AegisName, then lowercase Name, then numeric id.
  - Amount omitted or ≤ 0 becomes 1.
  - Non-stackables are added one at a time, `amount` times. A stackable is added once with the full amount.
  - Items are identified (`identify = 1`).
  - Pet eggs get our own refusal message.
  - Results:
    - Not found or bad syntax: our own message via `0x008e`.
    - The last add failed: `0x0b41` with its result code.
    - Success: our own `0x008e` line.
  - **Wording is our own; never copy rAthena `msg_conf` text.**
- An unknown command from group 99 gets our own message.

### 2.8 Persistence (catalog `mechanics/inventory.md` §11)
- **Table `inventory`:**
  - `id BIGINT GENERATED ALWAYS AS IDENTITY`, `char_id` (FK → characters, `ON DELETE CASCADE`, indexed).
  - Item columns: `nameid`, `amount`, `equip`, `identify`, `refine`, `attribute`, `card0..3`,
    `option_id0..4` / `option_val0..4` / `option_parm0..4`, `expire_time`, `favorite`, `bound`,
    `unique_id`, `equip_switch`, `enchantgrade`.
  - Column types: u32 → `BIGINT`; u64 `unique_id` → `BIGINT` bit-cast.
- **`characters.unique_item_counter BIGINT NOT NULL DEFAULT 0`**, added by a `DO` block that runs
  `ALTER TABLE` only when the column is missing, because `01_schema.sql` is `CREATE TABLE IF NOT EXISTS` and
  existing dev databases never pick up new columns otherwise. (An unconditional `ALTER` takes an exclusive lock
  on every server start and deadlocks with concurrent character writes.)
- **Load:** `ORDER BY id`, so holes compact. **Save:** one transaction that deletes all rows for the character,
  inserts the slots in slot order and updates the counter.
- **Triggers:**
  - logout / eviction / failed hand-off: after the position;
  - cross-map warp: when the warp is taken (before the hand-off, so an eviction in the same tick can't skip it);
  - autosave: every 300 s per online character, skipped when the inventory revision hasn't changed.
  All of them go through the single ordered writer, so the inventory and position for one character never
  reorder.
- **Re-entry before the write lands:** `resolve_entry` takes a still-queued snapshot over the DB, the same
  pattern as the existing recent-position cache.

## 3. Scenarios

**TOOL** (`tools/item-import/tests`, `tools/mob-import/tests`):
- **TOOL-ITEM-01, type rules:** `DelayConsume` becomes `Usable`. An equip type with no `Locations` becomes
  `Etc`. Invalid types become `Etc`. `Both_Hand` / `Both_Accessory` masks.
- **TOOL-ITEM-02:** `AliasName` resolves to `ViewId`. An unknown alias is a warning, and `ViewId` stays 0.
- **TOOL-ITEM-03:** a `Stack` node without `Inventory` gives `StackInventory: false`.
- **TOOL-ITEM-04:** a repeated id merges field by field. Output is sorted, and every file has the header.
- **TOOL-ITEM-05:** counts match the catalog (gated on `RATHENA_PATH`).
- **TOOL-MOBDB-01:** `Drops` resolved by AegisName through `ItemDb`. Unknown items are dropped with a warning.
  At most 10; `Index` overwrites; rate 0 is removed.
- **TOOL-MOBDB-02:** `Class` and `Mvp` are kept; `MvpDrops` at most 3.
- **TOOL-MOBDB-03:** counts match (gated).

**DB** (`crates/ro-db/tests`):
- **DB-ITEM-01:** round-trip load/write. Lookups by id, lowercase aegis and lowercase name.
- **DB-ITEM-02:** rejects a bad header, a duplicate id or a duplicate aegis name, and unknown keys.
- **DB-ITEM-03:** the shipped data loads and every file has the SPDX header.
- **DB-MOBDB-01..03:** the same for mobs, plus `boss_type()`.

**PKT** (`crates/ro-packets/tests`):
- **PKT-ITEM-01, table lengths:**
  - `0x0add` 24, `0x00a1` 6, `0x009d` 19 (no longer rejected), `0x0b41` 70;
  - `0x0b08` dynamic, `0x0b0b` 4, `0x01d7` 15, `0x00af` 6;
  - `0x0b09`, `0x0b39` and `0x0a9b` dynamic.
- **PKT-ITEM-02:** decodes `0x0362` → `PickUpItem` and `0x0363` → `DropItem` (wire index kept).
- **PKT-ITEM-03:** `0x0add` encodes the captured payload byte-for-byte (§2.3, PF-02).
- **PKT-ITEM-04:** `0x0b41` success encodes the captured first 32 bytes. The failure form is index 2, count 0,
  result, zeros. Length 70.
- **PKT-ITEM-05:** `0x0b08` / `0x0b09` / `0x0b39` / `0x0b0b` / `0x0a9b` layouts. The lengths match the captured
  39 / 277 / 209 for 1 / 4 / 3 entries.
- **PKT-ITEM-06:** `0x009d` (19) and `0x00a1` (6) layouts.
- **PKT-ITEM-07:** `0x00af` layout.
- **PKT-CHAT-01:** `0x00f3` decodes with and without a trailing NUL; the captured `testtest : @item knife`.
- **PKT-CHAT-02:** `0x008d` / `0x008e` encode.

**PER** (`crates/ro-persistence/tests/postgres_repo_tests.rs`):
- **PER-INV-01:** a save/load round-trip keeps every field, including u32::MAX cards and u64 `unique_id` with
  the top bit set.
- **PER-INV-02:** holes compact on load, and slot order is kept.
- **PER-INV-03:** a save replaces the rows atomically (two saves give the last state), and the counter is
  persisted.
- **PER-INV-04:** `load_play_state` returns `str` and `group_id`. Deleting a character cascades.

**ENG** (`crates/ro-engine/tests`):
- **ENG-INV-01:** `add_item` result codes 1, 2, 4, 5 and 7 in check order. Nothing changes on failure.
- **ENG-INV-02:** stackables merge on the key; non-stackables take one slot each; the lowest empty slot is
  used; `unique_id` is assigned.
- **ENG-INV-03:** `remove_item` updates the weight and leaves a hole.
- **ENG-INV-04:** the entry list order and contents, including when there are no stackables or no equipment.
  Repeated on a second `0x007d`.
- **ENG-INV-05:** max weight comes from STR. An unknown id shows as Etc, weighs 0 and is kept in the snapshot.
- **ENG-INV-06:** a cross-map warp carries the inventory (the snapshot is equal after arrival).
- **ENG-ITEM-01:** a floor item is visible on entry (`0x009d`), while walking in (`0x009d`) and walking out
  (`0x00a1` to self).
- **ENG-ITEM-02:** expiry at 60 s sends `0x00a1` to the area and frees the id.
- **ENG-ITEM-03:** pickup success order for the picker and for an observer.
- **ENG-ITEM-04:** pickup failures: dead (0x0080 + result 6), unknown id, distance 3, early loot tick per
  owner tier, overweight / full (result 2 / 4, item stays).
- **ENG-ITEM-05:** sitting picker succeeds and stays sitting; a walking picker keeps walking.
- **ENG-ITEM-06:** drop success: cell rules (the own cell is never chosen while a candidate is free; the edge
  margin), packet order, an observer sees `0x0add`.
- **ENG-ITEM-07:** drop failures (each gate) send `0x00af` count 0 only.
- **ENG-ITEM-08:** the floor id counter wraps and skips taken ids. Capacity exhaustion fails cleanly.
- **ENG-MOBDROP-01:** roll: per-entry `% 10000 < rate`, the 9000 cap rule, db order.
- **ENG-MOBDROP-02:** cell pattern: center, then SE / W / N offsets, fallback search, stay on a blocked offset.
- **ENG-MOBDROP-03:** owner ticks, normal vs boss / MVP.
- **ENG-MOBDROP-04:** DropEffect fields in `0x0add`.
- **ENG-CHAT-01:** public chat goes to ±9 excluding the speaker, plus the echo.
- **ENG-CHAT-02:** `@…` from a non-GM is chat.
- **ENG-CMD-01:** `@item 501 5` adds 5 Red Potions (one `0x0b41`).
- **ENG-CMD-02:** `@item Knife 3` adds 3 separate slots.
- **ENG-CMD-03:** `@item "Red Potion"`, an aegis name, a numeric id; lookup order.
- **ENG-CMD-04:** an unknown item or bad syntax gets a message and no add.
- **ENG-CMD-05:** overweight or full sends the `0x0b41` failure with the last code.
- **ENG-CMD-06:** a multi-item `:` list; amount ≤ 0 → 1; an unknown command message.

**BIN** (`bins/map-server/tests`):
- **BIN-ITEM-01:** the entry list order is on the wire after `0x007d`.
- **BIN-ITEM-02:** `@item`, logout, relog: the item is listed. Holes compact.
- **BIN-ITEM-03:** a cross-map warp keeps the inventory and saves it.
- **BIN-ITEM-04:** autosave writes after the interval only when the revision changed.
- **BIN-ITEM-05:** two clients: a drop is seen by the observer, a pickup by both.
- **BIN-ITEM-06:** unset item/mob dirs warn and the server still runs.
- **BIN-CHAT-01:** a name-prefix mismatch closes the session.
- **BIN-CHAT-02:** a non-GM `@item` is broadcast as chat.

## 4. Licensing
- Both importers are clean-room, written from the catalog notes only. They are MIT, with MIT-compatible
  dependencies.
- Generated files carry the SPDX header and attribution. `NOTICE.md` gets "Item database" and "Mob database"
  sections (source paths, commit, date, changes, regenerate command).
- Data is read from disk at runtime; never `include_str!`. `ro-db` keeps `exclude = ["data/"]`.
- **Test fixtures** are our own small samples.
- **Chat and command reply text** is our own wording.

## 5. Deferred (move into `DEFERRED.md` at Finish)
- **Mob drops:**
  - Calling `drop_mob_items` at mob death (combat), with owners from the damage log and the +0 ms timing.
  - The renewal drop level penalty and player drop-rate bonuses.
  - MvpDrops and MVP rewards.
- **Looter mobs** (`MD_LOOTER`, Ai 02 such as the Poring): they pick up floor items, and their `lootlist` drops
  at death with `loot = true` (first loot item goes N, no effect). The mob importer will need the looter bit.
- **Autoloot and party loot share:** `party_share_loot` and party item-share tiers in the loot check.
- **Equip/unequip** (`0x0998` / `0x00ab`), `0x013c`, `0x01d7` look packets, and the equip index at load.
- **Item use** `0x0439` / `0x01c8`, `0x07fa` deletes, item scripts, cooldowns.
- **Rental items:** `expire_time` timers and the rental-expired packets.
- **Overweight statuses** `SC_WEIGHT50` / `SC_WEIGHT90`, and `0x0ade` at entry.
- **Inventory expansion:** `inventory_slots` and a non-zero `0x0b18`.
- **Max weight:** the per-job `MaxWeight` (`job_stats.yml`) and bonuses (Enlarge Weight Limit, mounts).
- **Drop gates:** the `NoDrop` GM override and group levels, the `nodrop` mapflag, `pc_cant_act` states (NPC
  dialog, trade, vending, storage, opt1), and `sc.cant.pickup` / `sc.cant.drop`.
- **Commands:** `@item2`, `@itembound`, pet eggs (`pet_create_egg`), the rest of the command list, and a real
  `groups.yml`-style permission model.
- **`0x009d` capture** (PF-03) and the `0x0b41` failure-form capture (PF-07): see the recapture in plan Finish.
