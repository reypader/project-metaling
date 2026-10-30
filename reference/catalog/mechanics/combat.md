# Combat Damage Formulas and Modifier Tables

Target client version: `PACKETVER = 20260219`, Renewal (`RENEWAL` and `RENEWAL_ASPD` defined). Citations are `$RATHENA_PATH/src/map/...` unless noted. This file currently covers the **normal (non-skill) melee attack of a player against a monster**; skills, magic, and player-vs-player are not covered. Lifecycle (request, timer, packets, delays): [auto_attack.md](./auto_attack.md). Sections 1-6 are source-derived; **section 7** checks them against a real-client capture ([`combat_sequence_1.txt`](../captures/combat_sequence_1.txt)) and **overrides** them where marked `CAPTURE` (notably the dagger size modifier vs Medium targets).

## Combat Damage Pipeline

* **Provenance:** `battle.cpp` (`battle_calc_weapon_attack` L5387-5780, `is_attack_hitting` L3228, `is_attack_critical` L3017, `battle_calc_damage_parts` L3889, `battle_calc_base_weapon_attack` L2443, `battle_calc_defense_reduction` L4720, `battle_calc_attack_post_defense` L4894), `status.cpp` (`status_calc_misc` L2600, `status_base_atk` L2424), `db/re/size_fix.yml`, `conf/battle/battle.conf`. All apply to `20260219` (server-side, not packet-version dependent except the crit type noted in section 6).
* Integer arithmetic throughout; `/` is truncating integer division unless a float is stated. `rnd()%N` = uniform integer `0..N-1`.
* **Order of operations for one swing** (`battle_calc_weapon_attack`):
  1. Initialise (`type = DMG_NORMAL`, `div = 1`, `amotion` = attacker amotion, `dmotion` = target dmotion, flag `BF_WEAPON|BF_NORMAL|BF_SHORT` (melee) or `BF_LONG` for bow/gun) (L5277-5318).
  2. **Lucky dodge**: if target `flee2 > 0` and `rnd()%1000 < flee2` -> `type = DMG_LUCY_DODGE (11)`, damage 0, done (L5409-5416). Mob `flee2` is 0 (`enable_perfect_flee = BL_PC|BL_PET`, `battle.conf:17`).
  3. **Multi attack** (Double Attack for dagger: Renewal chance `7 * TF_DOUBLE level` %, `div = 2`, `type = DMG_MULTI_HIT (8)`) (L4267+). Defer unless Thief line is implemented.
  4. **Critical roll**, then **hit roll** (a crit always hits).
  5. If it hits and the target is not an "infinite defense" plant: compute base damage parts, apply skill ratio (100 for a normal attack), DEF reduction, element, crit multiplier, cards/masteries, div multiplier.
  6. `battle_calc_attack` post-step (L6765): if `damage + damage2 < 1` -> miss (`dmg_lv = ATK_MISS`, `dmotion = 0`, `div = 1` for players).

## 1. Hit chance

Source: `is_attack_hitting`, `battle.cpp:3228-3315`.

A normal attack auto-hits if: it is a critical; or the attacker has Perfect Hit bonus roll; or `SC_FUSION`; or the target has a hard status (`opt1` stone/freeze/stun/sleep-class, except stone-wait and burning). Otherwise:

```text
flee    = target.flee            (agi_penalty applies to PC targets only; see below)
hitrate = attacker.hit - flee    (Renewal base hitrate is 0, pre-renewal was 80)
+ AC_VULTURE level (Renewal, players), + bow/arrow hit bonuses, weaponry/sword-training hidden bonuses (defer)
hitrate = clamp(hitrate, min_hitrate = 5, max_hitrate = 100)
hit     = rnd()%100 < hitrate
```

* Miss result: `dmg_lv = ATK_FLEE`, damage 0, packet type stays `DMG_NORMAL (0)` with damage 0 (client shows "Miss"). Lucky dodge is the only miss with its own type.
* `agi_penalty_type 1`, `agi_penalty_target 1` (PC only), `agi_penalty_count 3`, `agi_penalty_num 10` (`battle.conf:85-95`): a **player** target attacked by >= 3 units loses 10% flee per extra attacker. Does not apply when the target is a mob.
* Stat bases (`status.cpp:2600-2740`, base level `L`):

| Stat | Player (Renewal) | Monster (Renewal) |
| :--- | :--- | :--- |
| `hit` | `L + dex + luk/3 + 175 + 2*con` + gear/skill bonuses | `L + dex + 150 + 2*con` |
| `flee` | `L + agi + luk/5 + 100 + 2*con` + bonuses | `L + agi + 100 + 2*con` |
| `flee2` (lucky dodge, 0.1%) | `luk + 10` + bonuses | 0 |
| `cri` (0.1% units) | `10 + 3*luk + L/10` + weapon bonuses; **x2 for Katar** (`status.cpp:6148`); dagger/katar mastery adds a flat bonus (`status.cpp:4535-4540`, defer) | 0 (`enable_critical` excludes `BL_MOB`) |
| `def` (hard, eDEF) | armor DEF sum | mob_db `Defense` |
| `def2` (soft, sDEF) | `(L + vit)/2 + agi/5` (+ bonuses) | `(L + vit)/2` |

  Trait stats `con`, `pow`, etc. are 0 unless the mob_db or char has them. Mob base stats come from `mob_db.yml` (`Str`, `Agi`, `Vit`, `Int`, `Dex`, `Luk`, `Level`), defaulting to 1 where unspecified (`mob.cpp:4950-4956`).

### 1.1 Monster target stats, exact (M0 round 2)

