# Specification: Equipment (equip, unequip, ammo, looks, gear status hook)

**Scope:**
- Players can equip and unequip worn gear and ammo from the equipment window, and take everything off at
  once.
- Other players see weapon, shield, shoes, headgear and garment changes, both live and in entry packets.
- Worn gear persists. The char-select screen shows it.
- After every equip change the server recalculates status and sends only the values that changed. In this
  feature the only inputs are the gear values (weapon ATK, hard DEF, attack range). The stats feature fills in
  the formulas behind the same hook.
- Item data gains the equipment fields (weapon/ammo type, ATK, DEF, range, level, gender, jobs, classes).

Out of scope (see §5):
- item scripts (`Script`, `EquipScript`, `UnEquipScript`, `bonus`), cards and combos. (Capture 2's Bravery Bag
  sends `0x010f` and `0x0441` around its push because its script grants a skill; we send neither.)
- refine and enchant-grade bonuses
- bind-on-equip
- the equip-switch window (`0x0a9b` stays empty)
- pet armor
- status effects that block equip (Strip, etc.) and map `NoEquip` restrictions
- re-checking worn gear after a level or job change (neither exists yet)
- the skill `AS_LEFT` (dual wield comes only from the Assassin-class rule)
- stat formulas: ASPD, status ATK/MATK, soft DEF, HIT, FLEE, CRIT, MaxHP/SP (the stats feature)

**Ground truth:** rAthena commit `d4b8e7b` at `PACKETVER = 20260219`, as recorded in the catalog:
- `reference/catalog/packets/equipment.md`: every equip packet, the **captured weapon swap**, take-off and drop
  order, `0x00ab` failure acks, unequip-all, `0x013c` / `0x013b`, `0x0291` messages, look packets.
- `reference/catalog/mechanics/equipment.md`: slot model §1, item fields §2, `pc_equipitem` §3, job/class/gender
  §3a, `pc_unequipitem` §4, views §5, status effect §6, persistence §7.
- `reference/catalog/packets/status_change.md`: `0x0983` and EFST 695 (ammo icon).
- `reference/catalog/packets/status.md` "Recalculation pushes": the diff rule and push order.
- `reference/catalog/packets/char.md` "Per-Character Look Fields": char-select look fields.
- `reference/catalog/mechanics/inventory.md` §4 and §6: deleting or dropping a worn item.
- `reference/catalog/db/item_db.md`: `SubType`, `Attack`, `Defense`, `Range`, `Jobs`, `Classes`, `Gender`,
  `EquipLevelMin/Max`.
- Capture `reference/catalog/captures/packet_sequence-20261023_030246.txt` lines 841–865 (Knife swap).
- Captures `equip_sequence.txt` and `equip_sequence_2.txt` (2026-10-25), written up in catalog
  `packets/equipment.md` "Captured equip sequences": take-off, unequip-all, failures, a shield, a two-hander,
  headgear, garments, shoes, ammo, entry looks, char-select looks.
- Capture `equip_sequence_3.txt` (2026-10-25, catalog "Dual wield, katar and shield (capture 3)"): an Assassin's
  dual-wield hand choice, looks and pushes, a katar, a shield in entry packets, deaths, char select `W_DOUBLE_DS`.
- `PACKET_FINDINGS.md` PF-11 (resolved), PF-16, PF-18 (resolved), PF-21.
- Capture `equip_sequence_4.txt` (2026-10-25, catalog "Dead, costumes, accessories and refine (capture 4)"):
  requests while dead, accessories, costumes, low/mid/top headgear, broken gear, gender, sitting, a two-hander
  over a shield, arrows with a katar, refine and MATK/MDEF pushes, char select `W_KATAR`/`W_MUSICAL`.
- **Still source-only:** the `0x099a` failure forms, bullets.

When this spec and the catalog disagree, the catalog wins and this spec gets fixed.

---

## 0. Data pipeline

```text
$RATHENA_PATH/db/re/item_db_equip.yml (+usable, etc) ──► tools/item-import (v2) ──► crates/ro-db/data/item/*.yml (GPL, ITEM_DATA v2)
                                                                                    │ ro_db::ItemDb (loaded once, Arc)
                                                                                    ▼
           ro_engine::MapInstance: equip rules, gear status, looks ◄── inventory items' `equip` masks
PostgreSQL inventory.equip (exists) + characters look columns ◄──► ro_persistence (save_inventory + looks)
```

