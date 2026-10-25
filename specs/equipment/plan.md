# Implementation Plan: Equipment

Spec: `specs/equipment/spec.md`. Planned on 2026-10-24. M0 was done in the planning session; M1 onwards is for
new sessions (see "Session split"). **Start every session by reading this whole file**: the notes below carry
context that isn't in the code.

## Decisions (settled with the user)
| Topic | Decision |
| :--- | :--- |
| Order | Equipment before stats. Equipment adds the status recompute-and-diff hook with gear-only values (weapon ATK, hard DEF, range); the stats feature fills in the formulas behind it. ASPD stays 0 until then. |
| Ammo | In scope: equip/unequip, pairing, the cascade when the weapon comes off, the map-entry resend. Ranged auto-attack (next) needs it. |
| Testing job-limited gear | No `@job`. Set the job by hand (`UPDATE characters SET job_class = 3 WHERE name = '…'`, Archer) before logging in. Job change belongs with stats. |
| Item scripts | Out of scope (user). |
| Horizon | Equipment → stats and attributes → auto-attack (melee and ranged) → monster death. Keep `DerivedStatus`, `GearStatus`, `WeaponType` and the job mapid table easy for stats and combat to pick up. |
| Planning defaults *(user may revisit)* | Ammo EFST icon: mirror rAthena (`0x0983` to the area, never ended). Look columns: rename `weapon_view` → `weapon_type`, add `robe_view`. No GM equip bypass. Dead equip/take-off: `0x0080` type 1 to the area, no ack. Unknown item ids: no ack. Dual wield only through the Assassin/Kagerou-class rule. |

## Notes for the implementer

### Git state at hand-off
- M0's docs (this spec and plan, `PACKET_FINDINGS.md`, `reference/catalog/`) are committed in
  "Equipment: M0 catalog, spec and plan" (CLAUDE.md allows M0 docs on their own). The tree was clean after it.
  M1 onwards stays uncommitted until Finish.
- The new capture (checkpoint below) doesn't exist yet. If session C starts without it, ask the user before
  M7 instead of building from source reading alone.

### Capture facts (`reference/catalog/captures/packet_sequence-20261023_030246.txt` lines 841–865)
- Sessions: `#6` = account 2000001 / char 150006 (`test2`, novice, STR 1), the equipper; `#3` = account 2000000
  (`test`), the observer.
- The proxy table then had `0x01d7` at 11 and no `0x0999`/`0x099a` rows, so frames are split wrongly. The
  hand-framed bytes are in `reference/catalog/packets/equipment.md` "Captured weapon swap". Use them as the
  PKT-EQ-02/03 vectors and the ENG-EQ-03 order.
- At login `#6` wore Knife (1201) at wire 2: `0x0b39` entry `02 00 | b1 04 00 00 | 05 | 02 00 00 00 |
  02 00 00 00`. The `0x09ff` other players got for `#6` had weapon `b1 04 00 00`.
- `0x0999` for a weapon has view 0. `SP_ATK2` = 17 = the Knife's `Attack`. No `0x013a`, weight or skill
  packets follow.

### Capture checkpoint (after M1, the user records it)
The current proxy can't frame `0x0999`, `0x099a`, `0x0bae`, `0x013c`, `0x013b` or `0x0291`, so record only
after M1 lands (rebuild the proxy). Against `./run_rathena_servers.sh`, two clients on one map, a `group_id 99`
account, the other observing. Set one character to Archer in rAthena's DB for the ammo steps.
1. Log in wearing a Knife, and a second time wearing arrows + a bow (the `0x0983`/`0x013c` entry position).
2. `@item` a Knife, a Guard (2101), a two-hander (e.g. 1116 Katana), a headgear with `View` (e.g. 2220 Hat),
   a Top+Mid headgear, a garment, shoes, a level-limited item.
3. Equip the knife, then the Guard, then the two-hander (displaces both), then take it off with `0x00ab`.
4. Equip and take off the hat, garment and shoes. Try the level-limited item (result 1).
5. Archer: equip the bow, equip arrows, take off the bow (cascade), equip arrows with no bow (`0x0291` 1691).
6. Unequip all from the equipment window.
7. Drop a worn knife. Drop part of the worn arrows.
8. Relog and look at char select with gear on (does the weapon show? `char.md` Q4).

Commit the capture under `reference/catalog/captures/` with a README row (redact PINs and `0x008e` text), then
check it against the spec before M7: list every difference, fix the spec and catalog (the capture wins), and
update PF-11.

