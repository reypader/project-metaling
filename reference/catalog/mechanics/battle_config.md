# Battle / Player Config Knobs Read by Player Stat Formulas

Target: `PACKETVER = 20260219`, renewal build. Source: `rathena` at `$RATHENA_PATH`, commit `d4b8e7b8f`. Registry table: `src/map/battle.cpp:8254-8906` (`battle_data[]`, columns: name, variable, default, min, max; out-of-range values are rejected/clamped to the column bounds at load). Post-load scaling: `battle_adjust_conf()` `battle.cpp:8962-8977`. Shipped values: `conf/battle/*.conf`. Formulas that consume these: [stats.md](./stats.md), [combat.md](./combat.md), [experience.md](./experience.md).

## Which default to use

Two defaults exist: the compiled default in `battle_data[]` and the value in the shipped `conf/battle/*.conf` (loaded at startup, wins). They DIFFER for several knobs (flagged "differs" below). "rAthena defaults" for Metaling = the shipped conf value. Commented-out conf lines (`//natural_heal_weight_rate`, `//open_box_weight_rate`) mean the compiled renewal default applies.

`BL_*` bitmask for the `enable_*`/`*_target` knobs (`map.hpp:293-305`): PC 0x1, MOB 0x2, PET 0x4, HOM 0x8, MER 0x10, ITEM 0x20, SKILL 0x40, NPC 0x80, CHAT 0x100, ELEM 0x200, ALL 0xFFF.

## 1. Derived-stat enables

| Knob | conf (file:line) | Compiled default (`battle.cpp`) | Shipped | Read at | Effect / renewal |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `enable_baseatk_renewal` | `battle.conf:14` | `:8258` BL_ALL | `0x29F` (= PC, MOB, PET, HOM, MER, NPC 0x80, ELEM 0x200) | `status.cpp:2432` (`status_base_atk`, renewal build only) | If `bl->type & mask` is 0, status ATK part is 0. Renewal reads THIS knob; `enable_baseatk` (`battle.conf:13`, `0x9`, `:8257`) is the pre-renewal one and is ignored. |
| `enable_critical` | `battle.conf:21` | `:8254` BL_PC | `17` (= 0x11: PC + MER) | `status.cpp:2723` (`status_calc_misc`) | Type not in mask: `cri = 0` (no crit ever). Otherwise renewal `cri += level/10 + 10 + luk*3`, clamp 1..SHRT_MAX. |
| `enable_perfect_flee` | `battle.conf:17` | `:8259` BL_PC\|BL_PET | `1` (PC only) | `status.cpp:2735` | Type not in mask: `flee2 = 0`. Otherwise `flee2 += luk + 10`, clamp 0..SHRT_MAX. |
| `critical_rate` | `battle.conf:25` | `:8256` 100 | 100 | `status.cpp:2756` | Scales `cri` of NON-PC, NON-MOB types (pets/homun/merc/elem). Players are explicitly skipped (`status.cpp:2753-2755`), so it does not affect player stats. |
| `mob_critical_rate` | `battle.conf:24` | `:8255` 100 | 100 | `status.cpp:2747` | Mobs only; not a player knob. Listed for completeness. |

No config knob feeds HIT, FLEE, soft/hard DEF/MDEF, or MATK in `status_calc_misc` (`status.cpp:2600-2760`); those are pure formulas of stats/level/equipment.

## 2. HIT rate (combat.md section 1)

All in `battle_calc_hit`-style function `battle.cpp:3236-3317` (and the skill variant `:6608-6633` for magic/misc).

