# Implementation Plan: Items

Spec: `specs/items/spec.md`. Planned on 2026-10-23. M0 was done in the planning session; M1 onwards is for a new
session. **Start by reading this whole file**: the notes below carry context that isn't in the code.

## Decisions (settled with the user)
| Topic | Decision |
| :--- | :--- |
| Mob drops | Mobs can't die yet (no combat; every mob has 1 HP, `crates/ro-engine/src/mob.rs:19`). Build the drop pipeline as an engine API with tests (M8). Calling it at death is deferred to the combat feature. |
| Test items | A GM `@item` command (M9). This needs `0x00f3` chat parsing. Not start items or SQL seed items. |
| Persistence | A whole-inventory snapshot on logout, map change and a periodic autosave (300 s, skipped if unchanged), through the existing ordered writer. Not write-through. |
| Looter mobs | Deferred (spec §5). |
| Importers *(default chosen in planning; user may revisit)* | Two crates, following CLAUDE.md's `tools/<dataset>-import` rule: `tools/item-import` and `tools/mob-import`. mob-import depends on `ro-db` and resolves drop AegisNames through `ItemDb` loaded from the generated `data/item/`, so run item-import first. |
| Mob fields *(default)* | Import `Level/Hp/WalkSpeed/Class/Mvp/Drops/MvpDrops`. Only the drop-related fields are used now. |
| GM gate *(default)* | Our own rule: `accounts.group_id == 99` can use every command. Anyone else's `@…` text is public chat (rAthena's no-permission behaviour). The seed sets account 2000000 (`test`) to 99. |
| Unknown item ids *(default)* | Kept, saved unchanged, weigh 0, listed as Etc with the stored id. |
| Drop level penalty *(default)* | Deferred to combat (it needs the killer and owners). |
| Max weight | `20000 + 300 × STR`. Per-job `MaxWeight` deferred. Captured: STR 1 → 20300, STR 11 → 23300. |
| `ItemId` home | `ro_core::ItemId(pub u32)`, because `InventoryItem` is shared by persistence, engine and map-server. `ro-db` gains a `ro-core` dependency (allowed). `MobId` stays in `ro-db`. |
| Inventory logic home | `ro-engine/src/inventory.rs` (it mutates state and needs `ro-db` types). Not `ro-mechanics`, which is formulas only and isn't an engine dependency yet. |
| Floor items in the world | A new `EntityKind::FloorItem` in `World`, reusing the spatial grid, `step_visibility` and the AoI emitters. Raise `DEFAULT_ENTITY_CAPACITY` 4096 → 16384 (capacity is just a limit; the columns grow lazily). |
| Timers | `BinaryHeap<Reverse<(expire_ms, Entity)>>` in `MapInstance`, popped in `step_tick`. Generational handles make stale entries harmless. |
| Save strategy | DELETE + multi-row INSERT (`UNNEST`) + counter UPDATE in one transaction. At most 100 rows, ids rise in slot order (stable wire indexes on relog), no row-id bookkeeping. |

## Notes for the implementer

### Git state at hand-off
- The user asked to **redact the capture and rewrite history**. The redaction is done in the working tree:
  - `0x08b8` PINs → `[redacted: PIN code]`;
  - the bogus PIN frames `0x3931` / `0x3236` → `0x????`;
  - the four `0x008e` lines → `[redacted: rAthena message text …]`.
- The `git commit --amend` of `b8cce20` ("add item drop packet sequence", HEAD, already on `origin/main`) and
  the `git push --force-with-lease` were **blocked by the auto-mode permission check**. The user runs them by
  hand.
  - Check `git log -1 --stat` before M1. If HEAD is still `b8cce20` and the capture is still modified, remind
    the user.
  - Amend only the capture file; leave the M0 catalog and spec files uncommitted.
- Commit-once-per-plan: everything else stays uncommitted until Finish. The generated data (M3) may be its
  own commit.

### Capture facts (`reference/catalog/captures/item_drop_pickup_sequence.txt`)
- **Sessions:** `#6` is account 2000000 / char 150000 (STR 11 → maxweight 23300), the attacker and picker.
  `#3` is account 2000001 / char 150006 (STR 1 → 20300), the observer. Both are in a party, on `prt_fild08`.
