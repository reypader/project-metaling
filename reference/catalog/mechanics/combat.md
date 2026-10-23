# Combat Damage Formulas and Modifier Tables

Target client version: `PACKETVER = 20260219`, Renewal (`RENEWAL` and `RENEWAL_ASPD` defined). Citations are `$RATHENA_PATH/src/map/...` unless noted. This file currently covers the **normal (non-skill) melee attack of a player against a monster**; skills, magic, and player-vs-player are not covered. Lifecycle (request, timer, packets, delays): [auto_attack.md](./auto_attack.md). No capture of a real attack exists; formulas are source-derived and unverified against the client.

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
| **Equip ATK** `equipAtk` | `status.eatk` (sum of gear `bonus bAtk`-style equip ATK), element fix with weapon ele | weapon ele |
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
* `size_mod`: **100 for every weapon and size except Knuckle and Whip vs Large = 75** (`db/re/size_fix.yml`, Renewal).
* Element table `attr_fix` (attacker element x defender element/level) multiplies each part as a percent; Neutral vs Neutral/most = 100. Elements, element-changing cards and the full table are **deferrable**: first implementation can use 100%. Poring is Water 1; Neutral vs Water = 100%.
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
* "Plant" mobs (`MD_...`/`is_infinite_defense`, e.g. Mushroom/Emperium-class "1 dmg" mobs): `battle_calc_attack_plant` -> 1 damage per hit if hit, 0 if miss; also defer-able.

## 5. Worked skeleton for a first implementation

Player (Novice, fist, base level L, STR/DEX/LUK/AGI known) vs a mob (level `ML`, `def` D, vit V, flee `ML+agi+100`):

1. `hit = L + dex + luk/3 + 175`; `flee = ML + mob_agi + 100`; `hitrate = clamp(hit - flee, 5, 100)`; roll `rnd()%100 < hitrate`.
2. `cri = 10 + 3*luk + L/10 - mob_luk*2`; `rnd()%1000 < cri` -> crit (auto-hit, skip step 1).
3. `batk = (str*10 + dex*2 + luk*10/3 + L*10/4)/10`; `damage = 2 * batk + weapon_roll` where `weapon_roll` = the variance formula above (0 bare-handed).
4. `damage = damage * (4000 + D)/(4000 + 10*D) - (ML + V)/2`; `max(damage, 1)`.
5. Crit: `damage = damage * 1.4` (floor).
6. Result: damage, `type` 10 for crit else 0 (miss -> damage 0, type 0).

**Deferrable for a first cut:** element table, size table (all 100 except Knuckle/Whip), cards and `bonus` rates, masteries, Double Attack, status-change modifiers, Res, plant mobs, overrefine, dual wield (`damage2`), Triple Attack, ammo for bows.

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

## Catalog status

* Normal player-vs-mob attack: covered here.
* **Not yet catalogued:** magic damage (`battle_calc_magic_attack`), skill ratios (`src/map/skills/<job>/*.cpp`), `attr_fix` matrix (`db/re/attr_fix.yml`), refine table, monster-vs-player damage, PvP/GvG caps.