| Knob | conf | Default | Read at | Effect |
| :--- | :--- | :--- | :--- | :--- |
| `min_hitrate` | `battle.conf:78` | 5 (range 0..100) | `battle.cpp:3312` | Lower clamp of `hitrate`. If `min > max`, min is lowered to max at load (`battle.cpp:8975`). |
| `max_hitrate` | `battle.conf:79` | 100 (0..100) | `battle.cpp:3312` | Upper clamp. Clamp happens before the flat +20 Rapid Smiting add (`:3315`). |
| `agi_penalty_type` | `battle.conf:85` | 1 (0..2) | `battle.cpp:3265`, `:6613` | 0 off; 1 = percent: `flee = flee*(100 - (n-(count-1))*num)/100`; 2 = flat: `flee -= (n-(count-1))*num`. `n` = number of mobs targeting the defender. |
| `agi_penalty_target` | `battle.conf:89` | 1 = BL_PC | `:3265` | Defender types penalized (`& target->type`). |
| `agi_penalty_count` | `battle.conf:92` | 3 (min 2) | `:3268` | Penalty starts when attacker_count >= count. |
| `agi_penalty_num` | `battle.conf:95` | 10 | `:3270-3272` | Per-extra-attacker amount (percent or flat per type). |

Renewal note: base `hitrate` starts at 0 in renewal (`:3259-3263`) vs 80 pre-renewal; all five knobs above still apply in renewal. Defaults penalize players: 3 attackers = -10% FLEE... precisely `(n-2)*10` percent for `n>=3`, so 3 mobs = -10%, 4 = -20%.

## 3. DEF / MDEF

| Knob | conf | Default | Read at | Effect / renewal |
| :--- | :--- | :--- | :--- | :--- |
| `vit_penalty_type` | `battle.conf:101` | 1 (0..2) | `battle.cpp:4780` | Same scheme as AGI penalty, applied to hard DEF (`def1`) and soft DEF (`def2`) of defenders in `vit_penalty_target`. Reads in renewal too (outside `#ifndef RENEWAL`). |
| `vit_penalty_target` | `battle.conf:105` | 1 = BL_PC | `:4780` | |
| `vit_penalty_count` | `battle.conf:108` | 3 (min 2) | `:4784-4789` | `target_count = min(targeted_by, 100/num + count-1)`. |
| `vit_penalty_num` | `battle.conf:111` | 5 (min 1) | `:4784-4793` | Percent (type 1) or flat (type 2) per extra attacker. |
| `weapon_defense_type` | `battle.conf:117` | 0 | `battle.cpp:4839` | If > 0: `vit_def += def1*type; def1 = 0` (hard DEF becomes flat soft DEF). Default 0 = off, so renewal ignores in practice. Also gates `max_def` clamp at load (below). |
| `magic_defense_type` | `battle.conf:120` | 0 | `battle.cpp:6111` | Pre-renewal magic branch only (`#else` of RENEWAL). Renewal MDEF reduction (`:6100-6105`) ignores it. |
| `max_def` | `player.conf:132` | 99 (0..INT_MAX) | `status.cpp:4587-4606` | Only inside `#ifndef RENEWAL`: caps hard DEF/MDEF at `max_def`, overflow to `def2` x `over_def_bonus`. Renewal build: NOT read by player DEF/MDEF. Load rule: if `max_def > 100` and `weapon_defense_type == 0`, `max_def = 100` (`battle.cpp:8972`). |
| `over_def_bonus` | `player.conf:137` | 0 (0..1000) | `status.cpp:4588,4605` | Same `#ifndef RENEWAL` blocks. Ignored in renewal. |
| `max_res_mres_ignored` | `player.conf:314` | 50 (0..100) | `battle.cpp:5603`, `:6063` | Caps RES/MRES ignore % (4th-job). `RES`/`MRES` formulas live in combat code, not stat window. |

Renewal hard DEF/MDEF cap comes from the data type clamp (`DEFTYPE_MIN/MAX`, `status.cpp:4580-4600`), not from config.

## 4. ASPD

Conf lines `player.conf:80,83,86,89`. All are ASPD-point values (100..199), converted at load in `battle_adjust_conf` (`battle.cpp:8965-8968`):

```
cfg = (AMOTION_ZERO_ASPD - cfg * AMOTION_INTERVAL) * AMOTION_DIVIDER_PC
      = (2000 - aspd*10) * 2          # AMOTION_ZERO_ASPD 2000, INTERVAL 10, DIVIDER_PC 2 (status.hpp:61-67)
```
So 190 -> 400, 193 -> 340 (stored as doubled amotion). `MIN_ASPD` = 8000 (`status.hpp:43-44`) is a constant, not config.