- **`0x0add`** (line 1116): `add0 | 41 00 00 00 | 8d 03 00 00 | 03 00 | 01 | 9d 00 | 4b 01 | 09 | 09 | 01 00 | 00 | 00 00`.
  24 bytes; the proxy splits off the last 2 as a bogus `0x0000` frame. Use it verbatim as the PKT-ITEM-03
  vector.
- **Pickup** (lines 1129-1141):
  - The client sends `0x0361` direction, then `0x0362 {41 00 00 00}`.
  - Picker: `0x0b41` → `0x00b0 {18 00 | 50 05 00 00}` (weight 1360) → `0x08c8` → `0x00a1`.
  - Observer: `0x009c` (from the direction intent) → `0x08c8` → `0x00a1`.
- **`0x0b41`** (70 bytes): the first 32 payload bytes are `05 00 01 00 8d 03 00 00 01 00` + 16×`00` (cards) +
  `00 00 00 00` (location) + `03` (type) + `00` (result).
  - The proxy truncates after 32 bytes, so PKT-ITEM-04 can only pin those bytes.
  - The Jellopy went to wire index 5 (slot 3) because of holes: a fresh add takes the lowest empty slot.
- **`0x08c8` pickup:** `80 84 1e 00 | 41 00 00 00 | 00…` with `type` = 1 at payload offset 24, everything
  else 0, tick included. Reuse `ServerEvent::EntityAction` (`crates/ro-core/src/message.rs:398`) with
  `action_type: 1, action_tick: 0`.
- **Entry lists** (lines 75-79):
  - `0x01d7`(15) → `0x0b08` (`08 0b 06 00 00 00`) → `0x0b09` → `0x0b39` → `0x0b0b` (`0b 0b 00 00`) →
    `0x0a9b` (`9b 0a 04 00`) → SP_WEIGHT → SP_MAXWEIGHT.
  - The whole list is re-sent on *every* `0x007d`; the client often sends `0x007d` twice.
  - `0x0ade {46 00 00 00}` (70%) comes earlier, from `pc_scdata_received`. It's deferred.
- **The drop that "did nothing"** (line 1151, `0x0363 {0c 00 | 02 00}` = wire index 12 ×2, not the Jellopy):
  - The proxy's *logger* stalled at 09:09:24.041. `PacketLogger::feed_and_log` waits on `Ok(None)` for a
    misframed dynamic length.
  - Forwarding is raw-before-log (`tools/proxy/src/forwarder.rs:187-204`), so the client was unaffected.
  - Most likely rAthena refused the drop (`0x00af` count 0; a NoDrop item?). See PF-21.
  - The Finish recapture settles it.
- **Mob kill timing** (PF-15): `0x0977` hp 0 → exp → `0x0add` in the same tick, then `0x0080` type 1 at
  +246 ms. The first drop is on the mob's own cell (157,331).
- **`0x00f3`** (`packet_sequence-20261023_030246.txt`): `testtest : @item knife`, length 26, **no NUL**.

