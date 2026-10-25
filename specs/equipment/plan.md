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
- **Exception:** M1 was committed and pushed on its own (`4fe67f1`) so the user could build the proxy on another
  machine. At Finish, the reviewer's diff is `git diff 6be7d91 -- . ':!tools/proxy'` plus untracked files, not
  `git diff HEAD`. `88b9b71` (proxy diagnostics + `0x080e`) is outside this plan.
- `handoff-A.md` (session A) is folded into this file and the catalog, and deleted.
- **Session A2 (2026-10-25)** did the capture check, M2 and M3. Its docs (both captures, catalog,
  `PACKET_FINDINGS.md`, this spec and plan) are committed on their own as "Equipment: capture check docs".
  The **M2 and M3 code and the regenerated item data are uncommitted** in the working tree, together with
  `crates/ro-db/data/NOTICE.md` and `crates/ro-db/DESIGN.md` (they describe that code and data). They stay
  uncommitted until Finish like the rest; the Finish diff above already covers them.

### Session B: start here
1. `git status` should show the M2/M3 code, the regenerated `crates/ro-db/data/item/*.yml` and the new
   `crates/ro-packets/tests/equip_codec_tests.rs` as uncommitted. Don't revert or re-do them.
2. **Run the DB-backed tests first** (`ro-persistence`, `bins/*`); session A2 couldn't (see the M3 note).
   The metaling and rAthena compose files share the project name "docker", so
   `docker compose -f tools/docker/docker-compose.yml up -d db` replaces a running `rathena-db`. Ask the user
   before starting it if `rathena-*` containers are up (`docker ps`). Then `./verify.sh` (all crates).
3. Capture 3 (`equip_sequence_3.txt`) arrived and was checked in session B; its docs are committed on their
   own as "Equipment: capture 3 check docs".
4. Read the "Implementation notes" for M2/M3 before touching ro-db, ro-core or ro-packets types.

### Captures (all checked against the spec on 2026-10-25)
Byte-level write-up: catalog `packets/equipment.md` "Captured weapon swap" and "Captured equip sequences". Use
those bytes as the PKT and ENG test vectors.
- `packet_sequence-20261023_030246.txt` lines 841–865: Knife-for-Knife swap (ENG-EQ-03, PKT-EQ-02/03). The
  proxy table then misframed it; use the hand-framed bytes.
- `equip_sequence.txt` (capture 1): one client, an Archer via `@job archer`. Shield, two-hander, headgear, shoes,
  level/job failures, ammo, login with arrows.
- `equip_sequence_2.txt` (capture 2): equipper `#6`/`#8`/`#10` (account 2000000, `test`, Archer) plus observer
  `#3`, full payloads. Garments (Hood, then Bravery Bag, which has a view), unequip-all, entry looks, char-select
  looks. No shoes or shield in it (capture 1 has them).
- **What changed in the spec from them** (the capture won each time):
  - §2.2 step 10: the new item takes each position as the `EQI` loop reaches it, so a later displaced item's look
    and push already count it. Build the worn-slot table up one position at a time and run each displaced
    item's recalc against it. Two-hander that holds only `HAND_L` so far: counts once, like any item.
  - §2.2 step 13: one `LOOK_WEAPON` per hand in the final mask (a two-hander sends two).
  - Head looks: a multi-slot headgear's lower slots are 0 by its own `Locations`, not by the slot table (no
    `LOOK_HEAD_MID` ever for Goggles).
  - §2.7: the login `0x0983` goes to self only (our ruling; rAthena's area copy arrives before the observer
    knows the unit).
  - §2.8: the client never sends a drop for a worn item. ENG-EQ-10 is a unit test only; not in the manual check.
  - §2.10 gap: rAthena also pushes ASPD on shield changes and ATK1 on bow changes.
  - §2.11: char select `weapon` = 1 for a Knife; the client draws no weapon/shield there (only in combat stance).
- `equip_sequence_3.txt` (capture 3, session B, 2026-10-25): equipper `#5`/`#8`/`#15` (`test`, `@job 12`
  Assassin), observer `#6`/`#12`/`#18`. Dual wield, a katar (Jur), Tsurugi + Guard, three deaths, the observer
  walking in and out of view, unequip-all, arrow failures, char select. Write-up: catalog `packets/equipment.md`
  "Dual wield, katar and shield (capture 3)". **What it settled** (the spec now says so):
  - Hand choice (§2.2 step 9): right free → right; right taken, left free → left; both taken → right. A
    one-hand request (`0x02`) goes right: the client's inventory `location` isn't refreshed on `@job`.
  - Looks: a left-hand weapon's item id goes in `shield` (`LOOK_WEAPON` and entry packets). Char select:
    left-hand weapon → `shield` 0, `weapon` = dual constant (Tsurugi + Damascus = 28 `W_DOUBLE_DS`); Guard →
    `shield` 1 (its `View`). Entry packet with a Guard: `shield` 2101.
  - ATK2 sums both hands; a two-hander counts once, including mid-displacement while it holds only `HAND_L`.
  - Death sends no look change; a dead player's entry packet keeps its weapons (`state` 1). Don't clear
    `UnitLooks` on death. The client hides them, and never shows an observer the combat stance on entry.
  - Gap until stats: a katar pushes `SP_CRITICAL` (2 ↔ 4) in rAthena; not here.
  - The `#8` log stalled after `@jlevel 99` (PF-21), so the reply to the arrow request at line 2978 is missing.