| Knob | conf | Default (compiled = shipped) | Class selector (`pc_maxaspd`, `pc.cpp:15254-15257`) |
| :--- | :--- | :--- | :--- |
| `max_aspd` | `player.conf:80` | 190 (`:8400`) | Fallback: everything else |
| `max_third_aspd` | `player.conf:83` | 193 (`:8401`) | `class_ & JOBL_THIRD` (checked first) |
| `max_extended_aspd` | `player.conf:86` | 193 (`:8747`) | `MAPID_SECONDMASK` is Kagerou/Oboro or Rebellion |
| `max_summoner_aspd` | `player.conf:89` | 193 (`:8402`) | `MAPID_FIRSTMASK == MAPID_SUMMONER` |

Used: `status.cpp:4618-4620` (base) and `:6415` (final): `amotion = clamp(amotion, pc_maxaspd(sd)/2, MIN_ASPD/2)`, i.e. lower clamp `2000 - maxaspd*10`. Applies in renewal (the renewal ASPD path is the one that uses them). Note: the chain only tests JOBL_THIRD, extended, summoner; 4th-job classes carry `JOBL_THIRD` in their mapid (e.g. Shadow Cross `0x11106`), so they use `max_third_aspd`. **Captured:** a Shadow Cross reached amotion 80 (below the 190 floor of 100; [captures/attribute_sequence.txt](../captures/attribute_sequence.txt) line 4243), consistent with the 193 cap. Mob/non-PC ASPD uses constants `MAX_ASPD_NOPC` instead (`status.cpp:5099`).

## 5. MaxHP / MaxSP / MaxAP

| Knob | conf | Compiled -> shipped | Read at | Effect |
| :--- | :--- | :--- | :--- | :--- |
| `hp_rate` | `player.conf:13` | 100 (min 1) -> 100 | `status.cpp:4355`, `:6202` | If != 100: `max_hp = (uint32)(hp_rate * (max_hp/100.0))` (float, truncated) applied after the class formula, before cap. |
| `sp_rate` | `player.conf:16` | 100 -> 100 | `status.cpp:4368`, `:6225` | Same for SP. |
| `max_hp_lv99` | `player.conf:98` | 330000 -> 330000 | `status.cpp:4359`, `:6206` | Cap when `base_level < 100`. |
| `max_hp_lv150` | `player.conf:99` | 660000 -> 660000 | `status.cpp:4361`, `:6208` | Cap when `100 <= base_level < 151`. |
| `max_hp` | `player.conf:100` | 1100000 -> 1100000 | `status.cpp:4363`, `:6210` | Cap when `base_level >= 151`. |
| `max_sp` | `player.conf:103` | 32500 -> **1000000** (differs) | `status.cpp:4371`, `:6228` | Cap, all levels. Min clamp for both HP and SP is 1. |
| `max_ap` | `player.conf:318` | 200 -> **1000** (differs) | `status.cpp:4379`, `:6499` | 4th-job AP cap. `ap_rate` (`player.conf:321`, 100) analog at `status.cpp:4375`. |
| `restart_hp_rate` | `player.conf:24` | 0 | `status.cpp:4389-4392` | Respawn HP, not part of max formulas. |

Order for HP: class formula -> `hp_rate` -> level cap -> (later) equipment/SC % modifiers in `status_calc_bl_main` (`:6196-6212` reapplies rate and cap after those modifiers). Also `pc.cpp:10494-10510` re-caps when a script sets max HP/SP.

## 6. Weight