### Existing code anchors (line numbers as of 2026-10-23)
- **`crates/ro-packets/src/table.rs`:**
  - `0x01d7 => Fixed(11)` at :235; `0x0add => Fixed(22)` at :956. Both are wrong.
  - `0x0362` / `0x0363` at :566-567 (correct).
  - `0x0a9b` Dynamic :941, `0x0ade` Fixed(6) :957, `0x0b09` :967, `0x0b39` :983 (OK).
  - `0x008d` / `0x008e` / `0x00f3` Dynamic :52/:53/:104.
  - **Missing:** `0x00a1`, `0x009d` (it's in the rejected-legacy list at :1206), `0x0b41`, `0x0b08`, `0x0b0b`,
    and **`0x00af`**.
- **`crates/ro-engine/src/engine.rs`:**
  - `DEFAULT_ENTITY_CAPACITY` :34.
  - `PLACEHOLDER_WEIGHT/MAX_WEIGHT` :41-42, sent in `actor_init` :379-414 (replace these).
  - `START_NPC_ID` :52; `VANISH_*` :63-64.
  - `emit_self` :270, `emit_aoi` :279, `entry_event` :296, `handle_intent` :321 (the intent match is around
    :363).
  - `despawn_player` :418; `step_tick` :852 (`advance_walkers` → intents → `plan_moves`).
- **Other engine files:**
  - `spatial.rs:137` `step_visibility`: enter/leave diffs. NPCs and mobs get no events (players are filtered).
  - `world.rs`: `EntityKind {Free, Player, Npc, Mob}` :15-22; `PlayerInfo` :26-37; dense columns :88-98;
    `gid_to_entity`/`char_to_entity` :106-107; `show`/`hide` :236-253; `despawn` :256.
  - `gid()` checks the sparse sets one by one. Add a dense `gids` column when adding the 4th kind.
  - `rng.rs` `SplitMix64` (`below`, `range_inclusive`), seeded from the map name and shared with warps and mob
    placement. Extra draws change later random warp cells; check tests that pin cells.
  - `warp.rs:170` `warp_cross_map`: move the inventory out of the component into `CharacterSpawn` (no
    clone).
- **`crates/ro-core/src/character.rs`:** `CharacterSpawn` :12 (`from_summary` :29). There are **15
  `CharacterSpawn {` literal sites** to update:
  - `ro-core/src/character.rs` ×2
  - `ro-engine/src/world.rs` ×2, `ro-engine/src/warp.rs` ×2
  - `ro-engine/tests/{npc,mob}_tests.rs` ×1 each, `map_instance_tests.rs` ×2, `warp_tests.rs` ×3
  - `bins/map-server/src/service.rs` ×2

  `CharacterSummary` (`ro-core/src/ids.rs:30`) has no `str`. `characters.str` exists in SQL; `group_id` is on
  `accounts`.
- **`bins/map-server/src/`:**
  - `service.rs`: `TICK_INTERVAL` 20 ms :27, `resolve_entry` :350 (calls `char_repo.find_by_id`),
    `hand_off` :610, `join` :670, `player_intent` :769, `remove_session` :838, `route` :924.
  - `claim.rs`: `PositionWrite` :39, `SharedState` :47 (`recent_position` :99, `persist` :106),
    `run_position_writer` :139, `Claim` :185 (`set_last` :197, `Drop` :203), `FakeRepo` :236.
  - Generalize the writer to an enum `{Position, Inventory}` and keep the per-character order.
- **`crates/ro-persistence/sql/01_schema.sql`:** `CREATE TABLE IF NOT EXISTS`, applied by `init_schema` via
  `include_str!`.
  - **New columns on existing tables must be added with `ALTER TABLE … ADD COLUMN IF NOT EXISTS`**, or dev DBs
    never get them.
  - `reset_tables` truncates `characters, accounts`; add `inventory`.
- **`02_seed.sql`:** seeds only account 2000000 `test` (`ON CONFLICT DO NOTHING`). Add
  `UPDATE accounts SET group_id = 99 WHERE account_id = 2000000;` so existing DBs get it. `test2` (2000001)
  exists only in the user's local DB.
- **Map-server test harness:** `bins/map-server/tests/common/mod.rs` (`data_loader`,
  `TestServer::start_with_loader`). Existing BIN tests that assert the post-`0x007d` sequence (weight
  placeholders) need updating.

### Catalog facts gathered in M0 (also written into the catalog)
- **Player drop** (`ground_items.md` "Player drop"):
  - `flags = 2|4`: bit 2 means search distance 2; bit 4 skips the untradable check; bit 1 means MVP timers.
  - `DIR_MAX` means the free-cell search always runs and never picks the own cell.
  - `item_stacking` defaults to yes, so only walkability and the edge margin block a cell. On failure the
    dropper's own cell is used.
  - Order: `0x0add` AREA → SP_WEIGHT → `0x00af`. No `0x07fa`; effect 0.
  - Dropping an equipped item is allowed in rAthena.
- **Pickup gates** (`ground_items.md` "Pickup gate details"):
  - `pc_cant_act` covers NPC dialog, chat room, trade, vending, storage, opt1 and UI windows. We have none of
    these.
  - Sitting is allowed; a walk is not stopped; walking to the item is client-side.
  - Dead: `0x0080` type 1 to AREA (self too), then result 6.
- **`0x0a9b`:** `u16 op, u16 len, {u16 index+2, u32 pos}*`. It's always sent (4 bytes when empty) right after
  `0x0b0b`.
- **`0x0ade`:** `u32 percent` (`natural_heal_weight_rate`, 70). Sent from sc-data receive, not the inventory
  path. Deferred.
- **Chat:**
  - Validation order: frame length < 4 → ignore; too short → ignore; name or `" : "` mismatch → disconnect;
    > 255 bytes → ignore; then `is_atcommand` on the stripped text.
  - Normal chat: `0x008d` (gid = speaker) to `AREA_CHAT_WOC` (±9), then `0x008e` to self.

### Pitfalls
- **The 2-byte failure-ack rule:** every rejected pickup must send `0x0b41` and every rejected drop `0x00af`
  count 0, or the client locks its pickup/drop UI.
- **Commands and chat text:** never paste rAthena `msg_conf` strings. Write our own.
- **Importer data:** pass an expanded path to `--rathena`, because `$RATHENA_PATH` may hold a literal `~`. Check
  debug-mode load time of ~29k items. If it's slow, add `[profile.dev.package.<yaml crate>] opt-level = 3`.
- **Hot path:** floor-item expiry pops a heap per tick. Don't scan all floor items, and don't clone item
  structs in the tick loop (move snapshots out only on save).
- **`unique_id` > i64::MAX:** stored bit-cast; test the top bit (PER-INV-01).

## Implementation notes (M1 onwards)
Rulings made during implementation, and review results later milestones must honour.

- **M3:** the mob importer's scenarios are `TOOL-MOBDB-01..03`: `TOOL-MOB-*` was already taken by the
  mob-spawning spec (`tools/npc-import`). The catalog's 5684 mob count included 3009 commented-out entries;
  live is 2675 (catalog fixed). The ro-db and importer tests use 2675.
- **M4:** the captured `0x08c8` pickup has `div` 0 (only src, target, type 1), so `EntityAction` type 1 has its
  own encoder branch. `0x0add` is named `ZC_ITEM_FALL_ENTRY5` (`ZC_ITEM_FALL_ENTRY` is `0x009e`).
- **M5 schema:** `init_schema` deadlocked under concurrent runs once the schema had an `ALTER`. Fixed with a
  `pg_advisory_xact_lock` at the top of `01_schema.sql` and a `DO` block that runs the `ALTER` only when the
  column is missing (spec §2.8 updated). `group_id` is `i32` everywhere, like `Account`.
- **M5 early review (reviewer agent), applied:** loads with more than 100 rows and saves with more than 100
  slots are refused (`Internal`) instead of silently truncated; `load_play_state` reads in one
  `REPEATABLE READ` transaction; the save's `UPDATE`-first order is commented and pinned by
  `per_inv_03_concurrent_saves_serialize`; index on `(char_id, id)`; the seed inserts group 99 directly.
- **M5 review, for M10 (must do):** the claim writer (`claim.rs` `run_position_writer`) retries *any* error
  until a newer write arrives, so a permanent error (a deleted character's `NotFound`, an `Internal`) stalls
  the whole queue forever. Retry only `Unavailable`; treat `NotFound` as done; log and drop `Internal`.
- **M5 review, not taken (rulings):**
  - A combined `save_character_state(position, inventory)` call: kept the plan's two calls through one ordered
    writer. Cost if wrong: a crash between the two writes of one logout leaves the new position with the old
    inventory (items gained since the last save are lost, never duplicated).
  - An explicit `slot` column: kept "ids rise in slot order" (the plan's decision). Cost if wrong: slot order
    could shuffle on relog if Postgres ever assigned identity values out of `ORDER BY` order; adding the column
    later needs a backfill.
  - Multi-process hand-off ordering (a `version` column): out of scope; one map-server process today.
- **M6:** the entry lists go out on *every* `NotifyActorInit`, before the "already visible" early return, so a
  repeat now resends them (two existing tests updated). Both spawn paths (`spawn_character`, `enter_by_warp`)
  go through `MapInstance::spawn_player`, which moves the inventory into the `inventories` component.
- **M8:** the API is `roll_mob_drops(mob, roll)` (pure), `MapInstance::drop_items(request, items, now)`
  (placement, owners, effects) and `drop_mob_items(request, now)` (both, with the instance RNG). Ruling:
  mob drops of weapons, armor, pet armor and shadow gear are unidentified, everything else identified
  (rAthena `itemdb_isidentified`, catalog `ground_items.md` `0x0add` notes); the spec was silent. Cost if
  wrong: one flag on equipment drops.
- **M9:** ruling: `ClientIntent::Chat.text` always holds the full `"<name> : <message>"`. The map-server
  checks the prefix and closes the session on a mismatch (M10); the engine strips the prefix it knows
  (dropping a mismatch) and also ignores messages over 255 bytes. Cost if wrong: none on the wire; the spec
  said the engine gets the stripped message. `@item` stops a non-stackable loop at the first failed add
  (later adds can't succeed), so a huge amount can't stall the tick; only the last failure is sent, like
  rAthena.
- **M10:** the service holds an `Arc<dyn MapRepository>` (character + inventory repository, blanket impl);
  the registries travel in `MapData.registries`; `MapServiceHandle::with_options` sets the autosave interval
  (tests use ~1 s). The writer is `Write { Position | Inventory }`, retries only `Unavailable`, treats
  `NotFound` as done, and logs and drops other errors (keeping the queued snapshot for a re-entry).
  Autosave is checked once a second per map actor. Ruling: map-server now runs `init_schema` at startup;
  nothing ran it before (the schema came only from Docker's first-volume init), so existing dev databases
  would never get the `inventory` table. Cost if wrong: one idempotent schema pass per start.
- **Final review (reviewer agent, 2026-10-23):** no Critical findings. Fixed:
  - (Important) a cross-map warp in the tick its session is evicted lost the inventory: the save now
    happens when the warp is taken, before the flush (`bin_warp_07_eviction_in_the_transfer_tick_…`
    RED→GREEN). Spec §2.8 updated.
  - (Important) transient database errors (SQLSTATE classes 08/40/53/57, a closed pool, a crashed worker)
    were `Internal`, so the writer dropped them; they are `Unavailable` now and retried
    (`transient_errors_are_unavailable` RED→GREEN).
  - Minors fixed: a pet-egg-only `@item` no longer says items were added; chat accepts at most 254 message
    bytes as the catalog says (spec said 255); weight math saturates; a failed warp despawn gives the
    inventory back; `gid()` asserts liveness in debug; a non-player mover skips building entry events;
    `drop_mob_items` borrows the mob instead of cloning it; the unreachable drop-path returns warn;
    BIN-ITEM-03 checks the saved content.
  - Not changed (rulings): a pickup or drop before `0x007d` gets no ack (gameplay intents before actor
    init are ignored everywhere; cost if wrong: a client that sends one in the warp window locks its
    pickup UI until the next ack).

## Milestones
Write tests first, named with spec IDs. Iterate with `./verify.sh -p <crate>`.

- [x] **M0 research, catalog, spec, plan** (2026-10-23)
  - Capture redacted (the amend and force-push are pending with the user, see above).
  - `captures/README.md` row added.
  - `PACKET_FINDINGS.md`:
    - PF-01, PF-02, PF-04 settled by capture (move to Resolved when M1 lands);
    - PF-06 settled (no NUL);
    - PF-07 and PF-08 half settled;
    - PF-15 confirmed;
    - PF-21 gets the logger stall.
  - Catalog verification notes updated in `ground_items.md`, `packets/inventory.md`, `mechanics/inventory.md`
    and `mobs.md`.
  - `metaling-explorer` added "Player drop", "Pickup gate details", `0x0a9b` / `0x0ade` and chat additions,
    plus `INDEX.md` rows.
  - This spec and plan.
- [x] **M1 ro-packets table** (2026-10-23)
  - Add or fix: `0x0add` 24, `0x00a1` 6, `0x009d` 19 (remove it from the legacy list), `0x0b41` 70, `0x0b08`
    Dynamic, `0x0b0b` 4, `0x01d7` 15, `0x00af` 6.
  - PKT-ITEM-01 in `crates/ro-packets/tests/framing_tests.rs`.
  - Move PF-01, 02 and 04 to the Resolved table, and trim the PF-21 list.
- [x] **M2 ro-core + ro-db** (2026-10-23)
  - ro-core: `ItemId`, `ItemOption`, `InventoryItem`, `CharacterInventory`, the wire `ItemEntry`, the
    intents and the events (spec §2).
  - `CharacterSpawn` gains `str`, `group_id` and `inventory` (15 sites).
  - ro-db: `item.rs` (`ItemType::{wire_type, is_stackable}`, `ItemData`, `ItemDb`) and `mob.rs` (`MobData`,
    `MobDrop`, `MobDb`, `boss_type`). Follow the `npc.rs` patterns and the `yaml.rs` header helper; re-export
    from the root.
  - DB-ITEM-01/02, DB-MOBDB-01/02.
  - `crates/ro-db/DESIGN.md`.
- [x] **M3 importers + data** (2026-10-23; the catalog's 5684 mob count included 3009 commented-out entries: 2675 live, catalog fixed)
  - `tools/item-import` and `tools/mob-import` (`[lints] workspace = true`). Generate into
    `crates/ro-db/data/{item,mob}/`.
  - `NOTICE.md` sections.
  - TOOL-ITEM-01..05, TOOL-MOBDB-01..03, DB-ITEM-03, DB-MOBDB-03.
  - Data in its own commit.
  - Add both tools to README Acknowledgements if a new source is used (the same rAthena repo, so probably only
    the NOTICE).
- [x] **M4 ro-packets codecs** (2026-10-23; the `0x08c8` pickup sets `div` 0, so the encoder got a type-1 branch)
  - Decoders `0x0362`, `0x0363`, `0x00f3` (raw bytes; tolerate a trailing NUL).
  - Encoders for every new event.
  - PKT-ITEM-02..07, PKT-CHAT-01/02 (byte vectors from the capture).
  - Move PF-06 to Resolved.
- [x] **M5 ro-persistence** (2026-10-23; early review done, see "Implementation notes")
  - `inventory` table plus the `ALTER` for `unique_item_counter`; the seed `UPDATE`.
  - `InventoryRepository { load_play_state, save_inventory }` on `PostgresRepo`; `reset_tables`.
  - PER-INV-01..04.
  - `DESIGN.md`.
  - **Early `reviewer` pass on the schema and repository API** (later milestones build on it).
- [x] **M6 engine inventory core** (2026-10-23)
  - `inventory.rs` (`PlayerInventory`, `add_item`, `remove_item`, entries) as a `SparseSet` on players.
  - `actor_init` sends the entry lists.
  - `inventory_snapshot` / `inventory_revision` API; the warp carries the inventory.
  - ENG-INV-01..06.
- [x] **M7 engine floor items, pickup, drop** (2026-10-23)
  - `floor_item.rs`, `EntityKind::FloorItem`, the dense `gids`, capacity 16384, the id counter, the expiry
    heap.
  - AoI branches.
  - `PickUpItem` / `DropItem` handling.
  - ENG-ITEM-01..08.
  - `ro-engine/DESIGN.md` §1a.
- [x] **M8 engine mob-drop API** (2026-10-23)
  - `drops.rs` (`roll_mob_drops`, `MapInstance::drop_mob_items`).
  - ENG-MOBDROP-01..04 with scripted rolls.
- [x] **M9 engine chat + `@item`** (2026-10-23)
  - `command.rs`.
  - ENG-CHAT-01/02, ENG-CMD-01..06.
- [x] **M10 map-server wiring** (2026-10-23)
  - Config and env (`ITEM_DATA_DIR`, `MOB_DATA_DIR`); `main.rs` registries.
  - `resolve_entry` loads the play state, preferring a recent snapshot.
  - Claim writer enum, hand-off save, autosave.
  - The chat prefix check (disconnect on a mismatch).
  - `run_metaling_servers.sh` exports.
  - Fakes get an inventory store.
  - BIN-ITEM-01..06, BIN-CHAT-01/02.
- [x] **Finish** (reviewer, fixes, DESIGN docs, DEFERRED and smoke done 2026-10-23; committed 2026-10-24. The manual client check and the recapture analysis are tracked in `DEFERRED.md`; the check is blocked on `specs/map-entry-settings`.)
  - Run the `reviewer` over `git diff HEAD` plus untracked files; apply its fixes; commit.
  - Update `DESIGN.md` for ro-engine, ro-db and ro-persistence.
  - `DEFERRED.md` "items" section from spec §5. Amend the mob-spawning "mob_db stats" entry: the data is now
    imported, the wiring isn't.
  - `./verify.sh --smoke`.
  - **Manual client check** (account `test`, group 99):
    - `@item 501 5`, `@item Knife`, `@item "Red Potion"`;
    - relog → persisted; warp → kept;
    - drop → the observer sees it;
    - pick up; walk away and back (`0x009d` / `0x00a1`);
    - 60 s expiry;
    - normal chat; a non-GM `@item` shows as chat.
  - **Recapture on rAthena** with the fixed proxy table:
    - a drop (the `0x00af` reply);
    - walking back to an old drop (`0x009d`, PF-03);
    - an overweight or full pickup and an early pickup (the `0x0b41` failure form, PF-07).