No new runtime directory: equipment fields live in the item files.

## 1. `ITEM_DATA` v2

`Header.Version: 2`. `ro-db` accepts only version 2 (no backwards compatibility; the data is regenerated).
New optional keys, omitted when default:

```yaml
  - Id: 1201
    AegisName: Knife
    Name: Knife
    Type: Weapon
    Weight: 400
    Locations: 0x2
    Slots: 3
    WeaponType: Dagger     # Weapon only: W_* name (catalog mechanics/equipment.md §2 "Weapon type enum")
    AmmoType: Arrow        # Ammo only: AMMO_* name (catalog §1 "Ammo")
    Attack: 17             # Weapon and Ammo
    MagicAttack: 0         # Weapon; imported, unused
    Defense: 0             # Armor and ShadowGear
    Range: 1               # Weapon; capped at 14
    WeaponLevel: 1         # Weapon, 1..=5
    ArmorLevel: 1          # Armor
    EquipLevelMin: 1       # 0 = no minimum
    EquipLevelMax: 0       # 0 = no maximum
    Gender: Both           # Female | Male | Both, after rAthena's forced values
    Jobs: [0xffffffffffffffff, 0xffffffffffffffff, 0xffffffffffffffff]  # class_base[3]; omitted = all
    Classes: 0x7f          # ITEMJ_* mask; omitted = all
    Refineable: true       # imported, unused
```

- **`Jobs`**: the importer resolves each job name to `(index, bit)` with the table in catalog §3a and builds the
  three bitsets the way rAthena does (reset to 0 when `Jobs` is present, then `All`, then each name; an
  unknown name makes the item usable by all jobs). An absent `Jobs` means all ones.
- **`Classes`**: ITEMJ bits from catalog §3a / `item_db.md`. Absent means every bit.
- **`Gender`**: forced values from catalog §3a ("Gender"): `W_MUSICAL` Male, `W_WHIP` Female, the two wedding
  rings.
- **Fields cleared by type**, as rAthena does (catalog `item_db.md`): `Attack` only for Weapon/Ammo,
  `MagicAttack`/`Range` only for Weapon, `Defense` only for Armor/ShadowGear, `SubType` only for Weapon/Ammo
  (Card subtypes are dropped). The final type decides (after the no-`Locations` demotion). A `SubType` on any
  other type skips the whole entry, as rAthena rejects it. `WeaponLevel` 0 becomes 1 on a weapon, `ArmorLevel`
  0 becomes 1 on armor.
- **`Jobs` resolution** normalizes names (case and underscores), so `Super_Novice` and `SuperNovice` match. The
  shipped data imports with no warnings.
- `ro-db` exposes them on `ItemData` with typed enums (`WeaponType`, `AmmoType`, `Gender`). `WeaponType`
  carries the `W_*` number (needed for the char-select `weapon` field and ASPD later).

## 2. Behaviour

### 2.1 State (catalog §1, §5, §7)
- **Source of truth:** the `equip` mask on each inventory item (already persisted). Nothing else is stored.
- **Derived per player, rebuilt at spawn and after every equip change** (not on the tick path):
  - the worn slot per position (rAthena `equip_index`), or a scan of at most 100 slots: implementer's call;
  - `UnitLooks`: weapon and shield client ids (`ViewId` or item id; shield 0 for a two-hander), head bottom /
    top / mid and robe view ids after the costume override (catalog §5). Entry packets read this cache, so
    building an entry event never touches the inventory;
  - looks are computed from the worn-slot table: weapon = the `HAND_R` item, shield = the `HAND_L` item unless
    it's the same item. A left-hand weapon goes in `shield` as its item id, like a shield (captured: Knife +
    Damascus = {1201, 1222}). A multi-slot headgear shows only in its highest slot, decided by the item's own
    `Locations` (Goggles, top + mid: top = 1, mid = 0, even while only its mid slot is assigned; captured);
  - `GearStatus` (§2.10).
- **Weapon type** (rAthena `status.weapon`): the right-hand `WeaponType`, or the left-hand one when the right
  is empty, or the dual-wield constant for a dagger/sword/axe pair (catalog §1 "Weapon type resolution";
  captured: Tsurugi + Damascus = `W_DOUBLE_DS` 28). `W_FIST` (0) when bare-handed.
- **Job identity:** a job id → mapid table (catalog §3a), in a new `ro-engine/src/job.rs`. Unknown ids act as
  Novice. Stats will reuse it.

