# Base and Derived Stat Formulas

Target client version: `PACKETVER = 20260219`, renewal build (`RENEWAL`, `RENEWAL_ASPD`, `RENEWAL_STAT` all defined in `src/config/renewal.hpp:55-74`). Provenance for every section: `rathena` at `$RATHENA_PATH`, commit `d4b8e7b8f` (2026-10-06), paths `src/map/status.cpp`, `pc.cpp`, `pc.hpp`, `conf/battle/*.conf`, `db/re/*.yml`. The derived-stat formulas below are checked against the captured status values in [Captured status values](#captured-status-values) ([captures/attribute_sequence.txt](../captures/attribute_sequence.txt)); sections not covered there are source-only. Wire packets: [../packets/status.md](../packets/status.md). Static job data (HP/SP tables, weights, ASPD bases, job bonuses): [../db/job_stats.md](../db/job_stats.md). Exp tables, level-up flow: [experience.md](./experience.md), [../db/exp_tables.md](../db/exp_tables.md).

All divisions are C integer division (truncate toward zero) unless a term is written with a decimal. Terms called "float" are computed in `float` and truncated once at the end, as marked.

Config knobs read by these formulas (caps, rates, penalties, defaults, load-time scaling): [battle_config.md](./battle_config.md).

## Derived Stat Formulas

### Stat sources (the six primary stats)

`status_calc_pc_sub` (`status.cpp:3754-4500`) rebuilds the player's BASE status from zero each time, then copies it to the battle status (SC/buff effects are layered later by `status_calc_bl_main`). For each of STR, AGI, VIT, INT, DEX, LUK and the six trait stats (POW, STA, WIS, SPL, CON, CRT) (`status.cpp:4247-4313`):

```
base_status.<stat> = job_bonus[job_level - 1].<stat>      (cumulative table from job_stats.yml)
                   + 10 for Super Novice (joblv >= 70 or third-class flag) with die_counter == 0, STR..LUK only
                   + passive skill bonuses (Hilt Binding STR+1, Dragonology INT +(lv+1)/2, Owl's Eye DEX+lv, ...)
                   + status.<stat>                          (allocated points, the DB column)
                   + card/script bonuses + equipment bonuses
cap 0..65535
```

The `0x0141` packet reports `base = status.<stat>` (allocated) and `bonus = battle - status` (job + equip + buffs). Job bonus level index is the character's job level (1-based, clamp by job max); tables are accumulated at load (see job_stats.md). Trait stats (POW..CRT) are 0 for non-4th classes (max 0).

### Base level / job tables used below

`level` = base level (`status.base_level`). `status_calc_misc(sd, base_status, base_level)` (`status.cpp:2600-2760`) fills the derived stats; values it adds on top of equipment/script bonuses already accumulated in the same fields (hit, flee, def2, mdef2, cri, flee2, patk, smatk, res, mres, hplus, crate).

### Status ATK (left side of ATK in the window)

`status_base_atk` (`status.cpp:2424-2480`), applied only if `enable_baseatk_renewal` includes players (default `0x29F`, which includes `BL_PC`):

```
ranged = weapon in {Bow, Musical, Whip, Revolver, Rifle, Gatling, Shotgun, Grenade}
(s, d) = ranged ? (DEX, STR) : (STR, DEX)        # s is the "main" stat, battle-status values
batk   = (s*10 + d*10/5 + LUK*10/3 + level*10/4) / 10  +  5*POW
```
Then clamped 0..65535 and `status->batk += ` this plus equipment base-ATK bonuses; `SCB_BATK` recomputes it when STR/DEX/LUK change in a buff. Weapon ATK (right side) = `watk + watk2 + eatk` (weapon base ATK incl. refine and overrefine, plus equip ATK); formulas for weapon ATK/refine/size are combat territory: [combat.md](./combat.md).

### Status MATK and MATK window sides

Status MATK (left side, identical min and max for players) (`status.cpp:2559-2595`):

```
status_matk = INT + INT/2 + DEX/5 + LUK/3 + level/4 + 5*SPL
```
Full min/max MATK (`status_calc_bl_main`, `status.cpp:6249-6345`):
```
wMatk   = rhw.matk + lhw.matk                      # weapon MATK incl. refine
var     = rhw.matk*rhw.wlv/10 + lhw.matk*lhw.wlv/10   # weapon-level variance
matk_min = status_matk + 3*soulball + wMatk - var
matk_max = status_matk + 3*soulball + wMatk + var
if rhw.overrefine>0: matk_min += 1; matk_max += overrefine - 1
Magic Power: *(100+val3)/100 ; then + ematk + ematk_hidden + pseudobuff + consumable ; then * matk_rate/100
cap 0..65535
```
What the window shows: left = status MATK (computed on the battle status), right = `rhw.matk + lhw.matk + ematk + consumable + pseudobuff` ([packets/status.md](../packets/status.md#0x00bd-zc_status)).

**Captured** ([captures/equip_sequence_4.txt](../captures/equip_sequence_4.txt), write-up in [packets/equipment.md](../packets/equipment.md#dead-costumes-accessories-and-refine-capture-4)): `SP_MATK1` (43) = the right side, `SP_MATK2` (44) = the left side. A Mage with a Rod +10 (MATK 30, weapon level 1) gets `SP_MATK1` 50 and `SP_MATK2` 27; a Knife +10 (MATK 0) gives `SP_MATK1` 20. So a level-1 weapon's refine adds 2 MATK per level, as it adds 2 ATK (`SP_ATK2` 17 → 37 for the Knife). The relog `0x00bd` has `max_mattPower` = `SP_MATK1`, `min_mattPower` = `SP_MATK2`. Armor +10 adds 18 to `SP_DEF2` (Hat 2 → 20, Silk Robe 20 → 38); the Silk Robe's script MDEF 10 goes to `SP_MDEF2`.

### DEF and MDEF (renewal)

> Open findings: [PF-19](../../../PACKET_FINDINGS.md#pf-19).

Soft (stat-derived, "left" in the window, stored as `def2` / `mdef2`; players):

```
SoftDEF  = equip_def2 + (int)( (level + VIT)/2.0 + AGI/5.0 )                     # one float expression, truncated
SoftMDEF = equip_mdef2 + (int)( INT + level/4.0 + (DEX + VIT)/5.0 )
clamp 0..32767
```
(`status.cpp:2640-2672`; monsters/mercs/homunculi use different formulas.) Hard (equipment, "right", stored as `def` / `mdef`):

* Hard DEF = sum of `def` of every equipped item (`status.cpp:3917`) + `(refine_def_bonus + 50) / 100` (rounded refine bonus from `refine.yml`, `status.cpp:4096`) + script `bonus bDef`; then `* def_rate/100` and `+20+20*lv` with Mado MainFrame. Pre-renewal `max_def` 99 over-DEF folding is `#ifndef RENEWAL`, so no cap here besides the type limits.
* Hard MDEF = script `bonus bMdef` sums (+1 for Endure), `* mdef_rate/100`. The renewal client shows MDEF possibly negative (`mdef2` is sent as-is in renewal, only pre-renewal clamps at 0).

Damage use of DEF/MDEF is in [combat.md](./combat.md).

### HIT, FLEE, perfect dodge, CRIT

`status_calc_misc` renewal player branch (`status.cpp:2620-2640, 2706-2730`), all added to existing equipment/script totals:

```
HIT   = bonus_hit  + level + DEX + LUK/3 + 175 + 2*CON         clamp 1..32767
FLEE  = bonus_flee + level + AGI + LUK/5 + 100 + 2*CON         clamp 1..32767
CRI   = bonus_cri  + level/10 + 10 + 3*LUK                     clamp 1..32767   (units: 1/10 crit)
FLEE2 = bonus_flee2 + LUK + 10                                 clamp 0..32767   (units: 1/10 perfect dodge)
```
The window shows `CRI/10` and `FLEE2/10` (integer division) (`clif.cpp:3672-3675, 4152-4153`). After `status_calc_misc`, `hit_rate`, `flee_rate`, `critical_rate`, `flee2_rate` percentages from items multiply the totals (`status.cpp:4415-4445`).

Trait-stat extras (4th classes): `PATK += POW/3 + CON/5`, `SMATK += SPL/3 + CON/5`, `RES += STA + STA/3*5`, `MRES += WIS + WIS/3*5`, `HPLUS += CRT`, `CRATE += CRT/3` (`status.cpp:2651-2677`).

### ASPD (renewal)

> Open findings: [PF-16](../../../PACKET_FINDINGS.md#pf-16).

Pipeline: `status_base_amotion_pc` (`status.cpp:2351-2400`) then the `SCB_ASPD` block of `status_calc_bl_main` (`status.cpp:6370-6420`). Constants (`status.hpp:44-67`): `AMOTION_ZERO_ASPD = 2000`, `AMOTION_INTERVAL = 10`, `AMOTION_DIVIDER_PC = 2`, `MIN_ASPD = 8000`.

```
base   = job.aspd_base[right weapon type]                    # job_stats: db/re/job_aspd.yml, default 200 for unlisted weapons
base  += job.aspd_base[SHIELD]            if a shield is equipped
base  += job.aspd_base[left weapon]/4     else if dual-wielding a different item
stat   = (ranged ? DEX*DEX/7.0 : DEX*DEX/5.0) + AGI*AGI*0.5       # ranged = same set as batk above, float math
stat   = sqrt(stat)*0.25 + 196
val    = skill adds (Advanced Book (lv-1)/2+1 with Book, Single Action (lv+1)/2 with guns, Star Devil
         (SG_DEVIL, Star Emperor or max job level) 1+lv, minus mount penalties: riding -(50-10*Cavalier Mastery), dragon -(25-5*lv))
P      = (int)( stat + (status_calc_aspd(fixed) + val) * AGI/200 )  -  min(base, 200)
```
`status_calc_aspd(fixed)` returns a flat bonus from buffs (`status.cpp:8237+`): 7 for the Two-Hand Quicken / One-Hand Quicken / Adrenaline Rush / Spear Quicken family, 6 for Adrenaline Rush (party), 5 for Fleet-style buffs, plus Assassin Cross of Sunset's `val2` (not exhaustive). Then:

```
amotion  = P * aspd_rate/1000                       # base aspd_rate = 1000 for players in renewal
amotion += max(195 - amotion, 2) * (aspd_rate2 + status_calc_aspd(percent)) / 100     # percent buffs (potions etc.)
amotion  = 2000 - amotion*10 ; amotion += bonus.aspd_add
amotion  = clamp(amotion, pc_maxaspd/2, MIN_ASPD/2)
adelay   = 2*amotion
```
`pc_maxaspd` is battle-config `max_aspd` (190), `max_third_aspd` 193, `max_extended_aspd` 193 (Kagerou/Oboro/Rebellion), `max_summoner_aspd` 193; converted at startup to `(2000 - cfg*10)*2` (`battle.cpp:8965`), so the floor is amotion 100 (ASPD 190) / 70 (193). The client shows ASPD `= 200 - amotion/10`.

Worked check (hand-computed): bare-fisted level-1 Novice, AGI=DEX=1: `stat = sqrt(0.2+0.5)*0.25+196 = 196.209`, `P = 196 - min(40,200) = 156`, `amotion = 2000-1560 = 440`, `adelay = 880`, displayed ASPD 156. Not captured for a Novice. A **bare Knight** with AGI = DEX = 1 shows 440 (Fist base 40 too). The captured Novice wore a Knife: 590, which needs a Novice Dagger base of 55. See the ASPD rows in [Captured status values](#captured-status-values).

### MaxHP / MaxSP

`status_calc_maxhp_pc` / `status_calc_maxsp_pc` (`status.cpp:3511-3600`), `lvl = max(base_level,1) - 1`:

```
dmax  = job.base_hp[lvl]                          # table from job_stats/job_basepoints (see job_stats.md)
if VIT > 0: dmax *= 1 + VIT*0.01                  # battle-status VIT (job bonus + equip + buffs included)
if upper class (JOBL_UPPER, i.e. Transcendent/High) or primary 4th: dmax *= 1.25
else if Taekwon ranker: dmax *= 3
dmax += equipment-only VIT bonus                  # items give +1 HP per VIT point on top of the %
dmax += bonus bMaxHP flat
dmax += dmax*equip_maxhp_pct/100 + dmax*item_maxhp_pct/100   # (two separate % pools)
dmax += (int64)(dmax * status_hpbonus_rate / 100)           # Aegis-style rounding
floor to uint, min 1
then: max_hp = hp_rate%*max_hp (hp_rate default 100) and capped by max_hp_lv99 (lv<100) 330000,
      max_hp_lv150 (lv<151) 660000, else max_hp 1100000 (player.conf:98-100)
```
MaxSP is identical with `base_sp`, INT, the equipment INT bonus, `sp_rate`, and cap `max_sp` 1000000. MaxAP (`base_ap`, no stat scaling) exists for 4th classes (~200 flat at lvl >= 200). When max shrinks below current HP/SP, current is clamped (and `SP_HP`/`SP_SP` pushed).

Base values when no table entry exists (`JobDatabase::calc_basehp/sp`, `pc.cpp:13710-13790`):

```
base_hp = 35 + floor(lvl_in * HpIncrease/100) + sum_{i=2..lvl_in} round(HpFactor/100 * i)       # round = floor(x+0.5)
base_sp = 10 + floor(lvl_in * SpIncrease/100) + sum_{i=2..lvl_in} round(SpFactor/100 * i)
```
(`lvl_in` is the 1-based level; Super Novice +2000 HP at 99 and +2000 at 150; Summoner x1.5 HP and SP; Ninja/Gunslinger SP adjustments.) In this renewal DB nearly all jobs have explicit `BaseHp`/`BaseSp` tables, so the formula applies only to levels a table omits (e.g. 3rd-class levels below 99). New character initial values (char server): `HP = 40*(100+VIT)/100`, `SP = 11*(100+INT)/100` (`char.cpp:1505-1507`).

### Max weight and overweight

`status_calc_weight` (`status.cpp:3652-3700`); weight unit is 0.1 (item weight 10 = 1.0):

```
max_weight = job.MaxWeight (default 20000, i.e. 2000.0) + status.str * 300      # allocated STR points only, NOT bonuses
CALCWT_MAXBONUS adds: bAddMaxWeight, Enlarge Weight Limit +2000*lv, riding with Cavalier Mastery learned +10000,
  dragon riding +5000+2000*Dragon Training lv, SC_KNOWLEDGE +max*val1/10, ALL_INCCARRY +2000*lv, Mado +15000
weight = sum(item.weight * amount) over inventory (CALCWT_ITEM)
```
Overweight (`pc_updateweightstatus`, `pc.cpp:3023-3053`): `pct = weight*100 / max_weight`; `>= 90` (`major_overweight_rate`) = SC_WEIGHT90 (cannot heal, attack or use skills); `>= 70` renewal (`natural_heal_weight_rate`, pre-renewal 50) = SC_WEIGHT50 (no natural HP/SP regen). Capture-verified example (novice STR 1): max 20300, weight 500 ([../packets/map_units.md](../packets/map_units.md#0x00b0-zc_par_change)).

### HP / SP natural regeneration

Base per-tick amounts (`status_calc_regen`, `status.cpp:5264-5300`), recalculated whenever VIT/INT/MaxHP/MaxSP change:

```
HP per tick = VIT/5 + max(1, MaxHP/200), then * hprecov_rate/100, min 0 for players, cap 32767
SP per tick = 1 + INT/6 + MaxSP/100 ; if INT >= 120: += (INT-120)/2 + 4 ; then * sprecov_rate/100
HP Meditatio: SP *= (100 + 3*lv)/100
```
Skill regen (separate accumulator, every `natural_heal_skill_interval` 10000 ms): Increase HP Recovery `lv*5 + lv*MaxHP/500` HP; Increase SP Recovery `lv*3 + lv*MaxSP/500` SP; while sitting Spirits Recovery etc. (`status.cpp:5295-5345`).

Timing (`status_natural_heal`, `status.cpp:15440-15640`; global timer every `NATURAL_HEAL_INTERVAL` = 500 ms, `map.hpp:75`):

```
interval_hp = natural_healhp_interval (6000) / (regen_rate_pct/100 * multi)
interval_sp = natural_healsp_interval (8000) / (regen_rate_pct/100 * multi)
multi = 1, +1 if sitting (dead_sit == 2)        # sitting halves the interval (doubles regen speed)
interval -= 250                                  # timer slack
fires when (last_tick + interval) <= now ; each fire heals one per-tick amount
```
Rules: no natural regen at all while dead or hidden/cloaked; none while overweight (>= 70%) except that sitting skill-regen still runs at half speed; while walking, natural HP regen is skipped unless the character has Moving HP Recovery (then the HP interval doubles), natural SP regen continues, and skill regen (the 10 s accumulators) is skipped. Poison, Bleeding, Berserk, Trick Dead and similar zero the regen flags. Heals go through `status_heal` to `pc_heal`, which pushes `SP_HP` / `SP_SP` (`0x00b0`) per tick. (Natural-regen AP behaviour for 4th classes was not analysed.)

### When recalculation happens and what is pushed

`status_calc_pc(sd, opt)` rebuilds base status and pushes diffs; triggers include login (`intif.cpp:3505`, `SCO_FIRST|SCO_FORCE`), level-up (`pc.cpp:8276`), job level-up (`:8346`), stat-up (`pc_statusup`, `:8904`), equip/unequip, skill learn, buff start/end and `bonus` scripts (many `pc.cpp` call sites), map change (`clif.cpp:11092`). Partial recalcs use `status_calc_bl(bl, {SCB_...})` flags (speed, ASPD, etc.). Push order and the per-field rule are in [../packets/status.md](../packets/status.md#recalculation-pushes).

## Stat point costs and limits

### Cost to raise one stat (renewal, `RENEWAL_STAT`)

`PC_STATUS_POINT_COST(low)` (`pc.cpp:8797-8802`) is the cost of raising a stat currently at value `low` by one point; a multi-point raise sums it per step (`pc_need_status_point`, `pc.cpp:8809`):

```
low <  100:  2 + (low - 1)/10
low >= 100: 16 + 4*((low - 100)/5)
```
Examples: 1->2 costs 2; 11->12 costs 3; 50->51 costs 6; 99->100 costs 11; 100->101 costs 16; 105->106 costs 20; 129->130 costs 36. Total from 1 to 99 per stat: 628 points; from 1 to 130: 1419. Cost reported as 0 when the stat is already at its max. Trait stats cost 1 point each (`pc_need_trait_point`, `pc.cpp:8964`). Pre-renewal formula `1 + (low+9)/10` is not used.

### Max stat per class

From `battle.cpp`/`player.conf:114-122, 310` and the `JobDatabase::loadingFinished` rules (`pc.cpp:14340-14410`); `MaxStats` in `job_stats.yml` is empty in this DB so these apply to every job:

| Class group | Max STR..LUK |
| :--- | ---: |
| Normal (1st/2nd), Novice, Super Novice, Gunslinger, Ninja, Taekwon etc. | 99 |
| Transcendent (High) 1st/2nd | 99 (`max_trans_parameter`) |
| Baby (non-3rd) | 80 |
| 3rd class | 130 |
| 3rd transcendent / primary 4th | 130 |
| Baby 3rd | 117 |
| Kagerou/Oboro, Rebellion (extended) | 130 |
| Summoner | 130 |
| 4th classes (also the "trait job" set) | 130; trait stats POW..CRT 110 (`max_trait_parameter`); all other classes' trait max = 0 |

The check order is: baby first, then Summoner, extended, trait/4th, 3rd (trans split), transcendent. The cap the client is told about is implicit: the cost field in `0x00bd`/`0x00be` is 0 when at max.

### Status points per level and reset

`db/re/statpoint.yml` holds cumulative totals (Level 1 = 48 ... Level 275 = 4099 points; trait totals start at level 201 with 3, +3 per level plus +4 extra every fifth level, 285 at 275). Points granted on level-up from L to L+1 are `table[L+1] - table[L]` (0 if not increasing, e.g. past 200): 3 per level for L=1..4, 4 for 5..9, then +1 per 5 levels up to L=95 (22), 23 from L=100, +1 per 10 levels up to 150 (28), then +1 per 7 levels to 35 at L=199, 0 from L=200. Details and the table-vs-formula option: [../db/exp_tables.md](../db/exp_tables.md#statpointyml).

* New characters: PACKETVER >= 20120307 clients send no stat distribution; the server creates every stat as 1 and `status_point = start_status_points` = 48 (`inter_athena.conf:16`, `char.cpp:1437-1461`, `char_clif.cpp:1270-1300`).
* Stat reset (`pc_resetstate`): `status_point = table_point(base_level)` (plus `transcendent_status_points` for Rebirthed/high classes, `pc.cpp:9358-9366`), all stats back to 1.
* `pc_statusup` gate: point total available, per-step cost, class max; request count capped to what is affordable ([../packets/status.md](../packets/status.md#stat-up-request-and-acknowledgement)).

## Refine bonuses (weapon ATK, weapon MATK, armor DEF)

Source: `rathena` `d4b8e7b8f`, `status_calc_pc_sub` equipment loop `src/map/status.cpp:3908-4096`, `battle.cpp:2402-2420,2443-2495`; data in [../db/refine.md](../db/refine.md). Applies to 20260219 (renewal). `R` = item refine (clamped to 20), `B(R)` = the cumulative `Bonus` for that item's group/level at refine R (1/100 units; 0 if R = 0, item not `Refineable`, or no entry).

* **Which items:** every worn item that is Weapon or Armor type, per `EQI` slot, same loop as `def`/scripts (duplicates of multi-slot gear skipped, ammo skipped). Armor type covers body armor, shield, garment, shoes, headgear and accessories alike, there is no location filter. Shadow gear and Ammo contribute nothing (shadow tables have no Bonus). Costume headgear/garments are normal Armor items but are not `Refineable` in item_db, so they give 0; no explicit costume check exists. There is **no broken-item check** in the pass: broken gear (`attribute == 1`) cannot be equipped (`pc.cpp:1890`). A worn item that later becomes broken is not re-checked by this pass.
* **Weapon ATK:** `wa.atk2 += B/100` (integer division), where `wa` is `rhw` or `lhw` (left only for a weapon in `EQI_HAND_L` with `equip == EQP_HAND_L`). Item `Attack` goes to `wa.atk`. Renewal grade bonus (only Weapon level 5, `enchantgrade > 0`): `wa.atk2 += (B/100) * gradeBonus / 100` (integer, grade percent from refine.md).
* **Weapon MATK (renewal):** `wa.matk += item.matk`, then `wa.matk += B/100` (and the same grade term `((B/100)*gradeBonus)/100`) **unless `sd->weapontype1 == W_BOW`** (the right-hand type; bows get ATK refine but no MATK refine). Then `weapon_matk_rate` (`bonus`) scales `wa.matk` by `1 + rate/100` over the item + refine MATK.
* **Overrefine:** `weapon_data.overrefine = RandomBonus(R) / 100` (the table's maximum, set whenever a refine entry exists, even if 0). Damage: `rnd()%overrefine + 1` is added to each weapon hit (1..overrefine) in `battle_add_weapon_damage` (`battle.cpp:2402-2420`), after the renewal variance roll; for MATK `matk_min += 1`, `matk_max += overrefine - 1` (see the MATK block above). The variance term uses only `wa.atk` (item ATK) and `wlv`, not `atk2` (`variance = 5*atk*wlv/100`); `atk2` is part of `watk` that variance is applied around (details: [combat.md](./combat.md)).
* **Weapon level 5 only (renewal):** `patk += R*2` and `smatk += R*2` (P.ATK / S.MATK, trait stats), once per worn level-5 weapon with a refine entry. Not yet modelled.
* **Armor DEF:** per armor item `refinedef += B(R)` (raw, 1/100 units), accumulated across all worn armor. After the loop and the combos, **one** rounding: `def += (refinedef + 50) / 100` (integer division, round half up on the total, not per item; `status.cpp:4096`). The item's own `def` was already added per item (`status.cpp:3917`).
* **Armor level 2 only (renewal):** `res += R*2` and `mres += R*2` (R/MR trait stats), per worn level-2 armor with a refine entry. Not yet modelled.
* **Recalc trigger:** refine and equip are part of the full `status_calc_pc`; the refine level is also clamped there if it exceeds `MAX_REFINE`.

### Check against the captures

| Capture ([stats section above](#derived-stat-formulas)) | Table | Result |
| :--- | :--- | :--- |
| Knife +10, `SP_ATK2` 17 -> 37 | Weapon level 1, B(10) = 2000 | +20 ATK (`2000/100`); 17+20 = 37. Matches. RandomBonus(10) = 900 -> overrefine 9 (not in `SP_ATK2`). |
| Rod +10, `SP_MATK1` 30 -> 50 | Weapon level 1, B(10) = 2000, not a bow | +20 MATK; 30+20 = 50. Matches. (Knife +10 -> `SP_MATK1` 20, same +20.) |
| Hat +10 and Silk Robe +10, `SP_DEF2` +18 each | Armor level 1, B(10) = 1800 | `(1800+50)/100 = 18`. Matches for each item worn alone. Rounding is on the summed raw bonus, so it differs from per-item rounding only for odd sums: three level-2 armors at +1 (120 each) give `(360+50)/100 = 4`, not `1+1+1`. |

## Captured status values

Source: [captures/attribute_sequence.txt](../captures/attribute_sequence.txt) (file line numbers). Packet order: [../packets/status.md](../packets/status.md#captured-stat-up-level-and-job-sequences). Field mapping as in [`SP_*` var ids](../packets/status.md#sp_-var-ids): soft DEF = `SP_DEF1`, hard DEF = `SP_DEF2`, soft MDEF = `SP_MDEF1`, hard MDEF = `SP_MDEF2`, status MATK = `SP_MATK2`, weapon MATK = `SP_MATK1`, status ATK = `SP_ATK1`, weapon ATK = `SP_ATK2`. "Battle" stats include the job bonus (the `0x0141` bonus). Every value in the tables below matches the formulas in this file by hand; the job-table inputs (HP/SP tables, ASPD bases, `MaxWeight`) come from `db/re` and are checked by tests once imported.

**Characters.**

* `testtesttest` (session `#5`, `#7`), male.
  * At login: Novice, base 1, job 1, all stats 1, 48 points.
  * Wears a Knife (1201, weapon level 1, ATK 17, refine 0) and a Cotton Shirt (2301, DEF 10).
* `test` (session `#3`): Bard (19), base 99, job 1, allocated STR 11, AGI 10, VIT 1, INT 1, DEX 10, LUK 1 (DEX +1 job bonus), 1217 points. Weapon view 1907.

| Lines | State | amotion | ATK1 / ATK2 | MATK2 / MATK1 | DEF1 / DEF2 | MDEF1 / MDEF2 | HIT | FLEE1 / FLEE2 | CRIT | MaxHP / MaxSP | MaxWeight |
| :--- | :--- | ---: | :--- | :--- | :--- | :--- | ---: | :--- | ---: | :--- | ---: |
| 294-321, 352 | Novice B1 J1, all 1, Knife + Cotton Shirt | 590 | 1 / 17 | 1 / 0 | 1 / 10 | 1 / 0 | 177 | 102 / 1 | 1 | 40 / 11 | 20300 |
| 402-416 | same at B11 J1 | 590 | 4 / 17 | 3 / 0 | 6 / 10 | 4 / 0 | 187 | 112 / 1 | 1 | 90 / 21 | 20300 |
| 435-450 | B11 J10 (job bonus +1 to all six) | 590 | 5 / 17 | 5 / 0 | 6 / 10 | 5 / 0 | 188 | 113 / 1 | 1 | 91 / 21 | 20300 |
| 681-695 | B6 J10, allocated STR 3, AGI 4, INT 12 (others 1) | 590 | 6 / 17 | 20 / 0 | 5 / 10 | 15 / 0 | 183 | 111 / 1 | 1 | 66 / 18 | 20900 |
| 45-72, 110 | Bard B99 J1 (above) | 470 | 38 / 142 | 27 / 0 | 52 / 60 | 28 / 0 | 285 | 209 / 1 | 2 | 4096 / 630 | 30300 |
| 1729-1747 | Swordman B37 J1, allocated STR 13, AGI 21, VIT 17, INT 19, DEX 29, LUK 16, Knife + Cotton Shirt | 470 | 33 / 17 | 47 / 0 | 31 / 10 (not pushed) | 37 / 0 | 246 | 161 / 2 | 6 | 834 / 99 | 31900 |
| 4576-4602, 4632 | Knight B99 J1, allocated STR 99, AGI 99, VIT 1 (+1), INT 1, DEX 9, LUK 1, bare-handed, Cotton Shirt | 270 (unbuffed; a late push at line 4669 shows 230 with the recorder's Berserk Potion active, see [capture 2](#capture-2-values)) | 125 / 0 | 26 / 0 | 70 / 10 | 27 / 0 | 283 | 298 / 1 | 2 | 8137 / 310 | 57700 |

**ASPD steps** (Novice B11-B26, job 10 bonus +1, Knife; values are battle AGI/DEX). Each matches `P = (int)(196 + sqrt(DEX²/5 + AGI²/2)/4) − 55`:

* AGI 6, DEX 2 → 580 (line 857).
* AGI 12 → 570 (909).
* AGI 17 → 560 (952).
* AGI 22, DEX 9 → 550 (1319).
* DEX 29 → 540 (1528).

**ASPD with shields and two-handers** (Swordman B48 J11, then Knight and others):

* Swordman with Blade (1107): 460. Adding a Guard (2101) gives 510. The Guard alone gives 440, bare hands 390 (lines 1953-2000, 2044-2097). So a shield adds the job's `Shield` base × 10 to amotion (Swordman 5 → +50), as the formula says.
* Lord Knight with Blade + Guard: 480 (2610). Rune Knight: 610 (2656).
* Knight B99, AGI = DEX = 1:
  * bare: 440 (4485);
  * wearing a katar, which a Knight can't use: **1250** (4409). The "unlisted weapon = 200" rule predicts 2040, so check the imported `job_aspd` rows before trusting the default (open question below).

**Refine** (Swordman, lines 1935-1999, 2063-2097):

* Blade (1107) +0 → `SP_ATK2` 53. Blade +6 → 65 (+12, 2 per refine level, as for the weapon-level-1 Knife) and `SP_MATK1` 12 (the same +12 as MATK).
* Guard (2101) +0 adds hard DEF 20 (10 → 30); Guard +6 gives 38, so the refine adds 8.

**4th-class MaxHP/MaxSP** (Shadow Cross 4254, base 170; lines 3753-3754, 3789). The fallback base formula above with Shadow Cross's `HpFactor` 64, `HpIncrease` 5398, `SpFactor` 7, `SpIncrease` 4 gives base HP 18513 and base SP 1034. Then:

| VIT / INT | MaxHP | MaxSP | Check |
| :--- | ---: | ---: | :--- |
| 4 / 3 (job 1) | 24066 | 1331 | `18513 × 1.04 × 1.25`, `1034 × 1.03 × 1.25` |
| 10 / 8 (job 60) | 25455 | 1395 | `× 1.10 × 1.25`, `× 1.08 × 1.25` |

All four are exact. This confirms the fallback formula and the 4th-class ×1.25. Truncating after each factor or once at the end gives the same four numbers.

**Stat costs** (`0x00be`) match the renewal formula: 2 for stats 1-10, 3 for 11-20, 4 for 21-30, 5 from 31. INT 99 shows 0 (the cap for a Bard).

**Status points** (`SP_STATUSPOINT`) match `statpoint.yml` deltas:

* Novice 48 → 85 at B11, 67 at B26 after spending.
* Bard B99 with 56 points spent shows 1217 (1273 − 56).
* The Knight reset at B99 gives 1273.

## Capture 2 values

Source: [captures/attribute_sequence_2.txt](../captures/attribute_sequence_2.txt) (rAthena `d4b8e7b`, `PACKETVER 20260219`; file lines). Packet orders: [../packets/status.md](../packets/status.md#capture-2-observer-stat-reset-failure-and-4th-class-jobs). Values are the pushed ones (the `0x00bd` window is sent only at login).

| Lines | State | MaxHP / MaxSP | amotion | Check |
| :--- | :--- | :--- | ---: | :--- |
| 68-69, 136 | Knight B99, STR 99, AGI 99, VIT 1 (+1), DEX 9 | 8137 / 310 | 270, then 230 (late push, line 136: Berserk Potion buff) | `7978 × 1.02`, `307 × 1.01` |
| 508-519 | same, B96, all stats 1 after the reset | 7671 / 300 | 440 (499) | `7521 × 1.02`, `298 × 1.01` |
| 563 | Merchant B96, all 1 | 2400 / 300 | 440 | `2377 × 1.01`, `298 × 1.01` |
| 614-616 | Merchant B96 + Tsurugi (item 1119, from the `0x0b41` at line 599) | - | 560 | `SP_ATK2` push 130 |
| 1076-1077 | Merchant B96 after `@blevel -90`'s reset, then B6 at 1095-1096 | 2400 / 300, then 73 / 28 | 560 | B6: tables 73 and 28 |
| 1231-1232 | Novice High (4001) B6, all 1 | 82 / 20 | - | `65 × 1.01 × 1.25`, `16 × 1.01 × 1.25` |
| 1295-1296 | Super Novice (23) B6 | 65 / 16 | - | tables 65 and 16, ×1.01 |
| 1328-1329 | Super Novice B1 | 40 / 11 | - | `40 × 1.01`, `11 × 1.01` |
| 1387-1399 | Hyper Novice (4307) B1 J1, all allocated 1 | 99 / 11 | 420 | see below |
| 1501 | Hyper Novice with INT 18 | 99 / 12 | - | `10 × 1.28` |

Row checks use the `job_db.yml` tables (`BaseHp`, `BaseSp`) and `status_calc_maxhp_pc` (`status.cpp:3508-3556`): `table[level-1] × (1 + battle_vit/100)`, then ×1.25 for `JOBL_UPPER` or a primary 4th class, truncated once. Every pushed MaxHP/MaxSP value in the table reproduces exactly.

**Hyper Novice (4307).** Three things apply, none of which the earlier notes covered together:

* **No ×1.25.** `pc_is_primary_fourth` (`pc.hpp:1740-1742`) covers Dragon Knight..Shadow Cross and Imperial Guard..Abyss Chaser only. Hyper Novice has no `JOBL_UPPER` flag either. Captured: MaxHP 99 = `floor(90 × 1.11)`, not 124.
* **All stats +10** from the Super Novice rule (`status.cpp:4268-4276`): `class_ & MAPID_SECONDMASK == MAPID_SUPER_NOVICE`, and `job_level >= 70` **or `JOBL_THIRD`** (Hyper Novice's mapid is `JOBL_FOURTH | MAPID_SUPER_NOVICE_E`, `map.hpp:252`), and `die_counter == 0`. Captured at job level 1: `0x0141` STR and AGI bonus 11 (10 + the `BonusStats` level-1 STR+1, AGI+1), VIT/INT/DEX/LUK bonus 10 (lines 1381-1386). Plain Super Novice at job level 1 gets only STR+1 (line 1293). The +10 also feeds VIT/INT in the MaxHP/MaxSP product above.
* **Super Novice HP adjustment applies.** `JobDatabase::calc_basehp` (`pc.cpp:13706-13745`) adds +2000 at base level >= 99 and +2000 at >= 150 when `(mapid & MAPID_SECONDMASK) == MAPID_SUPER_NOVICE`, which includes Hyper Novice, Super Novice E and Super Baby. The adjustment sits in the **fallback** calc, which fills every level whose table entry is 0 (`pc.cpp:14288-14296`); Hyper Novice has no explicit `BaseHp`/`BaseSp`, so it takes the fallback with `HpFactor` 38, `HpIncrease` 5584, `SpFactor` 6, `SpIncrease` 40 (`job_stats.yml:7522-7530`). The capture covers only B1 (90 and 10, which match both the fallback and `job_db.yml`). **Gap found by reading, not by capture (resolved: data regenerated, confirmed by [capture 3](#capture-3-hyper-novice-values)):** `crates/ro-db/data/job/job_db.yml` Hyper_Novice `BaseHp` has no +2000/+4000 (level 99 gives 7445, rAthena gives 9445; level 150 gives 12716, rAthena gives 16716; SN-line rows 23 and Super_Novice_E do have them: 2530 at 99, 4785 at 150 for E). `BaseSp` matches the fallback exactly. SP has no such adjustment.

**Stat points.** `statpoint.yml` totals again: B99 1273, B96 1207, B6 64, B1 48. Trans grant +52 (Novice High, line 1214); Hyper Novice grants none.

## Capture 3 Hyper Novice values

Source: [captures/attribute_sequence_3.txt](../captures/attribute_sequence_3.txt) (file lines; applies to 20260219). Packet orders: [../packets/status.md](../packets/status.md#capture-3-elvmax-unequip-hyper-novice-jlvl-observer). Hyper Novice (4307) with no equipment bonus. `0x0141` VIT is type 15 and INT type 16 (not `0x31`/`0x32`, which are `SP_HIT`/`SP_FLEE1`). The stat sums below are base 1 plus the captured bonus.

| Lines | State | VIT bonus / INT bonus | MaxHP / MaxSP | Check (`job_db.yml` lines 13428-13429) |
| :--- | :--- | :--- | :--- | :--- |
| 667-668, 683-684 (and 796-797, 812-813) | B99 J1 | 10 / 10 (VIT 11, INT 11) | 10483 / 385 | `9445 × 1.11 = 10483.95`, `347 × 1.11 = 385.2` |
| 834-835 | B100 J1 | same | 10588 / 392 | `9539 × 1.11`, `354 × 1.11` |
| 868-869 | B101 J1 | same | 10691 / 399 | `9632 × 1.11`, `360 × 1.11` |
| 944-945, 960-961 | B99 J52 | 16 / 20 (VIT 17, INT 21) | 11050 / 419 | `9445 × 1.17 = 11050.65`, `347 × 1.21 = 419.9` |
| 1009-1010 | B150 J52 | same | 19557 / 908 | `16716 × 1.17 = 19557.7`, `751 × 1.21 = 908.7` |
| 1043-1045 | B201 J52 | same | 26879 / 1585, `SP_MAXAP` 200 | `22974 × 1.17 = 26879.6`, `1310 × 1.21 = 1585.1` |
| 532-533 (Swordman, for contrast) | B99 J1 | 0 / 0 | 4038 / 210 | `3999 × 1.01`, `208 × 1.01` |

Findings:

* **Every pushed MaxHP/MaxSP matches** the formula in [MaxHP / MaxSP](#maxhp--maxsp): table value × (1 + stat/100), truncated once, **no ×1.25** for Hyper Novice at any level (1 to 201), and the Super Novice +2000 (B99) / +4000 (B150) HP is in the table value (9445 and 16716).
* **The generated data is now correct.** The gap noted under [Capture 2 values](#capture-2-values) (7445 / 12716) no longer exists: `crates/ro-db/data/job/job_db.yml` Hyper_Novice `BaseHp` has 9445 at level 99, 9539 at 100, 9632 at 101, 16716 at 150 and 22974 at 201. `BaseSp` has 347, 354, 360, 751 and 1310 at the same levels, all matching the capture.
* **+10 all stats confirmed at J1 and J52.** J1 bonus: STR 11, AGI 11, VIT/INT/DEX/LUK 10 (lines 665-670). J52: STR 20, AGI 15, VIT 16, INT 20, DEX 15, LUK 16 (942-947), which is the `BonusStats` sums through level 52 (STR 10, AGI 5, VIT 6, INT 10, DEX 5, LUK 6) plus 10. Trait bonus at J52: POW 9, STA 4, WIS 4, SPL 8, CON 8, CRT 3 (962-967), exactly the `BonusStats` sums. `SP_PATK`..`SP_CRATE` there: 4, 3, 9, 9, 3, 1 (968-973).
* **Trait points:** `@job 4307` grants 7 (lines 643, 778), shown by every level tail (`SP_TRAITPOINT` 7 at B100, B101, B150) and 10 at B201 (the first `TraitPoints` entry in `db/re/statpoint.yml` is 3, at base 201). `@job 1` away clears them to 0 (709).
* **Status points** in the tails: 1295 (B100), 1318 (B101), 2545 (B150), 4099 (B201), all `statpoint.yml` totals; no 4th-class bonus.
* **ASPD** (amotion) 570 at J1, 560 at J52 (AGI 12 to 16). Hyper Novice at B99 was not captured with a weapon.
* **Generated data:** nothing here contradicts `job_db.yml`. Stale text: the "Gap found by reading" sentence in the Hyper Novice bullet under Capture 2 is resolved by the above.

## Open questions

* Knight + katar amotion 1250 ([Captured status values](#captured-status-values)): find which `job_aspd` entry or default produces it once `tools/job-import` has run. If the Knight row has no Katar, the default is not 200.
* Hard DEF/MDEF post-renewal clamps (`DEFTYPE_MIN/MAX`) were not located in the headers read.
* 4th-class AP regeneration and the `status_heal(..., 1)` third parameter in the natural-heal loop were not analysed.
* Equipment ATK (weapon ATK/refine/overrefine) and status effects layered by `status_calc_bl_main` (buff percentages) are out of scope here; they belong in combat.md / status.md.