- Not captured (source reading): `0x099a` failure forms, dead gates (no request was sent while dead), costumes,
  bullets.
- Ammo requests come with position `0` or `0x8000`; `0x0bf5` from the window is `ff 83 00 00`.
- Bravery Bag's `0x010f`/`0x0441` come from its item script (skill grant). Out of scope; don't send them.
- The capture-1 client freeze didn't repeat in capture 2 (PF-21 note). Nothing for metaling to copy.
- Inventory indexes shift between sessions in the captures (rAthena re-packs on load); match items by id.
- **Shield in an entry packet** means the `shield` field (offset 31) of the `0x09fd`/`0x09fe`/`0x09ff` packets
  an observer gets when the wearer comes into view. The captures only ever show 0 there (no one wore a shield
  while an observer saw them enter); the live `LOOK_WEAPON` {1201, 2101} for a Guard is captured. It's a
  byte check, not a visual one: the client draws weapons and shields only in combat stance (user), on the
  map and at char select alike.

### Capture follow-ups (optional; told to the user 2026-10-25)
**Items 1 and 2 are done by capture 3** (above); item 3 only partly (deaths, but no equip request while dead).
The original list:
1. **Dual wield** (M5 rules, M7 looks and status, M9 char select): an Assassin (`job_class` 12) with two
   daggers. Equip one, then a second (which hand does it take? spec §2.2 step 9, catalog §1 "dual-wield hand
   choice"), swap one, take each off, unequip all; an observer watching; relog to char select (the `weapon`
   field should be a dual constant, `DOUBLE_DD` 25). Highest value: the hand choice and the look values are
   pure source reading today.
2. **Shield in an entry packet** (M7, ENG-EQ-11): wear Knife + Guard, the observer walks out of view and back,
   and the wearer relogs while observed. Cheap; source is clear (`shield` = 2101).
3. **Dead gates** (M7, ENG-EQ-06): die, then try to equip and take off. Shows whether the client even sends
   `0x0998`/`0x00ab` while dead. Cheap but low value: the spec's answer (clear-unit, no ack) is harmless.
4. **Bullets** (M5 pairing): skip. Needs a Gunslinger and a gun, and it only checks `0x0291` 2493, the same
   path as arrows' 1691.
The take-off failure forms can't be captured: the client never asks to take off an item that isn't worn.

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
| **C: engine flows** | M7, M8 | The core: needs the captures and the rules fresh in context. The heaviest session. (The capture check ran early, in session A2.) |
| **D: wiring and finish** | M9, Finish | BIN tests, reviewer over the full diff, fixes, DEFERRED, smoke, manual check, commit. The reviewer needs a fresh context. |

If a session runs short, cut at a milestone boundary; never mid-milestone.

## Implementation notes (M1 onwards)
Rulings made during implementation, and review results later milestones must honour.

- **M1 (session A, 2026-10-25):** besides the six rows, added `0x07e6` 8 and `0x09cd` 8 (catalog
  `packets/equipment.md` "Siblings"; `0x07e2` 8 was already there). `pkt_eq_01_table_lengths` in
  `crates/ro-packets/tests/framing_tests.rs` pins all spec PKT-EQ-01 lengths plus those two and `0x013a`.
  PF-21's missing list and PF-18's "still to add" note are updated. The proxy was rebuilt; the capture
  checkpoint is unblocked.