### 2.2 Equip request (`0x0998`, catalog `packets/equipment.md` `0x0998` and mechanics §3)
Inputs: wire index (slot = index − 2), requested position mask. Checks in order; the first hit decides. A
failure `0x0999` is `{index, location 0, view 0, result}` (captured: `16 00 00 00 00 00 00 00 01`).

1. Dead: `0x0080` type 1 to the area including self (like the pickup gate), no ack. The client sends no
   `0x0998` while dead (capture 4), so this is safety only.
2. Slot out of `[0, 100)` or empty: no ack.
3. Not identified: `0x0999` result 2.
4. Unknown item id (not in `ItemDb`) or a type that can't be worn (only Weapon, Armor, Ammo, ShadowGear can):
   no ack. Pet armor: no ack (deferred).
5. Ammo: the requested position becomes `EQP_AMMO` whatever the client sent (it sends `0` or `0x8000`;
   both captured).
6. `pos` = the item's `Locations`, widened to `EQP_ARMS` for a dagger / one-hand sword / one-hand axe when the
   character's mapid is in the Assassin or Kagerou/Oboro line (catalog §1 "Two-handed and dual wield").
7. `pc_isequip` subset, in catalog §3a order:
   1. `EquipLevelMin` > base level, or `EquipLevelMax` set and < base level: result **1**.
   2. Gender mismatch: result 2.
   3. Broken (`attribute != 0`): result 2.
   4. Ammo pairing (catalog §1 "Ammo"): arrows need a bow, instrument or whip; bullets and shells need a gun;
      grenades need a grenade launcher; dagger ammo needs `AS_VENOMKNIFE` (no skills exist, so it always
      fails); cannonballs fail (no Mado Gear or cart). Shuriken, kunai and throwing weapons pass. Failure:
      `0x0291` with the catalog message id (1691 for arrows, 2493 for bullets/shells/grenades), then
      result 2.
   5. Job (`Jobs` bitset at the index for the mapid's branch, bit = base-class number): result 2.
   6. Class (`Classes` against the mapid's upper/baby/third/fourth flags, renewal table): result 2.
8. Fit: `pos & requested == 0`, or the item is already worn: result 2.
9. Resolve the final mask (accessory auto-slot, dual-wield hand choice; catalog §1). Two-handers keep
   `EQP_ARMS`. Hand choice for a request naming both hands (captured, capture 3): left only if the right hand is
   taken and the left is free, otherwise right (displacing the right-hand item). A request naming one hand uses
   it: the client sends its inventory list's `location`, which isn't refreshed on a job change, so a dagger listed
   before `@job` asks for `0x02` only.
10. **Take the positions one by one** (captured; catalog "The new item takes each slot as the loop reaches
    it"). Walk the final mask's positions in rAthena `EQI` order (catalog §1: accessories, shoes, garment, head
    low / mid / top, armor, `HAND_L`, `HAND_R`, costumes, ammo, shadow). For each one:
    1. if another item holds it, displace that item: §2.6 with recalc on, forced, no ammo cascade (rAthena
       flag `1|2|4`). It sends its own looks, `0x099a` and status push. An item covering two positions is
       displaced once;
    2. assign the position to the new item in the worn-slot table right away.

    So a later displacement's looks and push already count the new item in the positions assigned before
    it. Bow over Knife + Guard: the Guard's push has no bow; the Knife's look is {0, bow} and its push holds
    the bow's ATK2 and range. Goggles over a Hat: the Hat's push holds the Goggles' DEF, and nothing is
    pushed after the Goggles' `0x0999`. Gear status (§2.10) counts each item once, wherever it sits.
11. Ack:
    - ammo: `0x0983` (EFST 695, state 1, total = remain = 9999, vals 0) to the area, then `0x013c` {index} and
      `0x013b` type 3 to self. No `0x0999`.
    - otherwise: `0x0999` {index, final mask, view, result 0} to self. `view` = item `View` if the final mask
      intersects `EQP_VISIBLE` (head low/mid/top, garment, the four costume slots), else 0.
12. Set `equip` = final mask.
13. Looks, to the area including self:
    - one `0x01d7` `LOOK_WEAPON` {weapon id, shield id} per hand position in the final mask, so a two-hander
      sends two identical packets (captured);
    - shoes: `0x01d7` `LOOK_SHOES` {view id or item id};
    - head / garment / costume positions: `0x01d7` `LOOK_HEAD_BOTTOM`/`MID`/`TOP` or `LOOK_ROBE` only for
      the views that changed after the costume override (catalog §5).
14. Ammo cascade: an item went into a hand (a weapon, or a shield that leaves no weapon) and the worn ammo
    no longer pairs with the resulting weapon type → unequip the ammo (forced, no recalc of its own: only
    its `0x099a`).
15. Recalculate status and push the diff (§2.10).

Nothing changes on any failure before step 10.

**Captured order for a one-hand weapon swap** (catalog "Captured weapon swap"), which the tests pin:
self gets `0x01d7`(old off) → `0x099a`(old) → push → `0x0999`(new) → `0x01d7`(new) → push; an observer gets
the two `0x01d7` only. The other captured orders (two-hander, headgear, garment, ammo, unequip-all) are in the
catalog's "Captured equip sequences" with byte values; the ENG tests use them.

### 2.3 Sitting
Sitting doesn't block equip, take-off, unequip-all or drop (catalog "Sitting/dead gates").

### 2.4 Take-off (`0x00ab`, catalog mechanics §4 and `packets/equipment.md` "Take-off / drop order")
1. Dead: `0x0080` type 1 to the area, no ack. The client sends no `0x00ab` while dead (capture 4).
2. Slot out of range: `0x099a` {index **2**, location 0, flag 1}.
3. Slot empty or not worn: `0x099a` {requested index, 0, 1}.
4. Otherwise run §2.6 with recalc on and the ammo cascade on (rAthena flag 1).

### 2.5 Unequip all (`0x0bf5`, catalog "unequip-all")
- Dead: `0x0bae` result 1, nothing removed. **Deliberate divergence** (user ruling, 2026-10-25): rAthena has
  no dead check here, and capture 4 (line 353) shows a dead player's unequip-all running in full; we treat
  that as a bug. Result 1 is the form rAthena sends for its `pc_cant_act` gate (whose other states don't
  exist here).
- The request mask comes from the window: `0x83ff` from the equipment window, `0x3f7c00` from the costume
  window (costumes and shadow gear; capture 4 line 396).
- For each worn position in rAthena `EQI` order whose mask intersects the request: §2.6 with flag 1. A
  two-hander is removed once; ammo already removed by its weapon's cascade is skipped.
- Each item recalculates on its own, so the client sees one push per item.
- Then `0x0bae` result 0.

### 2.6 Unequip core (`pc_unequipitem`, in packet order)
1. Clear the item's positions from the derived state.
2. Right hand: `0x01d7` `LOOK_WEAPON` to the area. Left hand: another `0x01d7` `LOOK_WEAPON` (so a two-hander
   sends two identical packets). Shoes: `0x01d7` `LOOK_SHOES` 0.
3. `0x099a` {index, the item's `equip` mask, flag 0} to self.
4. Head / garment / costume: `0x01d7` for each view that changed.
5. Ammo cascade (only when on): a ranged weapon (bow, instrument, whip, guns, grenade launcher) left a hand →
   the worn ammo's `0x099a` {ammo index, `EQP_AMMO`, 0}, no recalc of its own, no EFST end.
6. `equip` = 0.
7. If recalc is on: §2.10.

### 2.7 Ammo
- Equip and pairing: §2.2.
- The ammo icon EFST is never ended, as in rAthena: unequipping ammo sends only `0x099a`.
  *(Planning default: mirror rAthena, including the area broadcast of `0x0983`.)*
- **Map entry:** when ammo is worn, `0x0983` + `0x013c` go out with the inventory lists, between `0x0b09` and
  `0x0b39` (captured twice). **To self only.** rAthena also sends that `0x0983` to the area, but observers get it
  before their entry packet for the player (capture 2 line 221 vs 222), so it can't show; metaling already sends
  the login `0x01d7` to self only for the same reason (`settings.rs` `send_weapon_look`). A player coming into
  view later gets no `0x0983` in rAthena either. The live equip's area `0x0983` stays.
- Using up ammo needs combat; out of scope here. When it comes, removing the whole worn stack unequips
  first (§2.8).

### 2.8 Removing a worn item (drop now; ammo use and other deletes later)
The client refuses to drop a worn item, or part of worn ammo ("must unequip first"), and never sends `0x0363`
(user report, capture 1). So this path is server-side safety against a stale or modified client, built from
rAthena's source; nothing captures it. Using up the last arrow in combat (capture 2 line 364) sends the ammo's
`0x099a` before `0x07fa`, which fits the same rule for the combat feature.

`remove_item` on a worn slot that reaches amount 0 runs §2.6 (forced, recalc on, ammo cascade on) before the
slot is cleared. A drop's order becomes (catalog `inventory.md` §6):
`0x0add` (area) → looks / `0x099a` / ammo `0x099a` / push → `SP_WEIGHT` → `0x00af`.
A partial drop of worn ammo leaves it worn and sends only `SP_WEIGHT` and `0x00af`.
Picking an item up clears `equip` on the copy (already true: `add_item` zeroes it).

### 2.9 Map entry and other players
- The inventory lists already carry `WearState` = `equip` (`0x0b39`) and the `0x01d7` `LOOK_WEAPON` at entry.
- Entry packets (`0x09fd`/`0x09fe`/`0x09ff`) send `UnitLooks`: weapon u32, shield u32, accessory (head bottom),
  accessory2 (head top), accessory3 (head mid), robe. Captured (catalog "Entry looks for the equipper"): a bow
  gives weapon 1701 and shield 0, Goggles accessory2 1 and accessory3 0, a Hat accessory2 16, the Bravery Bag
  robe 2. Capture 3: Tsurugi + Guard gives weapon 1119, shield 2101; Damascus R + Tsurugi L gives 1222, 1119.
- Death changes no looks: no `0x01d7` on death or revive, and an observer's entry packet for a dead player
  (`state` 1) still carries the weapons (capture 3 line 1540). The client hides them on a dead body. Entry
  packets carry no combat stance.
- The login status push and the `0x00bd` window use the gear values (§2.10).

### 2.10 Status hook (catalog `packets/status.md` "Recalculation pushes", mechanics §6)
- **`DerivedStatus`**: one value for every derived field in the push order (HIT, FLEE1, ASPD, SPEED, ATK1,
  DEF (hard), ATK2, DEF2 (soft), FLEE2, CRITICAL, MATK min/max, MDEF hard/soft, ATTACKRANGE, MAXHP, MAXSP, HP,
  SP, trait values). Stored per player.
- **This feature computes only the gear part:**
  - `atk2` = right-hand weapon `Attack` + left-hand weapon `Attack` (refine 0; no refine bonus yet). A
    two-hander counts once. Captured: Knife + Damascus 135, Tsurugi + Damascus 248, the Jur 125 both while it
    holds only `HAND_L` mid-displacement and after.
  - `hard_def` = Σ `Defense` of worn items, each item once, ammo skipped.
  - `matk1` = right-hand weapon `MagicAttack` + left-hand weapon `MagicAttack` (refine 0), sent as
    `SP_MATK1` and as the window's right side (`matk_bonus`). Captured (capture 4): a Rod +10 (MATK 30) gives
    `SP_MATK1` 50 and `SP_MATK2` 27 (status MATK; stays 0 here until stats). Item MDEF goes to `SP_MDEF2`
    (`hard_mdef`), but renewal item MDEF comes from item scripts, so it stays 0 here.
  - `range` = max(1, right weapon `Range`, left weapon `Range`). Bare hands: 1.
  - Everything else keeps today's values (loaded HP/SP/MaxHP/MaxSP; 0 for the rest).
- **`recalc(e)`**: compute the new value, compare it with the stored one, send one packet per changed field
  in the catalog push order, store the new value. Wire mapping: `SP_ATK2` ← `atk2`; a hard-DEF change sends
  `SP_DEF1` (soft) then `SP_DEF2` (hard), as renewal does; a MATK change sends `SP_MATK2` then `SP_MATK1`
  (captured); `SP_ATTACKRANGE` (`0x013a`) ← `range`.
- **The login push** (`send_login_status_push`) and `0x00bd` (`atk_bonus`, `matk_bonus`, `hard_def`) send the stored
  values instead of 0. The push still sends every field once (`DEFERRED.md` "Fix 2 simplifications").
- **Weight** isn't touched by equip.
- **Hand-off to the stats feature:** stats replaces the gear-only computation with the full
  `status_calc_pc`. The diff, push order, storage and call sites stay as they are.
- **Gear status reads the worn-slot table**, which §2.2 step 10 fills one position at a time, so a displaced
  item's recalc already counts the new item in the positions assigned so far. Each item counts once.
- *Gap until stats lands:* rAthena also pushes `SP_ASPD` on every weapon or shield change (capture: 440 ↔ 590),
  and `SP_ATK1` when a bow comes on or off (status ATK uses DEX for ranged weapons), and `SP_CRITICAL` when a
  katar comes on or off (capture 3: 2 ↔ 4). Here ASPD and ATK1 don't
  change and aren't pushed, so some captured pushes are shorter here (bow over Knife + Guard: no final push at
  all).

### 2.11 Persistence and char select (catalog §7, `packets/char.md`)
- `inventory.equip` already round-trips.
- **`characters` look columns** hold the char-select values (catalog `char.md`):
  - weapon = the weapon **type** (`W_*` or dual constant);
  - shield = the left-hand armor's `View` (0 for a left-hand weapon or nothing);
  - head top / mid / bottom and robe = view ids after the costume override.
  Schema: rename `weapon_view` → `weapon_type` and add `robe_view`, both through the guarded `DO` block
  (`specs/items` M5 note). The other `*_view` columns keep their names. *(Planning default.)*
- **Written with the inventory:** `save_inventory(char_id, inventory, looks)` updates the look columns in the
  same transaction. There's no save per equip, as in rAthena. An equip change bumps the inventory revision so
  autosave, logout and map change pick it up.
- **Char select:** `CharacterSummary` gains `CharacterLooks`, loaded by every summary query. `0x0b72` /
  `0x006b` / `0x0b6f` fill `weapon` (i16 type), `shield`, `accessory` (head bottom), `accessory2` (head top),
  `accessory3` (head mid) and `robePalette` (robe view, i32). Captured: a Knife gives `weapon` 1, Goggles
  `accessory2` 1 and `accessory3` 0, the Bravery Bag `robePalette` 2. Capture 3: Tsurugi + Damascus `weapon` 28
  and `shield` 0 (a left-hand weapon isn't a shield), the Jur 16, Tsurugi + Guard `weapon` 2 and `shield` 1 (the
  Guard's `View`). The client never draws the weapon or shield
  on the select screen (only in combat stance), so a missing weapon there is expected; headgear shows.

## 3. Scenarios

**TOOL** (`tools/item-import/tests`):
- **TOOL-ITEM-06:** `SubType` maps to `WeaponType` / `AmmoType`. Fields are cleared by type. `Range` is capped at
  14. `WeaponLevel` is clamped to 5.
- **TOOL-ITEM-07, `Jobs`:** `All`, a named list, `All` plus an exclusion, an absent key and an unknown name give
  the catalog §3a bitsets. Knight → index 1 bit 1, Crusader → index 2 bit 1, Novice → index 0 bit 0.
- **TOOL-ITEM-08:** `Classes` groups (`All_Upper`, `All_Third`, which excludes Fourth), and the forced `Gender`
  for a whip, an instrument and a wedding ring.
- **TOOL-ITEM-05 (updated):** counts still match (gated on `RATHENA_PATH`).

**DB** (`crates/ro-db/tests`):
- **DB-ITEM-04:** v2 round-trip with every new key. Defaults are omitted on write.
- **DB-ITEM-05:** a v1 header is rejected.
- **DB-ITEM-03 (updated):** the shipped v2 data loads. Knife is `Dagger`, `Attack` 17, `Range` 1.

**PKT** (`crates/ro-packets/tests`):
- **PKT-EQ-01, table lengths:** `0x0999` 11, `0x099a` 9, `0x0bae` 3, `0x013c` 4, `0x013b` 4, `0x0291` 4,
  `0x0983` 29, `0x0196` 9, `0x0998` 8, `0x00ab` 4, `0x0bf5` 6.
- **PKT-EQ-02:** decodes `0x0998` (the captured `06 00 02 00 00 00`, and ammo `1b 00 00 80 00 00`), `0x00ab`
  (`1a 00`) and `0x0bf5` (the captured `ff 83 00 00` = `0x83ff`).
- **PKT-EQ-03:** `0x0999` and `0x099a` encode the captured bytes (`06 00 02 00 00 00 00 00 00`;
  `02 00 02 00 00 00 00`), the `0x0999` failure `16 00 00 00 00 00 00 00 01`, the headgear ack
  `18 00 00 01 00 00 10 00 00`, plus the `0x099a` failure form `02 00 00 00 00 00 01`.
- **PKT-EQ-04:** `0x0bae`, `0x013c`, `0x013b`, `0x0291` and `0x0983` (EFST 695, 9999/9999) layouts.
- **PKT-EQ-05:** `0x01d7` for `LOOK_SHOES`, `LOOK_HEAD_*` and `LOOK_ROBE` (15 bytes; `val2` 0).
- **PKT-EQ-06:** entry packets put `UnitLooks` at the weapon/shield/accessory/robe offsets (catalog
  `map_units.md`).
- **PKT-EQ-07:** the char-select entry puts `CharacterLooks` at offsets 90/96/98/100/102/162 (catalog `char.md`).
- **PKT-EQ-08:** a player's `body` is its job id: char-select offset 88 and the entry packets' `body`; NPCs and
  mobs send 0. Found in the manual check (an SQL-made Archer drew a Novice body); every capture agrees.

**PER** (`crates/ro-persistence/tests/postgres_repo_tests.rs`):
- **PER-EQ-01:** `save_inventory` with looks updates the look columns in the same transaction, and summary
  queries return them.
- **PER-EQ-02:** the schema pass is idempotent on a database that has the old `weapon_view` column, and on a
  fresh one.

**ENG** (`crates/ro-engine` unit and integration tests):
- **ENG-EQ-01, rules:** `pos` widening for an Assassin with a dagger only. The accessory auto-slot (captured, capture 4)
  picks right (`0x08`), then left (`0x80`) when right is taken; both taken → right (its item is displaced). The dual-wield hand choice, as captured: right free → right; right taken and
  left free → left; both taken → right; a one-hand request (`0x02`) → right. A two-hander gets `EQP_ARMS`. Weapon
  type: dagger + sword → 28, dagger + dagger → 25, katar → 16, bare → 0.
- **ENG-EQ-02, checks in order:** level too low → 1, over max → 1; gender, broken, job (Novice vs a Swordman-only
  item; Knight id 7 for a Knight item), class (a transcendent-only item on a novice) → 2. Level beats gender.
- **ENG-EQ-03, captured swap:** wearing Knife A, equip Knife B. Self gets exactly the captured order (pushes
  contain `SP_ATK2` 0 then 17; no ASPD). An observer gets the two `0x01d7`. Swapping back mirrors it.
- **ENG-EQ-04:** a bow (`0x22`) over Knife + Guard, as captured: Guard off (look {Knife, 0}, `0x099a`, push of
  DEF only), then Knife off (look {0, bow}, `0x099a`, push of ATK2 and range that already count the bow), then
  `0x0999` with `0x22`, two identical `LOOK_WEAPON` {bow, 0}, and no final push. Over a Knife alone the Knife's
  look is {0, bow}. Taking the bow off sends two identical `0x01d7` before its `0x099a`.
- **ENG-EQ-05:** headgear: `0x0999` carries `View`. A Top+Mid headgear sends only `LOOK_HEAD_TOP`, never
  `LOOK_HEAD_MID`. Goggles over a Hat: Hat `0x099a` → `LOOK_HEAD_TOP` 0 → push with the Goggles' DEF →
  `0x0999` (view 1) → `LOOK_HEAD_TOP` 1, no push. A garment with a view sends `LOOK_ROBE` after `0x0999` and
  after its `0x099a` on take-off. A costume top overrides a normal top and removing it restores the normal view.
  Capture 4: with the costume worn, a normal top's equip or removal sends no `LOOK_HEAD_TOP`, and a costume with
  the same view as the normal top sends none; a low + mid + top headgear sends only `LOOK_HEAD_TOP`, on equip
  and on removal; `UnitLooks` and `CharacterLooks` carry the costume's view.
- **ENG-EQ-06, failures:** not identified → result 2; already worn → 2; wrong position → 2; dead → `0x0080` and
  no ack; empty or out-of-range slot → nothing.
- **ENG-EQ-07, take-off:** success order; the three `0x099a` failure forms.
- **ENG-EQ-08, unequip all:** per-item packets and pushes, then `0x0bae`. Dead → `0x0bae` result 1 and nothing
  removed (our ruling, §2.5). A two-hander is removed once; ammo
  removed by a bow's cascade isn't removed twice.
- **ENG-EQ-09, ammo:** an archer (job 3) with a bow equips arrows → `0x0983` (area) + `0x013c` + `0x013b` 3, no
  `0x0999`. Arrows with no bow (or a katar, capture 4) → `0x0291` 1691 + result 2. Taking off the bow removes the arrows (their
  `0x099a` before the bow's push). Equipping a dagger over the bow removes the arrows.
- **ENG-EQ-10, drop worn (unit test only; the client won't send it, §2.8):** dropping a worn weapon gives `0x0add` → look / `0x099a` / push → `SP_WEIGHT` →
  `0x00af`, and the observer sees the look. Dropping 5 of 100 worn arrows keeps them worn.
- **ENG-EQ-11, entry looks:** an observer's entry packet for a player wearing a knife, a shield, a headgear and
  a garment carries those looks. Spawning with worn gear builds `UnitLooks` and `GearStatus` from the masks.
- **ENG-EQ-13, dual wield (capture 3):** an Assassin (job 12) wearing a Knife equips a Damascus → `0x0999`
  location `0x20`, `LOOK_WEAPON` {1201, 1222}, push ATK2 135. A Jur over Damascus L + Gladius R → Damascus off
  (look {1219, 0}, ATK2 105), Gladius off (look {0, 1250}, ATK2 125), `0x0999` `0x22`, two {1250, 0}, no final
  push. Entry packets carry the left-hand weapon in `shield`; char-select looks give `weapon` 28 for Tsurugi +
  Damascus.
- **ENG-EQ-12, map entry ammo:** worn arrows add `0x0983` + `0x013c` between `0x0b09` and `0x0b39` on every
  `0x007d`, to self only (an observer gets neither).
- **ENG-STAT-01, hook:** no change → no packets. ATK2 change → `SP_ATK2` only. DEF change → `SP_DEF1` then
  `SP_DEF2`. Range change (a bow) → `0x013a`. A Rod → `SP_ATK2` 15, `SP_MATK2` 0, `SP_MATK1` 30. Several changes come in catalog order.
- **ENG-STAT-02, login:** a login wearing a Knife sends `SP_ATK2` 17 in the push and `atk_bonus` 17 in `0x00bd`.
- **ENG-STAT-03:** an equip change bumps the inventory revision (autosave sees it).

**BIN** (`bins/*/tests`):
- **BIN-EQ-01:** over TCP, `0x0998` on a knife gives `0x0999` and `0x01d7`, and a second client sees the
  `0x01d7`.
- **BIN-EQ-02:** equip, log out, log back in: the item is still worn (`0x0b39` `WearState`), and the char list
  shows the weapon type and head views.
- **BIN-EQ-03:** equip, then a cross-map warp: still worn, and the looks hold on the new map.

## 4. Licensing
- Item data stays GPL under `crates/ro-db/data/item/`, regenerated by `tools/item-import`. Update the
  `NOTICE.md` item section (new fields, date, commit).
- The job-name table in the importer and the job id → mapid table in `ro-engine` are facts (numbers and names)
  written from catalog §3a. Don't copy rAthena's enum or switch code.
- `0x0291` message ids are numbers only. Never send rAthena or client message text.

## 5. Deferred (move into `DEFERRED.md` at Finish)
- Item scripts and bonuses; cards and combos; `EquipScript` / `UnEquipScript`.
- Refine and grade bonuses to ATK/DEF (refine is always 0 today).
- ASPD, status ATK/MATK, soft DEF/MDEF, HIT, FLEE, CRIT, MaxHP/SP from gear: the stats feature fills
  `DerivedStatus`.
- Bind-on-equip (`bound = 4`, `clif_notify_bindOnEquip`).
- Equip switch (`0x0a9b` contents, `equip_switch` column use).
- Pet armor.
- Strip / `cant.equip` / `SC_PYROCLASTIC` / `SC_KYOUGAKU` blocks; map `NoEquip`; `PCBLOCK_EQUIP` and NPC-script
  gates with `MSI_CAN_NOT_EQUIP_ITEM` (372).
- Re-checking worn gear on level or job change (rAthena force-unequips).
- `AS_LEFT` dual wield for non-Assassins; the `AS_VENOMKNIFE` dagger-ammo rule.
- Soul Linker Super Novice shortcuts; the `PC_PERM_USE_ALL_EQUIPMENT` GM bypass.
- `skillrange_from_weapon` skill-list re-send on a range change.
- `canequip_tick`.
- The ammo EFST end (`0x0196`): rAthena never sends it. Revisit if the client keeps the icon after unequip.
- A GM `@job` command (job change belongs with stats). Until then, test job-limited gear with
  `UPDATE characters SET job_class = …`.