Source: `mob.cpp:4940-4975` (db defaults), `5044-5217` (field parsing), `5370-5412` (motions), `5537-5580` (`loadingFinished`), `status.cpp:2600-2740` (`status_calc_misc`), `2775-2893` (`status_calc_mob_`), `6428-6445` (`SCB_DSPD`). Applies to `20260219` Renewal, default battle config.

* **When computed:** once per mob_db entry in `MobDatabase::loadingFinished`, via `status_calc_misc(&data, &mob->status, mob->lv)` with `lv` = db `Level` (default 1). At spawn `status_calc_mob_(SCO_FIRST)` just `memcpy`s that db status into `md->status` **unless** a special flag applies (`mobs_level_up` > db level, `special_state.size`, guardian, slave inherit). `mobs_level_up` default is `no` (`monster.conf:249`).
* **Spawn-line level:** a level in the spawn line sets `md->level` (`mob.cpp:477-478`, kept if `0 < level <= MAX_LEVEL`, `status.cpp:2779-2784`). It does **not** touch `hit`, `flee`, `def2`, `batk`, `max_hp` or any stat (all baked from the db `Level`). It only changes `status_get_lv(md)` (used by e.g. Target-weak mode checks, zeny/drop-by-level), and, only with `mobs_level_up: yes`, adds `md->level - db Level` to all six stats and recomputes via `status_calc_misc(md, status, md->level)`. For first-cut implementation: **use db `Level`**, ignore the spawn-line level.
* Stat defaults in db: `Str Agi Vit Int Dex Luk` default **1** (`mob.cpp:4950-4955`); an explicit `0` stays 0 (`max(0, stat)`, e.g. plants have Agi/Vit/Dex/Luk 0). `con/pow/sta/spl/wis/crt` stay 0 (not in mob_db), so all `2*con`, `res`-from-`sta` and `crate` terms vanish.
* Exact values for a mob with db level `L`:

| Field | Value | Notes |
| :--- | :--- | :--- |
| `hit` | `clamp(L + dex + 150, 1, 32767)` | `status.cpp:2653-2657`; the `+ luk/3` term is PC only |
| `flee` | `clamp(L + agi + 100, 1, 32767)` | `status.cpp:2658-2662`; **no `luk/5`** for mobs (PC only) |
| `flee2` | `0` | `enable_perfect_flee = PC|PET`; mob branch zeroes it (`status.cpp:2712-2717`) |
| `cri` | `0` | `enable_critical` excludes mobs (`status.cpp:2699-2708`) |
| `def` (hard) | db `Defense` (`0..` capped to `DEFTYPE_MAX`) | `mob.cpp:5120-5131`; unchanged in Renewal for mobs (the `+vit +lv/2` rule is homunculus only) |
| `def2` (soft) | `clamp((int)(((float)L + vit) / 2), 0, 32767)` | `status.cpp:2663-2669`; the float division of non-negative values is identical to integer `/2` |
| `mdef` / `mdef2` | db `MagicDefense` / `(int)((int_ + L) / 4.0)` | `mdef2` mob branch `status.cpp:2672-2676` |
| `res` | db `Resistance` (default 0) | set at db load (`mob.cpp:5148-5155`); `status_calc_misc` keeps it for `BL_MOB` and adds `sta + sta/3*5` = 0 |
| `mres` | db `MagicResistance` (default 0) | same |
| `luk` used by the player's crit roll | db `Luk` (default 1) | `cri -= tstatus->luk * 2` (`battle.cpp:3061`, factor 3 only when a mob attacks a PC) |
| `size`, `race`, `def_ele`, `ele_lv` | db `Size` (default Small), `Race` (default Formless), `Element` (default Neutral), `ElementLevel` (default 1) | `mob.cpp:5260-5290, 5326-5350`; not level dependent |
| `amotion`, `adelay` | db `AttackMotion` (cap `1..4000`), `AttackDelay`; then `adelay = max(adelay, amotion)` | `mob.cpp:5370-5390, 5551` |
| `dmotion` | db `DamageMotion * monster_damage_delay_rate / 100` (rate default 100, only applied when != 100) | `mob.cpp:5397-5408`; **never recomputed** for mobs (`SCB_DSPD` only touches PC/HOM, `status.cpp:6434-6446`), so the value in `0x08c8` `dmg_speed` is the plain db value |
| `clientamotion` | db `ClientAttackMotion`, else `amotion` | `mob.cpp:5391-5398` |

* **Correction to the table in section 1:** the "Monster" `flee2` row is 0 and the `cri` row is 0 (already there); nothing else in that table changes. Poring (1002) worked values: `L=1, dex=6, agi=1, vit=1, luk=5` -> `hit = 157`, `flee = 102`, `def = 2`, `def2 = (1+1)/2 = 1`, `dmotion = 480`.

### 1.2 Infinite-defense ("plant") targets

Source: `is_infinite_defense` `battle.cpp:2890-2917`, plant branch `battle.cpp:4951-5030` (`battle_calc_attack_plant`), call sites `battle.cpp:5452, 5762-5765`.