- **M2 (session A2, 2026-10-25):**
  - `ro_db` exports `WeaponType` (24 item subtypes, `id()` = `W_*`; no dual-wield constants, those are
    the engine's resolved type in M5), `AmmoType` (`id()` = `AMMO_*`), `Gender`, `ALL_JOBS`,
    `ALL_CLASSES`. `ItemData` has the new fields flat (YAML keys as spec §1) and now implements
    `Default` (`Etc`, id 0, all jobs/classes, override 100); test builders use `..ItemData::default()`.
  - `ItemType` derives `Default` (`Etc`).
  - `ro-db`'s YAML loader raises serde-saphyr's node budget (250k → 2M nodes, 8M events) in
    `yaml.rs` `parse_options`: `item_equip.yml` v2 is ~310k nodes. Applies to every ro-db dataset.
  - Importer: `Jobs`/`Classes` keep file order (`Switches`); `All` in `Jobs` applies first; a later
    `Jobs` replaces, a later `Classes` edits (an earlier absent `Classes` counts as `All`). Unknown job
    or class names allow everything, with a warning. The real import gives **0 warnings**.
  - Spec fix from the catalog: `Defense` is kept only for Armor/ShadowGear; a `SubType` on a type other
    than Weapon/Ammo/Card skips the entry.
  - Data regenerated (`item_equip.yml` grew from ~91k to ~156k lines); `NOTICE.md`, `ro-db/DESIGN.md`
    updated. Values match the captures (Bow ATK 15 / range 5, Guard DEF 20, Goggles 5, Bravery Bag 20).
- **M3 (session A2, 2026-10-25):**
  - ro-core intents: `EquipItem { index, position }`, `UnequipItem { index }`, `UnequipAll { location }`
    (wire index kept, like `DropItem`).
  - ro-core events: `EquipAck { index, location, view, result }` (0x0999), `UnequipAck { index,
    location, failed }` (0x099a), `UnequipAllAck { failed }` (0x0bae), `AmmoEquipped { index }`
    (0x013c), `ActionFailure { failure_type }` (0x013b), `ClientMessage { msg_id }` (0x0291),
    `StatusIconStarted { efst, gid, state, total_ms, remain_ms, values }` (0x0983, generic). Looks keep
    using the existing `SpriteChanged`.
  - `UnitLooks { weapon: u32, shield: u32, head_bottom, head_top, head_mid, robe: u16 }` is
    `UnitAppearance.looks`; `CharacterLooks { weapon_type, shield, head_top, head_mid, head_bottom,
    robe: u16 }` (serde) is `CharacterSummary.looks`. Both `Default` = bare. Every existing literal
    (engine `appearance`, persistence `list_by_account` row mapping, map-server `claim.rs` test and
    `tests/common`) passes `::default()` for now: **M4 must load `CharacterLooks` in persistence,
    M7 must fill `UnitLooks` in `engine.rs` `appearance` (~:215-265) from the cache.**
  - ro-packets: decoders `CzReqWearEquipPacket` / `CzReqTakeoffEquipPacket` /
    `CzReqTakeoffEquipAllPacket` in `incoming/map.rs` and `decode_frame`; encoders in the new
    `outgoing/equip.rs`, plus `encode_zc_msg_state_change3` in `outgoing/status.rs`; opcode consts
    added (`ZC_REQ_WEAR_EQUIP_ACK`, `ZC_REQ_TAKEOFF_EQUIP_ACK`, `ZC_ACK_TAKEOFF_EQUIP_ALL`,
    `ZC_EQUIP_ARROW`, `ZC_ACTION_FAILURE`, `ZC_MSG`); all reported by `is_implemented_outbound_opcode`.
    Entry packets and `encode_character_info` write the looks. Tests:
    `crates/ro-packets/tests/equip_codec_tests.rs` (PKT-EQ-02..07, captured bytes).
  - Not run this session: the DB-backed tests (`ro-persistence`, `bins/*`). The metaling DB wasn't up,
    and `docker compose ... up -d db` would have replaced the user's running `rathena-db` (shared
    compose project "docker"). They compile and pass clippy; run them at the start of session B.

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
- [x] **Capture check** *(session A2, 2026-10-25; moved ahead of M2)*
  - Both captures redacted and committed under `reference/catalog/captures/` with README rows.
  - Catalog: `packets/equipment.md` "Captured equip sequences", `mechanics/equipment.md` §1/§3/§5, `char.md`
    (Q4 settled), `status_change.md` (EFST 695 confirmed), `INDEX.md`. PF-11 resolved; PF-21 gets four
    missing rows and the freeze note. Spec fixed (see "Captures" above).
- [x] **M2 item data v2** *(session A2, 2026-10-25)*
  - ro-db: `WeaponType` (with `W_*` numbers), `AmmoType`, `Gender`, the new `ItemData` fields, v2 header only.
    DB-ITEM-04/05.
  - `tools/item-import`: parse `SubType`, `Attack`, `MagicAttack`, `Defense`, `Range`, `WeaponLevel`,
    `ArmorLevel`, `EquipLevelMin/Max`, `Gender`, `Jobs`, `Classes`, `Refineable` with the type-clearing and
    forced-gender rules; the `Jobs` name table from catalog §3a. TOOL-ITEM-06..08, TOOL-ITEM-05 updated.
  - Regenerate `crates/ro-db/data/item/`, update `NOTICE.md`, DB-ITEM-03 updated. `ro-db/DESIGN.md`.
- [x] **M3 ro-core vocabulary + ro-packets codecs** *(session A2, 2026-10-25)*
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
  - Make looks and gear status functions of a **worn-slot table** (position → slot), not of the items' `equip`
    masks: M7's displacement runs recalcs while the new item holds only some of its positions (spec §2.2
    step 10). Head views use each item's own `Locations` for the multi-slot rule.
- [ ] **M6 engine status hook** *(session B)*
  - `DerivedStatus` stored per player, `GearStatus`, `recalc(e)` with the ordered diff push; login push and
    `0x00bd` use stored values; spawn builds them from the worn set. ENG-STAT-01/02.
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