### Catalog facts gathered in M0 (all written into the catalog)
- Job/class/gender checks, job id → mapid bits, `Jobs` name → `(index, bit)`, renewal `Classes` table:
  `mechanics/equipment.md` §3a. Level gives result 1; gender, job and class give 2. Level beats gender, gender
  beats job and class.
- Char-select fields (`packets/char.md` "Per-Character Look Fields"): `weapon` (offset 90, i16) is the `W_*`
  **type**, not an item id. `shield` (98) is the left armor's `View`. Accessory/2/3 are bottom/top/mid views.
  The robe goes in `robePalette` (162, i32). The entry struct is 175 bytes.
- Entry packets use item ids/aliases for weapon/shield, never the type (`vd.look` vs `status.weapon`).
- Equip doesn't trigger a save; the look columns persist with the next save.
- Ammo: `0x0983` (29 bytes, EFST 695 by counting the enum, state 1, total = remain = 9999, vals 0) to the
  area, then `0x013c`, `0x013b` type 3. Nothing ever ends the EFST. At login `0x0983` + `0x013c` sit between
  the normal and equip lists.
- Take-off failures: out of range → `0x099a {2, 0, 1}`; not worn → `{index, 0, 1}`. Dead is silent apart from
  the clear-unit.
- Unequip-all recalculates once per item. `0x0bae` comes last.
- Sitting blocks nothing here.
- Drop of a worn full stack: `pc_unequipitem(2|1)` inside `pc_delitem`, after `0x0add` and before `SP_WEIGHT`
  / `0x00af`. A partial drop keeps the item worn.
- `0x0291` ZC_MSG = `{u16 op, u16 msg id}`, self. `MSI_FAIL_NEED_EQUIPPED_BOW` 1691, `MSI_WRONG_BULLET` 2493
  (PF-18 resolved).
- **Fixed in M0:** the explorer's note said displacement uses flag `1|2|4` with "no calc". Flag 1 *is* the
  recalc, and the capture shows the push, so the catalog now says each displaced item gets its own push.

### Existing code anchors (line numbers as of 2026-10-24)
- **`crates/ro-packets/src/table.rs`:** has `0x00ab` 4 (:66), `0x0998` 8 (:837), `0x0bf5` 6 (:1030),
  `0x0983` 29 (:835), `0x0196` 9 (:196). **Missing:** `0x0999`, `0x099a`, `0x0bae`, `0x013c`, `0x013b`, `0x0291`.
  Check `0x013a` and `0x07e6`/`0x09cd` while there (PF-21).
- **`crates/ro-packets/src/outgoing/map.rs`:** `put_unit_head` :93 writes weapon/shield/accessory as 0
  (:106-108); `put_unit_middle` writes accessory2/3 and robe as 0 (:113-118). Feed them from
  `UnitAppearance` (`crates/ro-core/src/message.rs:340`).
- **`crates/ro-packets/src/outgoing/char.rs`:** char entry writes weapon :33 and shield :37 as 0,
  accessory/accessory2/3 0, `robePalette` 0 (:68). Source: `CharacterSummary` (`crates/ro-core/src/ids.rs:30`).
  There are 8 files with `CharacterSummary {` literals (ro-core ids.rs + vocabulary tests, persistence
  character.rs, ro-packets encoder.rs + char.rs + framing tests, map-server claim.rs + tests/common).
- **`crates/ro-db/src/item.rs`:** `ItemType` :32, `ItemData` :97. **`tools/item-import/src/lib.rs`:** `RawItem`
  :173, `merge` :220, `item_type` :252, `location_bits` :270, `build` :315.
- **`crates/ro-engine/src/`:**
  - `engine.rs`: `appearance` :210, `entry_event` :324, `handle_intent` :354 (the match from :364),
    `actor_init` :418 (calls `send_weapon_look` at :421), `send_inventory` :450, `spawn_player` :471.
  - `inventory.rs`: `revision` :100, `add` :167 (bumps :248), `remove` :253 (bumps :273), `hand_looks` :111
    (already returns the `LOOK_WEAPON` pair; reuse for `UnitLooks`).
  - `status.rs`: `PUSH_DERIVED_PARAMS` (zeros), `BARE_HAND_RANGE`, `send_login_status_push`,
    `send_initial_status`, `status_window`. The hook replaces the zeros with stored `DerivedStatus` values.
  - `settings.rs:106` `send_weapon_look` (self only at entry; leave it).
  - `floor_item.rs:230` `drop_item` (worn-item path goes in here or in `inventory.remove`).
  - `world.rs:31` `PlayerInfo` (has `job`, `sex`, `clevel`; no looks yet).