| Knob | conf | Default | Read at | Effect |
| :--- | :--- | :--- | :--- | :--- |
| `natural_heal_weight_rate` | `player.conf:69` (commented out) | 70 renewal / 50 pre-renewal (`battle.cpp:8391-8395`, range 0..100) | `pc.cpp:3030`; also sent to client `clif.cpp:22048` | Weight percent `weight*100/max(max_weight,1)` (integer, `pc.cpp:3012-3016`) at/above which the 50% icon (SC_WEIGHT50) shows and `regen.state.overweight` is set (HP/SP regen halved, `status.cpp:15495-15509`). Comparison is `>=`. |
| `major_overweight_rate` | `player.conf:77` | 90 (`battle.cpp:8906`) | `pc.cpp:3030` | `>=` this percent: SC_WEIGHT90 (cannot attack/use skills/heal). |
| `open_box_weight_rate` | `player.conf:73` (commented, unverified line) | 90 renewal / 70 | `pc.cpp:6395` | Item box open limit; not a stat formula. |
| `max_cart_weight` | `player.conf:140` | 8000 | `battle.cpp:8970` (x10 at load -> 80000), `status.cpp:3722` | Cart base capacity in 0.1 units; GN_REMODELING_CART adds `5000*lv`. |

Player max weight itself (`job max_weight + STR*300 + bonuses`, `status.cpp:3663-3690`) has no config knob.

## 7. Natural regen timing

| Knob | conf | Default | Read at | Effect |
| :--- | :--- | :--- | :--- | :--- |
| `natural_healhp_interval` | `player.conf:57` | 6000 ms (min `NATURAL_HEAL_INTERVAL`) `battle.cpp:8387` | `status.cpp:15548` | `rate = interval / (regen.rate.hp/100 * multi)`. |
| `natural_healsp_interval` | `player.conf:60` | 8000 ms `:8388` | `status.cpp:15577` | Same for SP. |
| `natural_heal_skill_interval` | `player.conf:63` | 10000 ms `:8389` | `status.cpp:15498-15639` | Skill-regen (HP/SP recovery skills) tick. |