* **Condition for a normal melee attack:** the target has the mode bit `MD_IGNOREMELEE` (`mob_db` `Modes: IgnoreMelee: true`; it is a *mode* flag, not Race or Class), and the attack is `BF_WEAPON|BF_SHORT` (a normal swing by a non-ranged weapon). `IgnoreRanged` covers `BF_WEAPON|BF_LONG`, `IgnoreMagic` `BF_MAGIC`, `IgnoreMisc` `BF_MISC`. Also true when the target has `SC_INVINCIBLE`. (`MD_` `Ai` bits do not matter.) `status.cpp:3827` strips these four modes from a *player's* base mode, and `mob.cpp:1084` treats a mob with any of them as "plant type" for spawn/Ai purposes.
* **Order in `battle_calc_weapon_attack`:** lucky dodge (mob `flee2` is 0) -> multi-hit -> crit roll -> **normal hit roll** (`is_attack_hitting`, uses the plant's `flee`; a plant with `Agi: 0` has `flee = 101`, so a low-level attacker can miss it: `hitrate = clamp(hit - 101, 5, 100)`) -> only if the hit lands and `is_infinite_defense` is true, skip all damage math.
* **Result** (`battle_calc_attack_plant`): if the swing hit (crit counts as hit), `damage = 1` for the right hand; left hand `1` (0 for Katar). Then the `div` fix (`battle_apply_div_fix`) and a final status/damage-block check (`battle_status_block_damage`) that can zero it. A miss stays `0` (`ATK_FLEE`, "Miss"). No DEF, element, size or card step. Crit still reports `type = 10` (the roll happened before) but the damage is 1. So an auto-attack deals **exactly 1 HP per landed hit**, no minimum-damage rule needed beyond that.
* **Normal field mobs with `IgnoreMelee`** (`db/re/mob_db.yml`, 162 entries mostly events/instances/MVP helpers): `1078 RED_PLANT`, `1079 BLUE_PLANT`, `1080 GREEN_PLANT`, `1081 YELLOW_PLANT`, `1082 WHITE_PLANT`, `1083 SHINING_PLANT`, `1084 BLACK_MUSHROOM`, `1085 RED_MUSHROOM`, `1182 THIEF_MUSHROOM`, `2033 GOLDEN_TULIP`, `2372-2374 SOIL/ROCK/THICKET`. All: Level 1, Def 100 (160 for Thief Mushroom), Agi/Vit/Dex/Luk 0, Race Plant, Element Earth (Shining Plant Holy), `DamageMotion 1`, `Ai 06`, HP 5-20. `npc/re/mobs/fields/` spawns include Green Plant/Blue Plant/Shining Plant in prt_fild01-06 and Green Plant, Red Mushroom, Shining Plant in pay_fild01-04. **Pupa (1008) and Poring (1002) are not infinite-defense** (no `Modes`), so they take normal damage.

## 2. Critical chance

Source: `is_attack_critical`, `battle.cpp:3017-3150`.

```text
cri = attacker.cri                      (0.1% units)
cri -= target.luk * 2                   (a PC attacker; x3 when a mob attacks a PC)
if target asleep: cri *= 2              (SC_SLEEP)
crit = rnd()%1000 < cri                 (also cri must be non-zero)
```

* Only players (and mercenaries) have a non-zero `cri` by default: `enable_critical: 17` = `BL_PC (1) | BL_MER (16)` (`battle.conf:21`), so **monsters never crit** (their `cri` is set to 0 in `status_calc_misc`, `status.cpp:2692-2699`). The `Monster` column in the stat table therefore shows the formula but the value is 0 with defaults. Bow/gun criticals use the same roll with extra ranged bonuses (defer).
* Target card `critical_def` reduces `cri` by that %. Defer.
* Renewal crit does **not** ignore DEF (the "crit ignores DEF" branch is pre-renewal only, `battle.cpp:3334-3338`).

## 3. Base damage of a player normal attack (Renewal)

Source: `battle_calc_damage_parts` L3889 + `battle_calc_weapon_attack` L5530-5640.

All parts are computed separately, then summed. `target` terms: `def_ele`/`ele_lv` (mob element), `size`, `race`.

| Part | Formula | Element/size |
| :--- | :--- | :--- |
| **Status ATK** `statusAtk` | `batk`, element fix (always **Neutral** attacker element vs target `def_ele`), then **x2** | neutral |
| **Weapon ATK** `weaponAtk` | variance roll (below) + overrefine, then size fix, then element fix with **weapon element** (default Neutral) | weapon ele, size fix |
| **Equip ATK** `equipAtk` | `status.eatk` (sum of gear `bonus bAtk`-style equip ATK) **plus ammo `Attack`** while a bow/gun swings (`sd->bonus.arrow_atk`, flat: not in the variance roll, not size-fixed; Arrow = 25; `battle.cpp:3431-3447`, see [auto_attack.md 5.3](./auto_attack.md#53-ranged-normal-attacks-and-ammo)), element fix with weapon ele (or ammo `arrow_ele`) | weapon ele |
| **Percent ATK** `percentAtk` | `(weaponAtk + equipAtk) * bonus.atk_rate / 100` | - |
| **Mastery ATK** `masteryAtk` | weapon masteries (Sword Mastery +4/lv etc.), no element | none (defer) |

`batk` for a **player** (`status_base_atk`, `status.cpp:2424-2490`, bow/gun/whip/instrument swap `str`<->`dex`):

```text
batk = (str*10 + dex*10/5 + luk*10/3 + L*10/4) / 10  +  5 * pow  (+ stat bonuses from items)
```

Weapon ATK variance (`battle_calc_base_weapon_attack`, `battle.cpp:2443-2495`), with `atk = rhw.atk` (item ATK), `watk = rhw.atk + rhw.atk2` (item ATK + refine bonus), `wlv` = weapon level:

```text
variance        = 5.0 * atk * wlv / 100        (float)
base_stat_bonus = atk * str / 200              (float; dex instead of str for bow/gun/whip/instrument)
atkmin = max(0,     int(watk - variance + base_stat_bonus))
atkmax = min(65535, int(watk + variance + base_stat_bonus))
damage = atkmax                      if critical (or Maximize Power)
       = uniform integer in [atkmin, atkmax]  otherwise
damage += rnd()%overrefine + 1       (if weapon over-refined; overrefine = item refine "randombonus_max")
damage += damage * weapon_damage_rate[weapon type] / 100   (card/bonus, default 0)
damage  = damage * size_mod[target.size] / 100
```

* With **no weapon equipped** the variance step is skipped and `damage = watk` (0 for bare hands), so only Status ATK contributes.
* `rhw.atk2` (refine bonus) = `refine_bonus / 100` from `db/re/refine.yml`-style tables (`status.cpp:3955-3961`); item `Attack` goes to `rhw.atk`.
* `size_mod[target.size]` = `atkmods` of the wielded weapon type, applied to the **weapon part only** (`battle_calc_sizefix`, `battle.cpp:2427-2433`, called at the end of `battle_calc_base_weapon_attack` L2492). Not applied to status ATK, equip ATK or arrow ATK (those are added separately). Skipped for non-players, with `bNoSizeFix` (`special_state.no_sizefix`), or when Weapon Perfection is active. A bare-handed swing uses `W_FIST`, whose row is 100/100/100, so it is a no-op. Truncating integer `/100`. **No `battle.conf` toggle** exists for it. Riding with a spear sets Medium = Large (`status.cpp:4238-4244`).
  * **Table actually loaded in Renewal** (`status.cpp:4226-4236` copies it into `atkmods`): the loader reads `db/size_fix.yml` (`getDefaultLocation`, `db_path` = `db`), whose Footer imports `db/re/size_fix.yml` (Renewal) then `db/import/size_fix.yml` (empty). Unlisted sizes default to 100; an overlay entry only overrides the sizes it names. `db/re/size_fix.yml` alone is just an overlay (Knuckle Large 75, Whip Large 75); the earlier claim "100 except Knuckle/Whip" came from reading only that overlay and was **wrong**. `db/pre-re/size_fix.yml` (Knuckle M75/L50, Whip L50) is not loaded in Renewal. Final table (Small / Medium / Large, %):

    | Weapon | S | M | L | Weapon | S | M | L |
    | :--- | :-: | :-: | :-: | :--- | :-: | :-: | :-: |
    | Fist | 100 | 100 | 100 | Bow | 100 | 100 | 75 |
    | Dagger | 100 | **75** | 50 | Musical | 75 | 100 | 75 |
    | 1hSword | 75 | 100 | 75 | Whip | 75 | 100 | 75 |
    | 2hSword | 75 | 75 | 100 | Book | 100 | 100 | 50 |
    | 1hSpear, 2hSpear | 75 | 75 | 100 | Katar | 75 | 100 | 75 |
    | 1hAxe, 2hAxe | 50 | 75 | 100 | Knuckle | 100 | 100 | 75 |
    | Mace | 75 | 100 | 100 | 2hMace, Staff, 2hStaff | 100 | 100 | 100 |
    | Revolver, Rifle, Gatling, Shotgun, Grenade, Huuma | 100 | 100 | 100 | | | | |

  * This table explains the capture (section 7): Dagger vs Medium = 75, Bow vs Small = 100. Provenance: `db/size_fix.yml` (base, L25-81), `db/re/size_fix.yml` (L33-39), loader `status.cpp:492-582`, `database.cpp:176-224` (Footer imports filtered by `Mode`). Applies to `20260219`.
  * **Licensing:** this is rAthena db YAML (GPL-3.0). It may enter the project only via an import tool (`tools/size-fix-import` writing under `crates/ro-db/data/`, merging base + Renewal overlay + import), never hand-typed into `crates/`/`bins/` code.
* Element table `attr_fix` (attacker element x defender element/level) multiplies each part as a percent; Neutral attacker is 100% against every defending element at levels 1-4 **except Ghost** (`db/re/attr_fix.yml`: Ghost lv1 90, lv2 70, lv3 50, lv4 0; Water/Earth/Fire/Wind/Poison/Holy/Dark/Undead/Neutral all 100). Elements, element-changing cards and the full table are **deferrable**: first implementation can use 100%. Poring is Water 1; Neutral vs Water = 100%.
* Cards/race-size-element %: Renewal applies attacker cardfix to `weaponAtk` and `equipAtk` only, before summing (`battle.cpp:5555-5575`). Defer.

Assembly (player attacker, `battle.cpp:5577-5640`):

```text
damage = statusAtk + weaponAtk + equipAtk + percentAtk
damage = floor(damage * (100 + patk) / 100)       // patk = P.ATK (trait; 0 unless pow/con > 0)
damage += masteryAtk
if critical: damage += damage * bonus.crit_atk_rate / 100    (cards; default 0)
damage = damage * skill_ratio / 100                         // = 100 for a normal attack
[Res reduction if target res > 0]
[DEF reduction, section 4]
[post-defense minimum 1, section 4]
[element step is a no-op for players in Renewal, already applied per part]
if critical: damage = floor(damage * (1.4 + 0.01 * crate))   // crate = status.crate = crt/3 + bonuses; 0 default
damage = damage * div          (double attack: div 2)
```

* **Res (target physical resist, Renewal trait)** (L5592-5620): if `target.res > 0`, `damage -= damage * (res/(res+400.0)) * 0.8`. Mob `Res` default 0 -> skip.
* **Critical damage** is applied **after** DEF reduction (`battle.cpp:5646-5658`): x`1.4` (+ `0.01 * crate` for players); monsters attacking get x1.4 flat. The weapon-ATK roll for a crit is already `atkmax`.
* Monster ATK (for later, mob hits player): weapon min/max = `Attack*80/100 .. Attack*120/100` from mob_db `Attack` (`status.cpp:2522-2552`); `batk = str + Level` (`status.cpp:2480`). Not needed for player-vs-mob.

## 4. DEF reduction

Source: `battle_calc_defense_reduction`, `battle.cpp:4720-4893`, Renewal branch.

For a monster target (no `tsd`): `def1 = status_get_def(target)` (hard DEF = mob_db `Defense`), `vit_def = def2` (= `(L + vit)/2`, mob soft DEF). The `vit_penalty` (multi-attacker DEF loss, `vit_penalty_type 1`, `vit_penalty_target 1`) only targets PC, so mobs get none (`battle.conf:101-111`). `weapon_defense_type` default 0.

```text
damage = damage * (4000 + def1) / (4000 + 10 * def1)  -  vit_def
```

* Applied to the full summed damage (status+weapon+equip+percent+mastery), once, per hand. Skipped if the attack ignores DEF (cards "ignore DEF by race/class", skills flagged `IGNOREDEFENSE`) or is "piercing".
* `def1 == -400` is clamped to `-399` to avoid division by zero.
* Post-defense (`battle_calc_attack_post_defense`, L4894-4949): Aura Blade etc. bonuses; then **`damage = max(damage, 1)`** (`battle_min_damage(..., 1)` right-hand). So a landed normal attack on a monster deals at least **1** damage, never 0 (outside plant/immunity cases).
* Later: GvG/BG reductions, `battle_calc_weapon_final_atk_modifiers` (Reject Sword, Poison React, status procs - all `sc`-driven), absorb/reflect (`battle_do_reflect`). All status-dependent; defer.
* "Plant" mobs (`MD_IGNOREMELEE` mode, e.g. Green/Red Plant, Mushrooms): 1 damage per landed hit, 0 on miss; exact rules in [section 1.2](#12-infinite-defense-plant-targets).

## 5. Worked skeleton for a first implementation

Player (Novice, fist, base level L, STR/DEX/LUK/AGI known) vs a mob (level `ML`, `def` D, vit V, flee `ML+agi+100`):

1. `hit = L + dex + luk/3 + 175`; `flee = ML + mob_agi + 100`; `hitrate = clamp(hit - flee, 5, 100)`; roll `rnd()%100 < hitrate`.
2. `cri = 10 + 3*luk + L/10 - mob_luk*2`; `rnd()%1000 < cri` -> crit (auto-hit, skip step 1).
3. `batk = (str*10 + dex*2 + luk*10/3 + L*10/4)/10`; `damage = 2 * batk + weapon_roll` where `weapon_roll` = the variance formula above (0 bare-handed). **CAPTURE:** confirmed for Knife and Bow; `weapon_roll` must include the size modifier (`w * size_mod[target.size] / 100`, truncating, applied to the rolled value before adding anything else) and, for a bow, a flat +25 arrow ATK after it. For a bow `batk` uses DEX where STR is, and the weapon bonus term uses DEX.
4. `damage = damage * (4000 + D)/(4000 + 10*D) - (ML + V)/2`; `max(damage, 1)`.
5. Crit: `damage = damage * 1.4` (floor).
6. Result: damage, `type` 10 for crit else 0 (miss -> damage 0, type 0).

**Deferrable for a first cut:** element table, size table (section 3 table; Dagger Medium 75 matters for the first cut), cards and `bonus` rates, masteries, Double Attack, status-change modifiers, Res, plant mobs, overrefine, dual wield (`damage2`), Triple Attack, ammo for bows.

## 6. Result struct -> `ZC_NOTIFY_ACT` mapping

| Condition | `type` (`e_damage_type`, `clif.hpp:691`) | `damage` | `div` | Notes |
| :--- | :--- | :--- | :--- | :--- |
| Normal hit | 0 `DMG_NORMAL` | total | 1 | |
| Miss (flee) | 0 `DMG_NORMAL` | 0 | 1 | `dmotion` sent as 0 |
| Lucky dodge | 11 `DMG_LUCY_DODGE` | 0 | 1 | |
| Critical | 10 `DMG_CRITICAL` | total | 1 | |
| Double attack | 8 `DMG_MULTI_HIT` | damage x2 | 2 | dagger Thief line |
| Double attack + crit | 13 `DMG_MULTI_HIT_CRITICAL` | damage x2 | 2 | only when `PACKETVER >= 20161207` (true for 20260219), `battle.cpp:5428-5434` |
| Target has Endure | 4 / 9 | total | 1 / n | `clif_calc_delay` converts types 0/10/13 to 4 and 8 to 9 (`clif.cpp:5144-5175`) |

`damage` in the packet is the capped `int32` sum of `damage + damage2` fields (the struct carries both; `damage2` is the left-hand/dual-wield part). Details and sender rules: [packets/map_units.md#0x08c8](../packets/map_units.md#0x08c8-zc_notify_act-aka-zc_notify_act3), [packets/combat.md](../packets/combat.md).

## 7. Capture: combat_sequence_1.txt (damage vectors, death, ammo)

Source: [`../captures/combat_sequence_1.txt`](../captures/combat_sequence_1.txt), real client, rAthena `d4b8e7b8f`, `PACKETVER 20260219`, 2026-10-30. **File lines** (proxy `#` + 10). Session `#3` = attacker A (`combattest1`, aid 2000000, char 150012), `#6` = observer B (`combattest2`, aid 2000001; B lands only a few swings, see 7.3). Not partied. Applies to 20260219. Decoded by script from the hex payloads (not eyeballed); chat markers ignored. Mob stats are from rAthena `db/re/mob_db.yml` (the project's `crates/ro-db/data/mob/mob_db.yml` only holds Level/Hp/drops today, no Defense/Vit/Size, so the importer must add them): Poring 1002 (Lv 1, HP 55, Def 2, Vit default 1, Medium, flee 102), Lunatic 1063 (Lv 3, HP 48, Def 18, Vit 3, Small, flee 106), Pupa 1008 (Lv 4, HP 51, Def 24, Vit 3, Small, flee 105, DamageMotion 1), Marine Sphere 1142 (Lv 51, HP 1831, Def 41, Vit 35, Small, flee 167, DamageMotion 1). Mob ids come from the entry packets (`job` at payload byte 19).

### 7.1 `0x08c8` as seen (layout confirmed)

Payload (opcode stripped, 32 bytes): `src u32 | dst u32 | server_tick u32 | src_speed u32 | dmg_speed u32 | damage u32 | is_sp u8 | div u16 | type u8 | damage2 u32`. For A's swings: `src_speed` = A's pushed `SP_ASPD` (590 / 580 / 530 / 540, i.e. `amotion`); `dmg_speed` = the mob's db `DamageMotion` on a hit (Poring 480, Lunatic 336, Pupa 1, Marine Sphere 1) and **0 on a miss**; `div` 1, `damage2` 0, `is_sp` 0 everywhere; type 0 for hit and miss, **10** for the one crit (line 2960). Mob to A: `src_speed` = mob `AttackMotion` (Poring 672, Lunatic 456), `dmg_speed` = A's `dmotion` (796 at AGI 1, 792 later; source: `800 - 4*AGI`) on a hit, 0 on a miss. A swing is also sent to B (`AREA`) with the same bytes.

### 7.2 Attacker stat phases (A)

`0x00bd` at login (line 107): status points 48, STR/AGI/VIT/INT/DEX/LUK all 1 (cost 2 each), ATK1 1, ATK2 17, MATK 0/1, soft/hard DEF 1/10, MDEF 1/0, HIT 177, FLEE 102, FLEE2 1, CRIT 1, ASPD 590, range 1; HP 40, SP 11, weight 500/20300. The new character already wears a **Knife (1201, ATK 17, wlv 1)** and a DEF-10 body item; `@item 1201` (line 2245) only adds a spare. Values are the pushed totals (`0x0141` base+bonus; ATK1 = `batk`, ATK2 = `watk+watk2+eatk`).

| Phase | Lines | B/J lv | STR DEX LUK (total) | ATK1 | ATK2 | HIT | CRIT | ASPD | Range |
| :--- | :--- | :--- | :--- | ---: | ---: | ---: | ---: | ---: | ---: |
| P1 login Knife | 581-679 | 1/1 | 1 1 1 | 1 | 17 | 177 | 1 | 590 | 1 |
| P2 job lv 2 (LUK +1 job bonus) | 816-843 | 1/2 | 1 1 2 | 2 | 17 | 177 | 1 | 590 | 1 |
| P3-P4 DEX +1 (job) | 940-1178 | 1/3-4 | 1 2 2 | 2 | 17 | 178 | 1 | 590 | 1 |
| P5-P6 base lv 2 | 1265-1848, 1921-1926 | 2/5-6 | 1 2 2 | 2 | 17 | 179 | 1 | 590 | 1 |
| P7 status points spent (lines 1941-2101) | 2121-2167 | 2/6 | 11 15 2 | 15 | 17 | 192 | 1 | 580 | 1 |
| P8 `@luk 98` (line 2288), base lv 3 | 2766-2813 | 3/7 | 11 15 100 | 48 | 17 | 226 | 31 | 580 | 1 |
| P9 job lv 8 (STR +1 job) | 2885-3122 | 3/8 | 12 15 100 | 49 | 17 | 226 | 31 | 580 | 1 |
| P10 Archer, Bow, 10 Arrows, `@blevel 10` | 3441-3497 | 13/11 | 12 15 99 | 53 | 15 | 236 | 30 | 530 | 5 |
| P11 `@luk -100` | 3571-3699 | 13/11 | 12 15 1 | 20 | 15 | 203 | 1 | 530 | 5 |
| P12 `@dex -11` | 3767 | 13/11 | 12 5 1 | 10 | 15 | 193 | 1 | 540 | 5 |
| P13 Owl's Eye 3, Vulture's Eye 6 | 4042-4478 | 13/11 | 12 8 1 | 13 | 15 | 196 | 1 | 540 | 11 |

Notes: the Bow (1701, ATK 15, range 5, wlv 1, `EquipLevelMin 4`) was refused twice at base level 3 (`0x0999` result 1, lines 3327, 3339) and accepted after `@blevel 10`. ATK2 stays 15 after the Arrows (1750, ATK 25) are equipped: arrow ATK is **not** in the status window but is in the damage (7.3). Owl's Eye (skill 43) adds DEX (+1 each), Vulture's Eye (44) adds +1 range each (`0x013a` 6..11 at lines 3952-3972). `SP_ASPD` 590 to 580 came with DEX 8+; the Archer's Bow is 530/540.

### 7.3 Damage vectors (every A swing at a mob, 70 of them) and verdict

Model used (all integer, `/` truncating): `W` = weapon roll in `[int(atk - 5*atk*wlv/100 + atk*stat/200), int(atk + 5*atk*wlv/100 + atk*stat/200)]` (`stat` = STR for Knife, DEX for Bow); `W = W * size_mod / 100`; `total = 2*ATK1 + W (+25 for arrows)`; `d = total * (4000+Def) / (4000+10*Def) - (MobLv + MobVit)/2`; crit: `W` = max of range, `d = floor(d * 1.4)`. **All 70 swings fit**; "captured" lists distinct non-miss values. Use these as unit-test vectors.

| Phase | Target (size, Def, soft) | Captured | Expected | Verdict |
| :--- | :--- | :--- | :--- | :--- |
| P1 (ATK1 1, STR 1; W 16..17) | Poring (M, 2, 1) | 12 (x5), miss x3 | W*75/100 = 12 both ends: 12 | fit, **only with size 75** |
| P2-P6 (ATK1 2) | Poring | 14 (x28), miss x4 | 14 | fit |
| P7 (ATK1 15, STR 11; W 17..18) | Lunatic (S, 18, 3) | 42, 42, 43, 43, miss x1 | 42..43 | fit |
| P7 before stat points (ATK1 2) | Lunatic | miss x3 (lines 1921-1926) | hit rate 73%, 3 straight misses p ~2% | plausible, see 7.5 |
| P8 (ATK1 48; W 17..18) | Pupa (S, 24, 3) | 104, 105, 105 | 104..105 | fit |
| P9 (ATK1 49, STR 12; W 17..18) | Pupa | 106, 106, 107, 107 | 106..107 | fit |
| P9 crit (line 2960) | Pupa | 149, `type 10` | W=18: (98+18)*4024/4240 = 110, -3 = 107, x1.4 = 149 | fit (crit uses atkmax, x1.4 after DEF, floor) |
| P10 (ATK1 53, DEX 15; W 15..16, +25 arrow) | Pupa | 135, 135, 136 | 135..136 | fit |
| P11 (ATK1 20) | Pupa | 72, 73, 73 | 72..73 | fit |
| P12 (ATK1 10, DEX 5; W 14..16) | Pupa | 54 | 52..54 | fit (top of range) |
| P13 (ATK1 13, DEX 8; W 14..16) | Marine Sphere (S, 41, 43) | 16, 18, 18, 18 | 16..18 | fit |
| B, Novice ATK1 1, Knife, STR 1 (aid 2000001, `#6`) | Pupa | 14, 14, 15, miss x2 | W 16..17: 14..15 | fit (Small = 100) |

Worked example (P1): `batk` = (10 + 2 + 3 + 2)/10 = 1, `statusAtk` = 2, Knife W = 16 or 17, x75/100 = 12 either way; `(2+12)*4002/4020 = 13`, minus `(1+1)/2 = 1` = **12**.

### 7.4 What the capture settles

* **Confirmed:** `statusAtk = 2*batk` (ATK1 push); Renewal DEF formula with hard Def `(4000+def)/(4000+10*def)` and soft DEF `(MobLv + MobVit)/2` for mobs (Pupa Def 24 and Marine Sphere Def 41/soft 43 both fit); crit = `atkmax` weapon roll, DEF applied, then `x1.4` floor; `batk` and the weapon bonus use DEX for a Bow; arrow ATK 25 is a flat add after the weapon roll (no size fix, no variance); a miss has `damage 0`, `type 0`, `dmg_speed 0`.
* **Corrected:** the section 3 size bullet was wrong (it read only the Renewal overlay); the full table has Dagger M75/L50, Bow L75, and so on. Nothing else in sections 3-5 needed a change.
* **Not exercised:** minimum damage 1, Large targets, element other than Neutral weapon (Water 1 and 2, Earth 1, Neutral 3 targets all gave 100%), Double Attack, `damage2`, Res.
* **Source gap closed:** the capture matches rAthena exactly once `db/size_fix.yml` (base) is included. Dagger Large 50 is now source-known, still unobserved.

### 7.5 Hit rate sanity

`hitrate = clamp(HIT - flee, 5, 100)` (+ Vulture's Eye level for a player in Renewal, `battle.cpp:3284-3287`). Poring flee 102: 40 swings, 7 misses (expected rate 23-25%). Lunatic flee 106: 8 swings, 4 misses (HIT 179 gives 73%; three misses in a row at the start is a ~2% event, not flagged). Pupa (flee 105): 0 misses in 15 A swings at HIT 193-236 (88-100%). Marine Sphere flee 167, HIT 196 + Vulture 6 = 35%: 4 hits in 7 swings. No lucky dodge (type 11) and no crit on a mob, as expected.

### 7.6 `0x0977` and swing timing

* `0x0977` `{mob id, hp, max_hp}` follows every damaging swing at **swing + amotion** (A's `src_speed`): offset from `amotion` is -23..+29 ms, mean +7 (n=56, host/proxy jitter). hp = previous hp minus the shown damage in all 56 cases; max_hp = db `Hp` (55, 48, 51, 1831). A miss produces no `0x0977`. Only damage-log members get it (A; B got 3 in total, when B had hit that mob).
* Swings repeat every **2 x amotion** (`adelay`): 590 gives 1164-1205 ms, 580 gives 1165-1177, 540 gives 1082-1105. First swing after a standing click: 5-7 ms (`0x0437` to `0x08c8`).
* Overkill: A's 107 at line 3122 (04.198) and B's 14 at line 3128 (04.569) were both shown; the mob died when A's hit landed (`0x0977 {0}` at 04.785 = A's swing + 587) and B's later hit was dropped (it would have landed at 05.159).

### 7.7 Kill order and vanish

See [mobs.md 3.1](./mobs.md#31-packet-order-for-one-normal-kill-single-player-loot-no-party-at-default-conf) (capture note). Summary over 27 kills with an `0x0977 {0}` for A (and 7 victims of the Sphere's explosion without one): `0x0977 {0}` then exp then drops, all in one millisecond, vanish type 1 **245-273 ms** later (median ~252) to A and B alike; `0x0add` for B arrives with A's, the same ms. Drops do happen (71 `0x0add` frames across both sessions: Jellopy 909, Fly Wing 601, 915, 705...; up to 6 from one Poring); we will not implement drops.

### 7.8 Ranged attack and ammo packets

* **Equip ammo** (lines 3429-3431, 4335-4337): `0x0998 {idx 10, pos 0x8000}` -> `0x0983` (EFST 695, to A **and B**) -> `0x013c {0a 00}` -> `0x013b {03 00}`. Matches [equipment.md](../packets/equipment.md).
* **Each shot** (lines 3433-3441, 3470-3475, 4040-4042, 4339-4343): `0x0437` -> 5-7 ms -> `0x0088` x2 (x3 if A was mid-walk, lines 4473-4475) -> **`0x07fa {01 00, idx 0a 00, amount 01 00}`** (reason 1) -> **`0x00b0 SP_WEIGHT`** (weight -1 = 0.1 unit per arrow) -> `0x08c8` (A and B). Auto-repeat shots (no new click) skip the `0x0088`: `0x07fa`, weight, `0x08c8` (lines 4052-4054, 4409-4411). **A miss also consumes an arrow** (lines 4052-4054, 4059-4061, 4476-4478). B sees only `0x08c8`, no `0x07fa`/`0x00b0`/`0x099a`. No `0x013c` or status packet per shot.
* **Last arrow** (10 arrows, 10th shot, lines 4058-4062): `0x099a {idx 0a, pos 00 80, result 0}` (ammo unequipped) -> `0x07fa` -> `0x00b0 SP_WEIGHT` -> `0x08c8` (the shot still fires, here a miss). No ATK push. The stack is gone: a later `@item arrow` (line 4320) makes a new stack at index 10, not equipped; `0x0b41` merges `@item arrow 100` into it; the client re-equips (4334).
* **No ammo:** the auto-repeat 1089 ms after the last shot gets `0x013b {00 00}` and the loop ends, no `0x0088`, no `0x08c8` (line 4079). A new click with no ammo: `0x0088` x2 (x3 mid-walk, lines 4218-4224, 4298-4301) -> `0x013b {00 00}`. Nothing is consumed.
* **Bow range:** shots landed at client distance up to **9** with range 11 (lines 4343, 4402: A (139,174)/(140,176) vs a Sphere at (148,172)), and 4-5 with range 5. The boundary was not probed.
* **Level gate:** `0x0999` result 1 for a Bow at base level 3 (item `EquipLevelMin 4`).

### 7.9 Line of sight ("obstruction", "not shooting")

Map `prontera`. Cells from rust-ro's `prontera.mcache` (value 1 = wall); the only wall near the test is a 2x2 block at (145..146, 175..176); a longer wall at (134..138, 172..175). Algorithm = [movement_aoi.md 5](./movement_aoi.md#5-line-of-sight-path_search_long) (re-checked against `path.cpp:128-188`). Target Marine Sphere at (143,172) (its `0x09fe`/`0x09ff`).

| A position | Line | LOS by algorithm | Server reply to `0x0437` |
| :--- | :--- | :--- | :--- |
| (148,174), walking | 4468-4478 | clear | `0x0088` x3, `0x07fa`, weight, `0x08c8` (miss) |
| (147,178) | 4487-4488, 4503-4505 | blocked by (145..146,175..176) | **one `0x0088` only** |
| (150,180) | 4512-4519 | blocked | one `0x0088` |
| (148,182) | 4550-4551 | blocked | one `0x0088` |
| (133,174) | 4590-4596 | blocked by (134..138,..) | one `0x0088` |

So a request that fails the LOS test gets **exactly one `0x0088` (the `clif_fixpos` from `unit_attack`) and nothing else**: no `0x0139`, no `0x013b`, no `0x08c8`, no ammo use, and the attack loop is dropped (`unit_attack_timer_sub` returns 0). Compare: in range with clear LOS but no ammo = two or three `0x0088` then `0x013b`. Open: lines 4195-4199 (A (157,173), no ammo, Pupa 110028182 at (152,176)) also got a single `0x0088` and no `0x013b`; the algorithm says clear with rust-ro's cache, so either the real map cache has a wall on that line or another early exit applied.

### 7.10 Marine Sphere ("oops")

Not a bug: Marine Sphere has the mob skill `NPC_SELFDESTRUCTION` (173) at idle after HP drops below 99% (`mob_skill_db.txt:724-727`, rate 5%, cast 2000 ms). Each time (lines 4101-4112 and 4424-4426): `0x09cb {173 | 1 | sphere | sphere | 1}`, one `0x08c8` per victim (`src` = sphere, type 0, `dmg_speed` 1, **damage = floor(sphere current hp x 1.5)**: 1813 -> 2719, 1779 -> 2668), then `0x0977 {0}` for A (A is in its damage log; exp 170/149 after the level penalty), then drops 4 at +0/+20/+20/+20 ms, then vanish type 1 for the victims (5 Pupas, +249 ms) and the sphere (+261..265 ms). Victims killed by the sphere give A no `0x0977`/exp. The sphere's own damage log still pays A out. Not planned.

## Catalog status

* Normal player-vs-mob attack: covered here; verified against a real capture for Knife and Bow ([section 7](#7-capture-combat_sequence_1txt-damage-vectors-death-ammo)).
* **Not yet catalogued:** magic damage (`battle_calc_magic_attack`), skill ratios (`src/map/skills/<job>/*.cpp`), `attr_fix` matrix (`db/re/attr_fix.yml`), refine table (now in [stats.md](./stats.md#refine-bonuses-weapon-atk-weapon-matk-armor-def), [../db/refine.md](../db/refine.md)), monster-vs-player damage, PvP/GvG caps.