- **`crates/ro-persistence/`:** `sql/01_schema.sql` :50-54 (`weapon_view` … `head_bot_view`, unused today),
  the advisory lock and guarded `DO` block from items M5. `repository/inventory.rs:26` `save_inventory`,
  `postgres/inventory.rs:235`. `postgres/character.rs:163` `list_by_account`.
- **`bins/map-server/src/claim.rs:45`** `WriteKind::Inventory(CharacterInventory)`: carry the looks too.

### Pitfalls
- **Revision bump:** `equip` changes mutate items in place, not through `add`/`remove`. Bump the inventory
  revision on every equip change, or autosave skips it (ENG-STAT-03).
- **Two pushes per swap:** the displaced item recalculates on its own before the new `0x0999`. Don't batch
  them into one recalc; the capture pins this.
- **Look ack order:** on unequip the look comes before `0x099a`; on equip `0x0999` comes before the look.
- **Two-hander unequip** sends two identical `0x01d7` (one per hand). Don't dedupe.
- **Weapon type vs look:** the char-select `weapon` is the `W_*` type; entry packets and `0x01d7` use the item
  id / alias. Keep two distinct types (`CharacterLooks` vs `UnitLooks`).
- **Hot path:** equip is intent-driven. Entry events read the cached `UnitLooks`; never scan the inventory or
  `ItemDb` while building an entry event in `step_visibility`.
- **Mob-dropped equipment is unidentified** (items M8 ruling), so it fails with result 2 until identification
  exists. Test with `@item`, which gives identified items.
- **Job ids with no mapid** (mounted ids like 13) act as Novice, as rAthena resets them.
- **Data regeneration:** bump `ITEM_DATA` to v2, regenerate all three files, update `NOTICE.md`; the generated
  data may be its own commit (items plan). Pass an expanded path to `--rathena` (`$RATHENA_PATH` may hold `~`).
- **Never paste message text**; `0x0291` carries numbers only.

## Session split (recommended)
Each session reads this file, does its milestones, runs `./verify.sh -p <crates>`, ticks the boxes, and adds
rulings under "Implementation notes". Work stays uncommitted until Finish (generated data may commit early).

| Session | Milestones | Why this cut |
| :--- | :--- | :--- |
| **A: wire and data** | M1, M2, M3 | Mechanical and leaf-level (ro-packets table, importer + ro-db, ro-core vocabulary + codecs). No engine context needed. M1 first so the user can record the capture while the rest runs. |
| **B: persistence and rules** | M4, M5, M6 | Schema + char select, then pure rule functions and the status hook. Each is testable alone; they're the foundation M7 builds on. |
| **C: engine flows** | Capture check, M7, M8 | The core: needs the capture and the rules fresh in context. The heaviest session. |
| **D: wiring and finish** | M9, Finish | BIN tests, reviewer over the full diff, fixes, DEFERRED, smoke, manual check, commit. The reviewer needs a fresh context. |

If a session runs short, cut at a milestone boundary; never mid-milestone.

## Implementation notes (M1 onwards)
Rulings made during implementation, and review results later milestones must honour.

- **M1 (session A, 2026-10-25):** besides the six rows, added `0x07e6` 8 and `0x09cd` 8 (catalog
  `packets/equipment.md` "Siblings"; `0x07e2` 8 was already there). `pkt_eq_01_table_lengths` in
  `crates/ro-packets/tests/framing_tests.rs` pins all spec PKT-EQ-01 lengths plus those two and `0x013a`.
  PF-21's missing list and PF-18's "still to add" note are updated. The proxy was rebuilt; the capture
  checkpoint is unblocked.

## Milestones
Write tests first, named with spec IDs. Iterate with `./verify.sh -p <crate>`.

- [x] **M0 research, catalog, spec, plan** (2026-10-24)
  - Found the swap capture (`packet_sequence-20261023_030246.txt` lines 841–865) and re-framed it in
    `packets/equipment.md` "Captured weapon swap"; `captures/README.md` row and `INDEX.md` rows updated.
  - `PACKET_FINDINGS.md`: PF-11 partly settled (lengths, displacement order); PF-16 evidence (440/590);
    PF-18 resolved from source (2493); PF-21 gets `0x099a`.
  - `metaling-explorer` added §3a job/class/gender, char-select look fields (`char.md`, `sql_schemas.md` fix),
    `status_change.md` (`0x0983`/`0x0196`, EFST 695), take-off/drop order, `0x00ab` failures, unequip-all,
    `0x0291` and the `MSI_*` values.
  - This spec and plan.