See [stats.md](./stats.md#hp--sp-natural-regeneration) for how the 500 ms tick and amounts combine with these.

## 8. Max stats (per class group) and stat-up rules

`max_*_parameter` knobs are copied into each job's `max_param[STR..LUK]` once at job DB load (`pc.cpp:14338-14390`), only when the job entry in `db/re/job_stats.yml` has `max_param == 0` for that stat (explicit per-job YAML values win). Runtime reads go through `pc_maxparameter(sd, param)` = `job->max_param[param]` (`pc.cpp:15234-15244`). Selection order (first match wins; `pc.cpp:14342-14376`):

| Order | Condition | Knob | conf line | Compiled | Shipped |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 0 | else (default) | `max_parameter` | `player.conf:114` | 99 | 99 |
| 1 | Baby and Third | `max_baby_third_parameter` | `:119` | 108 | **117** |
| 2 | Baby | `max_baby_parameter` | `:118` | 80 | 80 |
| 3 | Summoner | `max_summoner_parameter` | `:121` | 120 | **130** |
| 4 | Kagerou/Oboro/Rebellion | `max_extended_parameter` | `:120` | 125 | **130** |
| 5 | `pc_is_trait_job` (4th job) | `max_fourth_parameter` | `:122` | 135 | **130** |
| 6 | Third and (Upper or primary 4th) | `max_third_trans_parameter` | `:117` | 135 | **130** |
| 7 | Third | `max_third_parameter` | `:116` | 135 | **130** |
| 8 | Upper (transcendent) | `max_trans_parameter` | `:115` | 99 | 99 |

Trait stats (POW STA WIS SPL CON CRT, `PARAM_POW..`): `max_trait_parameter` (`player.conf:310`, compiled 100, **shipped 110**) for trait jobs, else 0 (`pc.cpp:14393-14401`). All `max_*_parameter` range 10..SHRT_MAX. The Marionette buff caps ally stats with plain `max_parameter` (`status.cpp:11407`).

Where the cap bites: stat-up request cost check returns 0 cost at max (`pc_need_status_point`, `pc.cpp:8811-8830`); `pc_maxparameterincrease` (`:8841-8860`, multi-point stat-up); `pc_statusup`/`pc_statusup2` (`:8888`, `:8946`) reject `current + inc > max`; trait versions `:8975-9107`; script `setparam` clamps `1..max` (`:10530-10554`; trait `0..max`). Cost formula: [stats.md](./stats.md#stat-point-costs-and-limits).

### Bypass (`atcommand_max_stat_bypass`)

There is no `atcommand_max_stat_bypass` battle knob in this rAthena. The bypass is the per-group permission `bypass_max_stat` (`conf/groups.yml:241`, `PC_PERM_BYPASS_MAX_STAT`), checked in `@str/@agi/...` (`atcommand.cpp:2980-2990`) which then caps at SHRT_MAX instead of `pc_maxparameter`. Normal stat-up packets never bypass. Out of scope for Metaling unless GM commands are added.

## 9. Stat points, level-up, EXP

| Knob | conf | Default | Read at | Effect |
| :--- | :--- | :--- | :--- | :--- |
| `start_status_points` | `conf/inter_athena.conf:16` (NOT a battle conf) | 48 (`map.cpp:5348`, parsed `map.cpp:4370`) | `pc.cpp:14526-14536` | Points at base level 1. With `use_statpoint_table` on, a mismatching level-1 row in `db/re/statpoint.yml` is an error and is overridden by this value. |
| `use_statpoint_table` | `exp.conf:107` | `yes` (`battle.cpp:8578`) | `pc.cpp:14519-14560` | yes: per-level points from statpoint.yml; missing levels logged and generated. no (or missing row): `points(level) = points(level-1) + (level-1+15)/5` for `level <= 200`, constant above 200. Level-up grants `pc_gets_status_point(base_level)` evaluated BEFORE `base_level++` (`pc.cpp:8258-8260`, i.e. the entry for the old level). |
| `use_traitpoint_table` | `exp.conf:112` | yes | `pc.cpp:14550+` | Same for trait points; generated rule for level > 200: `(l-200)*3 + (l-200)/5*4`, else 0. |
| `multi_level_up` | `exp.conf:19` | `no` | `pc.cpp:8255` | Off (or below the `_base` threshold): after a level-up, leftover exp is capped to `next-1` so at most one level per kill/exp gain. On: unlimited carry. |
| `multi_level_up_base` | `exp.conf:24` | 0 | `pc.cpp:8255` | With multi_level_up on: only allow multi-level when `base_level >= value` (0 = always). |
| `multi_level_up_job` | `exp.conf:25` | 0 | `pc.cpp:8330` | Same for job level. |
| `max_lv` | `client.conf:49` | 99 | `clif.cpp:1014-1019` | Client display aura/level cap only; real max level comes from the job DB. |
| `base_exp_rate` / `job_exp_rate` | `exp.conf:12,15` | 100 | exp gain | Rates, see [experience.md](./experience.md). |

## 10. Knobs the stat formulas do NOT read

No config in: status ATK (except the enable mask), MATK, soft DEF/MDEF, HIT, FLEE, renewal ASPD base value, MaxHP/MaxSP class formulas (only rate/caps), max weight, `max_walk_speed` (`player.conf:92`, 300, converted `100*150/value` at load, `battle.cpp:8969`, floor on walk delay `status.cpp:6087`; movement, not stats).

## Metaling `BattleConfig` fields implied (all with shipped defaults)

`enable_baseatk_renewal=0x29F`, `enable_critical=0x11`, `enable_perfect_flee=0x1`, `min_hitrate=5`, `max_hitrate=100`, `agi_penalty_{type=1,target=1,count=3,num=10}`, `vit_penalty_{type=1,target=1,count=3,num=5}`, `weapon_defense_type=0`, `max_{aspd=190,third_aspd=193,extended_aspd=193,summoner_aspd=193}`, `hp_rate=100`, `sp_rate=100`, `max_hp_lv99=330000`, `max_hp_lv150=660000`, `max_hp=1100000`, `max_sp=1000000`, `max_ap=1000`, `natural_heal_weight_rate=70`, `major_overweight_rate=90`, `max_cart_weight=8000`, `natural_heal{hp=6000,sp=8000,skill=10000}`, the nine `max_*_parameter` (99/99/130/130/80/117/130/130/130) + `max_trait_parameter=110`, `start_status_points=48`, `use_statpoint_table=true`, `multi_level_up=false` (+base/job 0). Skip in renewal: `enable_baseatk`, `max_def`, `over_def_bonus`, `magic_defense_type`.
