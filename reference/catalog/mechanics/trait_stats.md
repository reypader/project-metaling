# Trait Stats (POW STA WIS SPL CON CRT) and AP

Source: rAthena `$RATHENA_PATH` @ `d4b8e7b8f`. Applies to `PACKETVER = 20260219` (renewal; all trait code is `#ifdef RENEWAL` / `PACKETVER_MAIN_NUM >= 20200916`). Linked from [stats.md](./stats.md#trait-stats-and-ap). Wire side: [../packets/status.md](../packets/status.md#trait-stat-up-0x0b24). Sections below were written from source; **[Captured] marks statements confirmed by [rathena_traits_sequence.txt](../captures/rathena_traits_sequence.txt)**, with the values in section 7.

## 1. Effects of the trait stats

Trait value = `BonusStats` job sum at job level + `char.<trait>` + item/equip `param_bonus`/`param_equip` (script `bPow`..`bCrt`, `status.cpp:4260-4265, 4303-4314`), clamped 0..65535; then SC adds `SC_BENEDICTUM`/`SC_MARINE_FESTIVAL` `val2` to POW and CRT (`status_calc_pow/_crt`, `status.cpp:7175-7285`). Only the 6 uses below exist in `status.cpp`; none feed MaxHP/MaxSP, ASPD, CRIT, DEF or MDEF.

| Derived | Trait term | Where |
| :--- | :--- | :--- |
| `batk` (status ATK, PC) | `+ 5*POW` added inside `(dstr*10 + dex*10/5 + luk*10/3 + lv*10/4)/10 + 5*POW` | `status.cpp:2477` |
| HIT | `+ 2*CON` (also in the SCB_HIT delta: `+2*(con_final - con_base)`) | `2642`, `6014` |
| FLEE | `+ 2*CON` (delta `6029`) | `2647` |
| PATK | `+ POW/3 + CON/5` | `2665-2668` |
| SMATK | `+ SPL/3 + CON/5` | `2670-2673` |
| RES | `+ STA + (STA/3)*5` | `2675-2677` |
| MRES | `+ WIS + (WIS/3)*5` | `2679-2681` |
| HPLUS | `+ CRT` | `2683` |
| CRATE | `+ CRT/3` | `2687` |

All six clamp 0..32767 (SHRT_MAX) and use integer division. The SCB_PATK/SMATK/RES/MRES/HPLUS/CRATE blocks (`6450-6490`) re-apply the term as a delta (`final - base`) when an SC changed the trait. Consumers of the derived values: PATK multiplies the final weapon damage `dmg*(100+patk)/100` (`battle.cpp:5532`, skipped only for Dragon Breath without Dragonic Aura); CRATE crit damage `1.4 + 0.01*crate` for players (`battle.cpp:5648`); SMATK `dmg += dmg*smatk/100` before the skill ratio (`battle.cpp:6016`); RES / MRES reduce physical/magic damage `x * res/(res+400) * 0.8` after item "ignore" capped by `max_res_mres_ignored` (`battle.cpp:5582-5609, 6044-6065`; detail not extracted); HPLUS multiplies healing `hp += hp*hplus/100` (`skill.cpp:771`). 4th-job skill ratios use POW/SPL/CON/CRT/STA directly (e.g. `skills/acolyte/*.cpp`, `skills/archer/*.cpp`, Barrier HP `+15*STA` `status.cpp:12826`, Dragon Training `+POW/5` `battle.cpp:4086`); out of scope here.

Non-4th classes: base `char` values are always 0 and the job `max_param` for POW..CRT is 0 unless the job is a trait job (`pc.cpp:14396-14408`), so `pc_traitstatusup` always fails for them. They can still have non-zero traits only through item/card `bPow..bCrt` bonuses or the two SCs above. `BonusStats` with trait keys exist in `db/re/job_stats.yml` only for: Abyss_Chaser, Arch_Mage, Biolo, Cardinal, Dragon_Knight(2), Elemental_Master, Hyper_Novice, Imperial_Guard(2), Inquisitor, Meister(2), Night_Watch, Shadow_Cross, Shinkiro, Shiranui, Sky_Emperor(2), Soul_Ascetic, Spirit_Handler, Troubadour, Trouvere, Windhawk(2). Job bonus is added by `base_status->pow += bonus[PARAM_POW]` indexed `[job_level-1]` (`status.cpp:4245-4265`). The Super Novice +10 bonus covers only the six primary stats.

Trait job set (`pc_is_trait_job`, `pc.hpp:1760`) = primary 4th (`MAPID_DRAGON_KNIGHT..MAPID_SHADOW_CROSS`, `MAPID_IMPERIAL_GUARD..MAPID_ABYSS_CHASER`) or "upper expanded second": Hyper Novice, Sky Emperor, Soul Ascetic, Night Watch, Shinkiro/Shiranui, Spirit Handler (`pc.hpp:1740-1754`).

Caps: `max_param[POW..CRT] = battle_config.max_trait_parameter` for trait jobs, else 0 (`pc.cpp:14396-14408`); `conf/battle/player.conf:310` = 110 (code default 100, `battle.cpp:8851`). A `MaxStats` entry in the job db overrides.

## 2. MaxAP

```
base_ap[L]  (L = max(base_level,1) - 1 index)   from job_basepoints.yml BaseAp (only parsed under HP_SP_TABLES, defined at config/core.hpp:49), else calc_baseap(level):
    calc_baseap(lv) = (lv >= 200 ? 200 : 0) + floor(lv * ApIncrease/100)
                      + sum_{i=2..lv} floor(ApFactor/100 * i + 0.5)
    (a BaseAp entry of 0 also falls back to the formula; ApFactor/ApIncrease default 0)   pc.cpp:13791-13807, 14296-14308
    The fallback is NOT job-guarded: every job in job_db gets it, for levels 1..MaxBaseLevel (the job argument is only used for ApFactor/ApIncrease; an unused mapid local exists).
    So a non-trait job (e.g. Rune Knight, no BaseAp, ApFactor=ApIncrease=0) has base_ap 0 below lv 200 and 200 at lv >= 200.
    job_basepoints.yml:19264-19290 BaseAp group (Level 200..275, each Ap 200, to line ~19442) lists 4th/trait jobs only: Dragon_Knight(2), Arch_Mage, Windhawk(2), Cardinal, Meister(2), Shadow_Cross, Imperial_Guard(2), Elemental_Master, Troubadour, Trouvere, Inquisitor, Biolo, Abyss_Chaser, Hyper_Novice, Sky_Emperor(2), Soul_Ascetic, Night_Watch, Shinkiro, Shiranui, Spirit_Handler -- i.e. the same values the formula yields.
    status_calc_maxap_pc (status.cpp:3611) has no trait-job guard (only job==null -> 1), and the callers at status.cpp:4374 / 6494 are unguarded: non-trait jobs can have max_ap 200 at base lv >= 200.
max_ap = base_ap + bMaxAP(fix)
max_ap += max_ap*bMaxAPrate_equip/100 + max_ap*bMaxAPrate_item/100     (both computed from the same pre-bonus value)
max_ap += int64(max_ap * rate_bonus/100);  floor at 0                  status.cpp:3611-3650
then: if ap_rate != 100: max_ap = (uint32)(ap_rate * (max_ap/100.));  max_ap = clamp(max_ap, 0, battle max_ap)    status.cpp:4374-4379, 6494-6499
```
`job` missing -> 1. `max_ap` battle config: `conf/battle/player.conf:318` = 1000 (code default 200, min 100, `battle.cpp:8853`); `ap_rate` 100; `restart_ap_rate` 0; `loose_ap_on_death` yes, `loose_ap_on_map` yes (VS maps), `keep_ap_on_logout` yes (`player.conf:321-337`). The table values are in [../db/job_stats.md](../db/job_stats.md). Non-trait jobs have max AP from the same path (zero if their table is empty).

Also written to `char.max_ap` (`sd->status.max_ap`, `status.cpp:4374`).

### Current AP behaviour

* **Login:** `status_calc_pc_sub` with `SCO_FIRST`: `battle_status.ap = status.ap` only if `keep_ap_on_logout == 1` (`status.cpp:3771-3777`); otherwise it is not loaded. The `ap` column is persisted (section 5). The initial `clif_initialstatus` push sends `SP_TRAITPOINT, SP_AP, SP_MAXAP, SP_UPOW..SP_UCRT` after the six trait vars and derived vars (`clif.cpp:4165-4195`), in that order. **[Captured]** (login block after `0x00bd`: `TRAITPOINT`, `AP`, `MAXAP`, then the six `SP_U<trait>`; `AP` is sent even at 0).
* **Save:** `pc_makesavestatus` copies `status.ap = battle_status.ap` (`pc.cpp:1378, 1396`); the `pc_setrestartvalue` non-resurrection branch (save-as-if-respawned) uses `max(battle ap, base ap)` (`pc.cpp:1154`).
* **Level-up:** `pc_checkbaselevelup` calls `status_percent_heal(sd,100,100)` = HP and SP only, no AP (`pc.cpp:8277`); same in `@blvl` (`atcommand.cpp:1691`). MaxAP follows level via `status_calc_pc(SCO_FORCE)` and is pushed by the diff if it changed. **[Captured]** MAXAP 200 pushed at B200, `SP_AP` never pushed by level changes.
* **Death / VS map:** `status_percent_damage(.., 0, 0, 100, 0)` = AP to 0 (`pc.cpp:9885, 7011`).
* **Job change / stat reset:** no explicit AP code; `pc_resetstate` does not touch or push AP (only `status_calc_pc`, so MaxAP is pushed if it changed).
* **Clamp when max shrinks:** in SCB_MAXAP, `if ap > max_ap: ap = max_ap` and `clif_updatestatus(SP_AP)` is sent immediately, before the diff (`status.cpp:6501-6506`). Script `set MaxAp` clamps the same way (`pc.cpp:10518-10527`).
* **Diff push** (renewal only, `status.cpp:6664-6692`): `SP_MAXAP` if `max_ap` changed, then `SP_AP` if `ap` changed, last in the fixed order (after CRATE).
* Wire: `SP_AP` / `SP_MAXAP` / `SP_TRAITPOINT` go as `clif_par_change` (`0x00b0`), trait values as `clif_couplestatus` (`0x0141`), `SP_U<trait>` as `0x00be` with `pc_need_trait_point(.., 1)` (`clif.cpp:3804-3870`).

## 3. Trait points

* Level-up L -> L+1: `trait_point += table_trait(L+1) - table_trait(L)` if positive, computed with the pre-increment level (`pc.cpp:8259, 8787-8797`); `SP_TRAITPOINT` is pushed right after `SP_STATUSPOINT` (`pc.cpp:8268`). Table values: [../db/exp_tables.md](../db/exp_tables.md). **[Captured]** tails send 7 (B200), +3 (B201), 19 (B205).
* `pc_resetstate` (`pc.cpp:9347-9420`): level capped to the class max (pushes `SP_BASELEVEL` if capped); `status_point = table(base_level)` (+ `transcendent_status_points` for upper or primary 4th); `trait_point = trait_table(base_level)`, **plus** `trait_points_job_change` (7) if `pc_is_trait_job(class)`. Stats set: 1,1,1,1,1,1, POW..CRT = 0. **[Captured]** `trait_point` = table + 7 (26 at B205, 7 at B195); the `0x0141`s carry base 1/0 and a stale `bonus` = old battle value - new base; the class used for `pc_is_trait_job` is the one in effect when the reset runs (see below). **[Captured]** with these pushes, in order: `SP_STR..SP_LUK`, `SP_POW..SP_CRT`, `SP_USTR..SP_ULUK`, `SP_UPOW..SP_UCRT`, `SP_STATUSPOINT`, `SP_TRAITPOINT`; then clears TK mission, then `status_calc_pc(SCO_NONE)` (so the derived diff comes after). No AP push.
* `@blvl -n`: reset when `status_point < remove || trait_point < remove_trait`, then each pool subtracted and floored at 0 independently (`atcommand.cpp:1706-1720`); already in [atcommands.md](./atcommands.md#baselevelup--joblevelup). Script `set BaseLevel` up-adds both pools (`pc.cpp:10419-10424`).
* `pc_resetlvl(type 1)` (`/resetlvl`-style GM/script path): trait_point = 0, traits = 0 (`pc.cpp:9237-9265`).
* Job change (`pc_jobchange`, `pc.cpp:10915-10975`): to a trait job from a non-trait job: `trait_point += trait_points_job_change` (7), pushes `SP_TRAITPOINT, SP_UPOW, SP_USTA, SP_UWIS, SP_USPL, SP_UCON, SP_UCRT`; from a trait job to a non-trait one: if `trait_point < 7` then `pc_resetstate` (forced reset) else `trait_point = 0` + the same 7 pushes. Trait job to trait job: nothing. (If base level exceeds the new class max, `pc_resetstate` runs first, `10922-10931`.) **[Captured, forced path]:** with the class already switched, that reset leaves `trait_point` 0 (non-trait), which is < 7, so a second `pc_resetstate` always follows; the >= 7 branch was not captured. +52 into / -52 out of a primary 4th class confirmed (1273 <-> 1325). Trait job to trait job: not captured. Note: leaving a trait job with enough unspent points does NOT zero the allocated trait stats, only the pool; the stats then stay (and max becomes 0).

## 4. `pc_traitstatusup` (`pc.cpp:9034-9085`)

Validation order, each failure sends only `0x00bc` ack `ok=0` (value 0) and returns:
1. `type` not in `SP_POW..SP_CRT` (219..224) or `increase <= 0`.
2. `max_increase = pc_maxtraitparameterincrease`: loop that adds 1 per point while `trait_points >= 0` and `value <= job max`; `increase = clamp(increase, 0, max_increase)`. Fail if `increase <= 0` or `current + increase > job max` (non-trait job: max 0, always fails).
3. `needed = pc_need_trait_point(increase)` = `increase` (1 per point; 0 if already at max); fail if `needed > trait_point`.

Success: `stat += increase` (so the client may send amount > 1; it is capped to affordable, not rejected; amount is `.W`, negative amount is dropped with a debug log and no ack, `clif.cpp:12738-12751`), `trait_point -= needed`, `status_calc_pc(SCO_NONE)` (diff pushes incl. the stat's own `0x0141` and affected derived values, first), then `0x00be` `SP_UPOW + (type-SP_POW)` **[Captured: whole success path, 39 requests, amount always 1; failure and amount > 1 not captured]**, (cost of the NEXT point, from `pc_need_trait_point(.,1)`: 1, or 0 at max), `SP_TRAITPOINT` (`0x00b0`), ack `0x00bc` `ok=1`, `value = min(new, 255)`, then `0x0141` again if new value > 255. Same shape as `pc_statusup` except `SP_STATUSPOINT` -> `SP_TRAITPOINT` and `SP_U<STR..>` -> `SP_U<trait>`; the ack uses the same `0x00bc` with `sp` = 219..224. Unlike `pc_statusup`, no achievement hook. The `0x0b24` handler exists only when `PACKETVER_MAIN_NUM >= 20200916` (true at 20260219). `clif.cpp:12742` header comment says SP_POW~SP_CON but the code accepts CRT.

## 5. Persistence

`char` table columns (`sql-files/main.sql:226-240`): `pow sta wis spl con crt` (smallint unsigned, default 0), `max_ap`, `ap` (int unsigned, 0), `trait_point` (int unsigned, 0). Loaded/saved with the rest in `char.cpp:931-989, 1049-1119, 315-329`; `mmo_charstatus` fields `pow..crt`, `ap`, `max_ap`, `trait_point` (`mmo.hpp:574-575`). The save UPDATE fires when any of `max_ap, ap, trait_point, pow..crt` differ from the cached copy (`char.cpp:300-302`). `new char` rows get the column defaults (0) and `max_ap`/`ap` are recomputed on first map-server calc.

## 6. Stat reset commands (stat side)

`@resetstat` with alias `@streset` (`conf/atcommands.yml:1220-1222`, `ACMD_FUNC(resetstat)` `atcommand.cpp:9648-9656`): `pc_resetstate(sd)` then message id **207** (name substituted). Also reachable as `/resetstate` (doc). `@reset` (`atcommand.cpp:7666-7673`) = `pc_resetstate` then `pc_resetskill(sd,1)`, message id **208**; `@resetskill` / `@skreset` is the skill-only one (msg 206). Packet order for `@streset`: the `pc_resetstate` pushes listed in section 3, the `status_calc_pc` diff, then the message line. This matches the capture-order of the `@blvl` reset branch ([atcommands.md](./atcommands.md#baselevelup--joblevelup)).

## 7. Captured values ([rathena_traits_sequence.txt](../captures/rathena_traits_sequence.txt), 20260219, DK, file lines)

Source-only predictions above held; nothing contradicted. Formulas checked against pushed values (`HIT = lv + dex + 175 + 2*CON`, `FLEE = lv + agi + 100 + 2*CON`, `batk = (str*10 + dex*2 + luk*10/3 + lv*10/4)/10 + 5*POW`, all with the DK J1 bonus STR+1/POW+1):

| Check | Observed |
| :--- | :--- |
| batk | B200: 57; B205 with 4 POW: 73; B195 reset: 56; +5 per allocated POW (489, 495, 502) |
| HIT / FLEE | B200 376/301; +2/+2 per CON (516-541); B205 CON 4: 389/314; reset to CON 0: 381/306 at B205 then 371/296 at B195 |
| PATK | 0, 1 at total POW 3 (497), stays 1 at POW 4 (no push); CON 4 adds 0 (CON/5) |
| RES | STA 1..7 = 1, 2, 8, 9, 10, 16, 17 (STA + STA/3*5) |
| HPLUS / CRATE | HPLUS = CRT (1..17); CRATE 1, 2, 3, 4, 5 at CRT 3, 6, 9, 12, 15 |
| MaxAP | 0 at B99/B195 (Knight and DK), 200 at B200, B201, B205 (`BaseAp` 200 from level 200); `AP` 0 throughout |
| SMATK / MRES | [rathena_trait_sequence_2.txt](../captures/rathena_trait_sequence_2.txt): SMATK 1 at SPL 3; MRES 1, 2 at WIS 1, 2; with the J60 bonus SMATK 4 (SPL 10, CON 7) and MRES 10 (WIS 5) |
| MaxAP, non-trait job | [rathena_trait_sequence_2.txt](../captures/rathena_trait_sequence_2.txt): Rune Knight T has MaxAP 200 at B200 and 0 at B190 (no job guard) |
| Trait job → trait job | [rathena_trait_sequence_2.txt](../captures/rathena_trait_sequence_2.txt): DK ↔ Meister sends no point pushes, and the allocated traits carry over |
| Not exercised | trait cap (110), AP > 0 (`@heal` has no AP argument; `@healap` does it), `keep_ap_on_logout`, leaving a trait job with ≥ 7 unspent points (the level-cap reset always ran first) |

* Trait points: 201..205 totals 3, 6, 9, 12, 19 (`statpoint.yml`), so B201 pushes 3, B205 pushes 19 (+16). The pool pushed in every level tail is the running total; `SP_TRAITPOINT` is 7 at B200 (no trait points before 201).
* Status points: B200/B201/B205 = 4151 (4099 + 52 trans points); B195 = 3980 (3928 + 52). After `@blvl -10`: 4151 - (4099 - 3928) = 3980.
* `@blvl -n` reset trigger is the trait pool alone (`trait_point` 2 < `remove_trait` 19, status points sufficient); the reset runs before the level change, then the pool drops by the table delta (26 - 19 = 7).
* `pc_traitstatusup` ack `value` = allocated stat; `bonus` in the trait `0x0141` = job bonus (POW 1 at J1). Order within the diff: derived values (`ATK1`, `HIT`, `FLEE1`) before the trait's `0x0141`; trait-derived (`PATK`, `RES`, `HPLUS`, `CRATE`) after it.
* DK at B99 J1: MaxHP 11575, MaxSP 467 (after the Knight's 8137/310); ASPD 540; weight 45300.

## Clamp order, trait-job exit, getting AP > 0 (rAthena d4b8e7b8f, renewal, applies to 20260219)

* Recalc clamp push order (`status.cpp`): `status_calc_pc_` runs `status_calc_weight` first (4650): `SP_WEIGHT` (3694, only if weight changed) then `SP_MAXWEIGHT` (3696, only if max changed). Then `status_calc_bl_main`: HP clamp push `SP_HP` (6217), SP clamp `SP_SP` (6235), AP clamp `SP_AP` (6505), all three before any diff packet. Then the diff block in `status_calc_bl_` (6577-6691) runs in fixed order (STR..LUK, HIT, FLEE, ASPD, ..., `SP_MAXHP` 6656, `SP_MAXSP` 6658, `SP_HP` 6660, `SP_SP` 6662, POW..CRT, PATK, SMATK, RES, MRES, HPLUS, CRATE, `SP_MAXAP` 6689, `SP_AP` 6691). HP/SP/AP are compared against the pre-recalc values, so each clamped value is pushed a second time in the diff block (HP, SP, AP duplicates). First diff packet = first changed field in that order (a stat or derived value if any changed, else `SP_MAXHP`).
* Leaving a trait job (`pc.cpp` 10915-10975, `pc_jobchange`), order: [level-cap reset block, if base level > new max: `pc_resetstate`, STATUSPOINT, TRAITPOINT, BASELEVEL, BASEEXP, NEXTBASEEXP] -> transcendent +/-52 `SP_STATUSPOINT` (only when upper-class-ness changes; or `pc_resetstate` if points < 52) -> `SP_USTR`,`UAGI`,`UVIT`,`UINT`,`UDEX`,`ULUK` (the 6 `0x00be`) -> trait block -> `SP_JOBLEVEL`,`JOBEXP`,`NEXTJOBEXP`. Trait block when `trait_point >= trait_points_job_change` (7): `trait_point = 0`, push `SP_TRAITPOINT`, `SP_UPOW`, `SP_USTA`, `SP_UWIS`, `SP_USPL`, `SP_UCON`, `SP_UCRT` (7 pushes, 6 `0x00be`-class cost pushes after the pool). Allocated `pow..crt` are NOT zeroed there; the `SP_U<trait>` values are the cost computed with the new class's max (0), per the catalog. If `trait_point < 7`, the block instead calls `pc_resetstate` (no explicit pushes in the branch). Entering a trait job adds `trait_points_job_change` and pushes the same 7.
* AP > 0 for a capture: `@healap [<AP>]` (`atcommand.cpp` 1429-1454, `conf/atcommands.yml:549`; no arg = full heal, negative = remove, msgs 821-823) is the easiest, GM level 1 `healap` group entry permitting. It is clamped by `status_heal` to `max_ap - ap` (`status.cpp` 1858), so it only works when MaxAP > 0, i.e. at base level >= 200 (capture: MaxAP 200 at B200, 0 at B190). So: `@blvl` to 200, then `@healap`. Script: `healap <ap>{,<char_id>};` (`script.cpp` 6024, `doc/script_commands.txt:4593`), same clamp. `setparam`/`set Ap, n` (`SP_AP`, `pc.cpp` 10518) clamps to `[0, max_ap]` and pushes nothing; `set MaxAp, n` (`pc.cpp` 10521, clamp `[1, battle max_ap 1000]`) only edits `battle_status.max_ap`, which the next MAXAP recalc overwrites, and then the AP clamp push fires (useful to capture the AP clamp). `bonus bMaxAP,n` (`pc.cpp` 3922) adds fixed MaxAP at any level; no item in `db/re/item_db_*.yml` uses `bMaxAP` or `healap` (grep found none), so there is no consumable.