- [x] **M1 ro-packets table** *(session A, 2026-10-25)*
  - Add `0x0999` 11, `0x099a` 9, `0x0bae` 3, `0x013c` 4, `0x013b` 4, `0x0291` 4. PKT-EQ-01.
  - Trim PF-21's list. Rebuild `tools/proxy` and tell the user the capture checkpoint is unblocked.
- [ ] **M2 item data v2** *(session A)*
  - ro-db: `WeaponType` (with `W_*` numbers), `AmmoType`, `Gender`, the new `ItemData` fields, v2 header only.
    DB-ITEM-04/05.
  - `tools/item-import`: parse `SubType`, `Attack`, `MagicAttack`, `Defense`, `Range`, `WeaponLevel`,
    `ArmorLevel`, `EquipLevelMin/Max`, `Gender`, `Jobs`, `Classes`, `Refineable` with the type-clearing and
    forced-gender rules; the `Jobs` name table from catalog §3a. TOOL-ITEM-06..08, TOOL-ITEM-05 updated.
  - Regenerate `crates/ro-db/data/item/`, update `NOTICE.md`, DB-ITEM-03 updated. `ro-db/DESIGN.md`.
- [ ] **M3 ro-core vocabulary + ro-packets codecs** *(session A)*
  - ro-core: intents `EquipItem {index, position}`, `UnequipItem {index}`, `UnequipAll {location}`; events for
    `0x0999`, `0x099a`, `0x0bae`, `0x013c`, `0x013b`, `0x0291`, `0x0983` (generic status-icon start);
    `UnitLooks` on `UnitAppearance`; `CharacterLooks` on `CharacterSummary` (8 literal files).
  - ro-packets: decoders `0x0998`, `0x00ab`, `0x0bf5`; encoders for the new events; entry packets and the
    char entry read the looks. PKT-EQ-02..07.
- [ ] **M4 ro-persistence + char select** *(session B)*
  - Schema: rename `weapon_view` → `weapon_type`, add `robe_view` (guarded `DO` block). Summary queries load
    `CharacterLooks`. `save_inventory(char_id, inventory, looks)` writes them in the same transaction.
  - PER-EQ-01/02. `ro-persistence/DESIGN.md`.
  - No early review: the schema change is two columns. Review early only if the repository API grows beyond
    this.
- [ ] **M5 engine rules (pure)** *(session B)*
  - `job.rs` (job id → mapid, branch, upper/baby/third/fourth), `equipment.rs` rule functions: equip point,
    `pc_isequip` subset, final-mask resolution, ammo pairing, weapon-type resolution, `UnitLooks` /
    `CharacterLooks` from a worn set (costume override). ENG-EQ-01, ENG-EQ-02.
- [ ] **M6 engine status hook** *(session B)*
  - `DerivedStatus` stored per player, `GearStatus`, `recalc(e)` with the ordered diff push; login push and
    `0x00bd` use stored values; spawn builds them from the worn set. ENG-STAT-01/02.
- [ ] **Capture check** *(session C start)*: compare the user's new capture with the spec (see checkpoint).
  Fix the spec and catalog first.
- [ ] **M7 engine equip flows** *(session C)*
  - `EquipItem` / `UnequipItem` / `UnequipAll` handling: gates, displacement, acks, looks to the area,
    `UnitLooks` cache refresh, revision bump, recalc. Entry events use `UnitLooks`.
  - ENG-EQ-03..08, ENG-EQ-11, ENG-STAT-03. `ro-engine/DESIGN.md`.
- [ ] **M8 engine ammo + worn-item removal** *(session C)*
  - Ammo equip acks, cascade on weapon removal and on a mismatched weapon, map-entry `0x0983` + `0x013c`;
    worn-item unequip inside remove/drop. ENG-EQ-09, ENG-EQ-10, ENG-EQ-12.
- [ ] **M9 map-server wiring** *(session D)*
  - Claim writer carries looks; engine API returns the looks with the inventory snapshot; char-server list
    sends them; fakes updated. BIN-EQ-01..03.
- [ ] **Finish** *(session D)*
  - Run the `reviewer` over `git diff HEAD` plus untracked files; apply its fixes.
  - `DESIGN.md` for ro-engine, ro-db, ro-persistence. `DEFERRED.md`: an "equipment" section from spec §5;
    remove the items-section equip bullets (lines ~79-80, ~87) that this feature closes.
  - `./verify.sh --smoke`.
  - **Manual client check:** `@item` a knife, a guard, a hat; equip, swap, take off, unequip all; the observer
    sees looks; relog keeps gear; char select (weapon shown?); an Archer (SQL job edit) with bow + arrows.
  - Commit.
