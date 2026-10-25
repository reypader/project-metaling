# Base and Derived Stat Formulas

Target client version: `PACKETVER = 20260219`, renewal build (`RENEWAL`, `RENEWAL_ASPD`, `RENEWAL_STAT` all defined in `src/config/renewal.hpp:55-74`). Provenance for every section: `rathena` at `$RATHENA_PATH`, commit `d4b8e7b8f` (2026-10-06), paths `src/map/status.cpp`, `pc.cpp`, `pc.hpp`, `conf/battle/*.conf`, `db/re/*.yml`. No proxy capture of the status window exists in the repo, so nothing here is capture-verified. Wire packets: [../packets/status.md](../packets/status.md). Static job data (HP/SP tables, weights, ASPD bases, job bonuses): [../db/job_stats.md](../db/job_stats.md). Exp tables, level-up flow: [experience.md](./experience.md), [../db/exp_tables.md](../db/exp_tables.md).

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

Worked check (hand-computed, not captured): bare-fisted level-1 Novice, AGI=DEX=1: `stat = sqrt(0.2+0.5)*0.25+196 = 196.209`, `P = 196 - min(40,200) = 156`, `amotion = 2000-1560 = 440`, `adelay = 880`, displayed ASPD 156.

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

## Open questions

* Hard DEF/MDEF post-renewal clamps (`DEFTYPE_MIN/MAX`) were not located in the headers read.
* 4th-class AP regeneration and the `status_heal(..., 1)` third parameter in the natural-heal loop were not analysed.
* Equipment ATK (weapon ATK/refine/overrefine) and status effects layered by `status_calc_bl_main` (buff percentages) are out of scope here; they belong in combat.md / status.md.
