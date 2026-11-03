# Skill casting: dispatch, hooks, cast flow, packets (renewal, PACKETVER 20260219)

Source: rAthena `$RATHENA_PATH` at `d4b8e7b8f`. All paths under `src/map/` unless noted. Applies to `20260219` (renewal) unless noted. Written for the "effects foundation" feature (First Aid, Bash, Mammonite, Heal, potions). Learn-only skill data (skill-up, list, reset) lives in [mechanics/skills.md](../mechanics/skills.md).

Tags: **[V]** verified by reading the source at the cited line; **[I]** inferred from source but not traced end to end; **[C]** confirmed by the real-client capture [`scripting_1.txt`](../captures/scripting_1.txt) (section 14; every `#NNNN` there is a proxy number, file line = number + 10). Default battle config values come from `conf/battle/skill.conf` (they equal the compiled defaults in `battle.cpp:8253+` unless noted).

## 1. Dispatch and hooks

* Source: `skills/skill_impl.hpp:8-90`, `skills/<job>/<skill>.cpp` (1242 files), `skill.cpp:4238,4590`. [V]
* Each skill with special behaviour is a `SkillImpl` subclass (`SkillImpl(SKILL_ID)`). Generic casting, requirements and delays are NOT in the subclass: they live in `unit_skilluse_id2`, `skill_check_condition_castbegin/castend`, `skill_consume_requirement` and are driven by `db/re/skill_db.yml`.
* Hooks (virtual, `skill_impl.hpp:18-58`): `castendNoDamageId` (support effect, called from `skill_castend_nodamage_id`, `skill.cpp:4590`), `castendDamageId` (`skill.cpp:4238`), `castendPos2` (ground), `calculateSkillRatio` (adds to `base_skillratio`), `modifyHitRate`, `applyAdditionalEffects`, `applyCounterAdditionalEffects`, `modifyDamageData`, `modifyElement`.
* Bases: `StatusSkillImpl` (`skill_impl.hpp:64`), `WeaponSkillImpl` (`:75`, castend = physical `skill_attack`), `SkillImplRecursiveDamageSplash` (`:82`).
* `skill_castend_id` picks `castend_nodamage_id` when the db `Type`/casttype is NoDamage, else `castend_damage_id` (`skill.cpp:5154-5157`). [V]
* Slice implementations: First Aid `skills/novice/firstaid.cpp:12-15` (0x09cb with 5, then heal 5, no scaling); Heal `skills/acolyte/heal.cpp:16-60`; Bash `skills/swordman/bash.cpp:12-30`; Mammonite `skills/merchant/mammonite.cpp:9-11`. [V]

## 2. Generic `skill_db.yml` fields

Source: `db/re/skill_db.yml` (loader `skill.cpp:14860+`). [V] for the field list, rest from R1.

* `CastTime`, `FixedCastTime`: exact renewal arithmetic in section 4.1 (YAML shapes: [../db/skills.md](../db/skills.md#skill_dbyml-castrequirement-fields-effects-import)). **Correction**: an absent `FixedCastTime` loads as 0, not -1, so the `default_fixed_castrate` 20/80 split does NOT run for shipped data (only an explicit `-1`, `NPC_EARTHQUAKE`). Neither slice skill has a CastTime.
* `AfterCastActDelay` (-> `canact_tick`, section 4), `AfterCastWalkDelay` (+`default_walk_delay` 300 ms for players, `skill.cpp:5145`), `Cooldown` (per-skill block + `0x043d`, `skill.cpp:5080-5086`), `Duration1/2`, `HitCount`, `Element`, `DamageFlags` (NoDamage, IgnoreDefense, Critical, ...), `TargetType` (Self/Attack/Support/Ground/Passive -> `INF_*_SKILL`), `Range` (negative = weapon range), `CastCancel`, `Flags` (`INF2_*`), `Requires` (`SpCost`, `HpCost`, `ZenyCost`, `ItemCost`, `Weapon`, `Ammo`, `State`, `Status`, `SpiritSphereCost`), `Unit`, `Status`. Level-indexed lists or one scalar.
* **Range -1** (Bash, Mammonite): with default `skillrange_from_weapon: 0` (`skill.conf:83`) `skill_get_range2` returns `1` (`skill.cpp:324-336`), i.e. adjacent, even with a bow. [V]

Slice rows (`skill_db.yml`): [V]

| Skill | Id | TargetType | Range | SP | Other | After-cast delay |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| First Aid `NV_FIRSTAID` | 142 | Self | 0 | 3 | NoDamage | none |
| Bash `SM_BASH` | 5 | Attack | -1 | 8 (lv 1-5), 15 (lv 6-10) | `Weapon` list (Fist, Dagger, 1h/2h Sword, 1h/2h Spear, 1h/2h Axe, Mace, 2h Mace, Staff, Knuckle, Musical, Whip; no Bow); `Duration2: 4500` (stun) | none |
| Mammonite `MC_MAMMONITE` | 42 | Attack | -1 | 5 | `ZenyCost` 100*lv | none |
| Heal `AL_HEAL` | 28 | Support | 9 | 13+3*(lv-1) | Holy, NoDamage, IgnoreDefense; no CastTime | 500 ms |

* Bash ratio: `100 + 30*lv` (`bash.cpp:12-14`). Stun only if `lv > 5` and Fatal Blow learned (`bash.cpp:22-29`), deferred. Mammonite ratio `100 + 50*lv` (`mammonite.cpp:9-11`). [V]

## 3. Cast flow (player, target skill), in order

1. `0x0438` -> `clif_parse_UseSkillToId` -> `clif_parse_skill_toid` (`clif.cpp:12870-12990`, parse stub `:12993`). Silent drops, no packet: `skill_lv < 1` is bumped to 1 (`:12875`); ground/passive skill (`inf` ground or 0) dropped; `pc_cant_act2` or chat open (`:12916`), sitting (`:12922`), skill blocked, menu open, cast in progress. Self skills force `target = self` (never trusted from the client). Level = `min(learned, requested)`; **0 learned -> dropped silently**. [V]
2. **Interval check** `canact_tick` (`clif.cpp:12937`): too early -> `0x0110` cause 4, but suppressed by default (section 6). [V]
3. `unit_skilluse_id2` (`unit.cpp:2077-2573`), exact order [V]: dead/blocked -> 0; `skill_check_condition_castbegin` (`unit.cpp:2324`; `skill.cpp:8237-9320`: SC_WEIGHT90 `WEIGHTOVER` first (`:8312`), then HP%/mhp (`:9220`), weapon (`:9246`), SP (`:9254`), zeny (`:9264`), ammo, items, state; each failure sends its own `0x0110`); `unit_stop_stepaction` (`:2342`, unconditional); walking step-action store (`:2345`); range check (silent drop, `:2362`); **`unit_stop_attack(src)` for non-combo skills** (`:2367`); `unit_stop_walking` (`:2477`); cast time via `skill_vfcastfix`. So SP/weapon/etc. refusals win over range and over the walking deferral: a 4-SP Bash on a far mob sends cause 1 whether standing or walking.
4. `clif_skillcasting` -> `0x0b1a` to AREA (**even with cast time 0**, `unit.cpp:2486`, `clif.cpp:5932-5972`). `delayTime` = cast ms, `element` from the skill (Neutral if none), `disposable` 0, `attackMT` 0.
5. `unit_set_attackdelay(CASTBEGIN_ID)` and `unit_set_castdelay` set `attackabletime`/`canact_tick` (section 4). Then instant cast calls `skill_castend_id` directly; otherwise a timer fires at `tick + casttime` (`unit.cpp:2538-2565`). [V]
6. `skill_castend_id` (`skill.cpp:4851-5190`): `skill_castend_id_check` (target relation; fail -> `0x0110`, section 6), `status_check_skilluse`, optional `skill_add_range` check, then **`skill_check_condition_castend`** (`:9325`): re-checks HP (cause 2, `:9497`) then weapon (cause 6, `:9502`) then ammo, items; each failure sends `0x0110` (not silent); it does **not** re-check SP or zeny. [V]
7. **Consume** `skill_consume_requirement(...,1)` (`skill.cpp:5017`, body `:9593-9680`): SP via `status_zap` -> `pc_damage` -> `0x00b0` SP_SP; then items; then zeny via `pc_payzeny` -> `0x00b1` SP_ZENY. Happens before the effect, so zeny/SP are spent on a miss too. If zeny is short at this point it pays what is left (`:9669`). [V]
8. `canact_tick = max(tick + delayfix, canact - 100)` (`skill.cpp:5076`); cooldown block; `0x0983` POSTDELAY (`:5108`, Heal only, section 4); `unit_set_walkdelay(300 + AfterCastWalkDelay)` (`:5145`).
9. Effect: `skill_castend_nodamage_id` (`0x09cb` then HP/SP push) or `skill_castend_damage_id` -> `skill_attack` (`0x01de`, HP lands after `amotion` via `battle_delay_damage`, `skill.cpp:3160-3175`). [V]
10. `ud->skill_id` cleared; `sd->skillitem` cleared (`skill.cpp:5179-5185`).

Out-of-order summary on the wire for a self cast: `0x0b1a` -> (`0x00b0` SP) -> (`0x00b1` zeny) -> (`0x0983` Heal only) -> `0x09cb` -> (`0x00b0` HP if HP actually changed). For Bash/Mammonite: `0x0b1a` -> `0x00b0` SP -> (`0x00b1`) -> `0x01de`, and the target's HP update after `amotion`. [V order; C confirmed, section 14]

## 4. Delays and `canact_tick`

Source: `unit.cpp:1869-1990` (`unit_set_attackdelay`, `unit_set_castdelay`), `skill.cpp:10385-10492` (`skill_delayfix`), `status.hpp:61`, `pc.cpp:15251`, `battle.cpp:8965`, `skill.hpp:201`. [V]

* **`skill_delayfix`** at default config for a player: `time = AfterCastActDelay[lv]` (negative values mean `-time + amotion`). `delay_dependon_dex` and `delay_dependon_agi` are both off, `delay_rate` is 100 (no change), and no `delayrate` item bonus or SC applies in the slice, so **result = `max(int(AfterCastActDelay), 0)`**. `min_skill_delay_limit` (100) is NOT applied here; it only applies in `unit_set_castdelay` (below) and to `no_skill_delay` unit types (default `BL_MOB` only).
* Cast begin (`DELAY_EVENT_CASTBEGIN_ID`, player, not an item skill): `attackabletime = max(old, T + adelay)`; `canact = max(old, T + amotion + maxaspd/2)` where `maxaspd/2 = 100` ms at default `max_aspd 190` (`max_aspd` is converted to `(2000 - 190*10)*2 = 200`, then `/AMOTION_DIVIDER_PC 2`). `adelay = 2 * amotion` for players.
* `unit_set_castdelay(casttime)`: `canact = max(canact, T + max(casttime, min_skill_delay_limit 100))`, then `+ SECURITY_CASTTIME 100`. With cast time 0: `canact = T + amotion + 200` after cast begin.
* Castend (same tick for 0 cast time): `canact = max(T + delayfix, canact - 100)`.
  * Bash, Mammonite, First Aid (no AfterCastActDelay): `canact = T + amotion + 100`. `attackabletime = T + adelay`, so the next auto-attack waits one full `adelay` from cast begin.
  * **Heal**: `canact = max(T + 500, T + amotion + 100)`. Bare-hand ASPD 150 (amotion 500) gives `T + 600`; below ASPD 160 the amotion term wins.
* **[C]** `canact = T + amotion + 100` confirmed: with amotion 590 First Aid casts 388 and 602 ms after the previous one were dropped silently, 679+ ms accepted; with amotion 440 Heal needed 540 ms and 592+ ms was accepted (section 14.1, 14.4).
* The attack timer and `canact` are separate fields in rAthena; metaling uses one shared `canact_at` (plan decision), so skills should also raise the auto-attack gate to `T + adelay`.
* **`0x0983` EFST_POSTDELAY**: `clif_status_change(EFST_POSTDELAY, flag 1, tick = delayfix)` at `skill.cpp:5108` (config `display_status_timers: 1`). `clif_status_change_sub` returns early when `type == EFST_POSTDELAY && tick == 0` (`clif.cpp:6479`), so **only Heal sends it** among the slice skills. EFST_POSTDELAY = **46** (counted from `status.hpp:1453-1503`, `EFST_BLESSING` = 10 cross-check). Layout (`clif.cpp:6488-6510`): op u16, efst u16@2, AID u32@4, flag u8@8, total u32@9, remain u32@13, val1/2/3 u32@17/21/25 = 29 bytes. Sent to AREA for a visible player (self included). **[C]** Captured: EFST id 46, flag 1, total = remain = 500, val1-3 = 0, `AID` = the **caster** (also when the target is another player), to caster and observer alike; Heal only (22 frames, no other skill sends it). Section 14.
* `0x043f` is the older (pre-20120618) form and `0x0196` the no-timer form (flag 0 end or `display_status_timers` off); neither is expected here.

### 4.1 Cast time at defaults: `skill_vfcastfix` (player, renewal)

Source: `skill.cpp:10253-10380` (function), caller `unit.cpp:2469-2471` (`skill_castfix_sc` only when `RENEWAL_CAST` is off) and `unit.cpp:1769,2589` (`skill_castfix` -> `skill_get_cast` = `CastTime[lv]` as `double`, `skill.cpp:10125`; the non-renewal body of `skill_castfix` is `#ifndef RENEWAL_CAST`). Config: `vcast_stat_scale` 530 (`battle.cpp:8552`, `conf/battle/skill.conf:54`), `default_fixed_castrate` 20 (range 0..100, `battle.cpp:8753`, `skill.conf:376`). `unit_skilluse_id2` may first rewrite `casttime` to -1 for special skills (basilica cancel, etc., `unit.cpp:2391-2460`) and doubles it for `GD_EMERGENCYCALL` with Leap; none apply to the slice. Applies to 20260219. [V]

Steps for a PC with no statuses, no item bonuses (`sd->bonus.*`, `skillfixcast` etc. all 0, `sc` empty), `castnodex` 0:

1. `time` = CastTime (double). If `time < 0` return 0. If caster is `BL_MOB` or `BL_NPC`: return `(int32)time` unchanged (no stat, no split, no fixed part: the fixed part is ignored for mobs).
2. `fixed` = `FixedCastTime[lv]` (int32). **If `fixed < 0`** (explicit -1 in YAML only): if `default_fixed_castrate > 0`: `fixed = (int32)time * rate / 100` (integer: truncate time first, then `*20/100`) and `time = time * (100 - rate) / 100` (double); else `fixed = 0`. Otherwise (`fixed >= 0`, including 0) `time` is left whole and `fixed` is used as given.
3. Item/status additive and rate terms: all zero at defaults (`reduce_cast_rate = 0`, `fixcast_r = 0`; `varcast_r` is never assigned in this function, so the `varcast_r < 0` block is dead).
4. Unless `castnodex & 1` (`IgnoreDex`): `time = time * (1 - sqrt(((float)(DEX*2 + INT)) / vcast_stat_scale))`. `DEX`/`INT` are `status_get_dex/int` (total, including bonuses). The division is float (the int is cast to float; the `int` scale is promoted), so the quotient is float32; whether `sqrt` then resolves to the float or double overload is toolchain-dependent, which only matters at exact-truncation boundaries. `time` stays double.
5. `time = time * (1 - (float)min(reduce_cast_rate, 100) / 100)` = unchanged at defaults.
6. Final: `result = (int32)( max((int32)time, 0) + (1 - (float)min(fixcast_r, 100)/100) * max(fixed, 0) )`. The variable part is truncated toward zero to an int32 FIRST (and clamped to >= 0), the fixed part is added as a double (at defaults exactly `fixed`), and the sum is truncated again (no rounding anywhere).

* **Clamp**: when `2*DEX + INT >= 530` the factor `1 - sqrt(x)` is <= 0 (it is NOT clamped before multiplying; it goes negative above 530), so `(int32)time <= 0` and the variable part becomes 0: the cast is instant except for the fixed part. At exactly 530 the factor is 0.
* Unset `FixedCastTime` (0) therefore means the **whole** `CastTime` is variable and reducible; a set `FixedCastTime` is a flat add-on never reduced by stats.
* Result 0 still sends `0x0b1a` (section 3) and skips the cast timer.
* `skill_castnodex` (`CastTimeFlags`): `IgnoreDex` 0x1 skips step 4; `IgnoreStatus` 0x2 skips status terms; `IgnoreItemBonus` 0x4 skips item terms. At defaults only `IgnoreDex` changes the result (52 skills carry it; then `time` stays unreduced).

Worked examples (float32 emulated; `sqrt` taken as float; `x = (2*DEX+INT)/530`):

| CastTime | FixedCastTime | DEX / INT | x | `1 - sqrt(x)` | variable part | fixed | result |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 1000 | absent (0) | 1 / 1 | 0.005660 | 0.924765 | 924.76 -> 924 | 0 | **924** |
| 1000 | -1 (explicit; split 20%) | 1 / 1 | 0.005660 | 0.924765 | 800 * 0.924765 = 739.81 -> 739 | `1000*20/100` = 200 | **939** |
| 1000 | absent (0) | 50 / 30 | 0.245283 | 0.504739 | 504.74 -> 504 | 0 | **504** |
| 2000 | 500 | 99 / 99 | 0.560377 | 0.251416 | 502.83 -> 502 | 500 | **1002** |
| 1000 | absent (0) | 200 / 70 | 0.886792 | 0.058303 | 58.30 -> 58 | 0 | **58** |
| 1000 | absent (0) | 265 / 0 | 1.0 | 0 | 0 | 0 | **0** |

`skill_delayfix` re-confirmed (`skill.cpp:10385-10492`): `time = AfterCastActDelay[lv]`; `time < 0` -> `-time + status_get_amotion`; for non-listed skills with `delay_dependon_dex`/`agi` off and `delay_rate` 100 there is no further change; returns `max((int32)time, 0)`. [V]

## 5. Packets at 20260219 (layouts and scopes)

Struct sizes from `packets_struct.hpp` / `packets.hpp` resolved for `PACKETVER 20260219` (all `>= 2018` branches). Client opcodes for this version are NOT shuffled (`clif_shuffle.hpp:4723-4761` `PACKETVER > 20180307` branch): `0x0438` (10) UseSkillToId, `0x0366` (10) UseSkillToPos, `0x0367` (31) UseSkillToPosMoreInfo; `0x0439` (8) UseItem from `clif_packetdb.hpp:1151`. [V]

| Op | Len | Dir | Name | Layout (offsets) | Scope |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `0x0438` | 10 | C->M | CZ_USE_SKILL | lv u16@2, skill u16@4, target u32@6 | |
| `0x0439` | 8 | C->M | CZ_USE_ITEM | index u16@2, AID u32@4 (index = inventory slot + 2) | |
| `0x0b1a` | 29 | M->C | ZC_USESKILL_ACK | src u32@2, dst u32@6, x u16@10, y u16@12, skill u16@14, element u32@16, delay_ms u32@20, disposable u8@24, attackMT u32@25 (`packets_struct.hpp:3952-3964`) | AREA |
| `0x09cb` | 17 | M->C | ZC_USE_SKILL | skill u16@2, value i32@4, target u32@8, src u32@12, result u8@16 (`:4675-4683`; `clif.cpp:6163`) | AREA of the target |
| `0x01de` | 33 | M->C | ZC_NOTIFY_SKILL2 | skill u16@2, src u32@4, dst u32@8, tick u32@12, sdelay i32@16, ddelay i32@20, damage i32@24, lv i16@28, div i16@30, type u8@32 (`:4658-4671`; `clif.cpp:6051`) | AREA |
| `0x0110` | 14 | M->C | ZC_ACK_TOUSESKILL | skill u16@2, btype i32@4, itemId u32@8, flag u8@12 (0 = fail), cause u8@13 (`packets_struct.hpp:2448-2461`; `clif.cpp:6008`) | SELF |
| `0x0983` | 29 | M->C | ZC_MSG_STATE_CHANGE3 | section 4 | AREA |
| `0x043d` | 8 | M->C | ZC_SKILL_POSTDELAY | skill u16@2, delay u32@4 (cooldown; only with `Cooldown`) | SELF |
| `0x01b9` | 6 | M->C | ZC_DISPEL | gid u32@2 (cast cancelled; `clif_skillcastcancel`, `clif.cpp:5975`) | AREA |
| `0x0117` | 18 | M->C | ZC_NOTIFY_GROUNDSKILL | skill u16@2, src u32@4, lv i16@8, x i16@10, y i16@12, tick u32@14 | AREA; ground skills only |
| `0x0147` | 39 | M->C | ZC_AUTORUN_SKILL | item-skill prompt (`clif.cpp:7303`) | SELF; item skills |
| `0x01c8` | 15 | M->C | ZC_USE_ITEM_ACK | index u16@2 (slot+2), itemId u32@4, AID u32@8, amount i16@12 (remaining), result u8@14 (`packets_struct.hpp:2577`; `clif.cpp:4483-4496`; opcode `packets_struct.hpp:312`) | AREA if ok, SELF if not |
| `0x00b0` | 8 | M->C | ZC_PAR_CHANGE | var u16@2, value i32@4. SP_HP=5, SP_SP=7, SP_WEIGHT=24 (`map.hpp:501`, `clif.cpp:3550,3683-3694`) | SELF |
| `0x00b1` | 8 | M->C | ZC_LONGPAR_CHANGE | var u16@2 (SP_ZENY=20), value i32@4 (`clif.cpp:3735`) | SELF |
| `0x0a27` | 8 | M->C | ZC_RECOVERY | type u16@2, amount i32@4 (`packets.hpp:1210-1215`, `clif.cpp:6840`) | SELF; see below |

* **`0x0b1a` element** (`clif_skillcasting`, `clif.cpp:5942-5946`): sent only if `ELE_NONE < ele < ELE_ALL` (0-9), else 0. So `Weapon`, `Endowed`, `Random` (`map.hpp:402-407`) all send 0; Heal sends 6 (Holy). [V, [C] Bash = 0 with Fire weapon/endow]
* **`0x09cb` field `value`** is named `level` in rAthena: `clif_skill_nodamage(src, dst, id, heal)` clamps `heal` into it, `result` = success (1). For Heal/First Aid `value` is the heal amount.
* **ZC_RECOVERY (`0x0a27`; the old `0x013d` is for `PACKETVER < 20141022`)** is sent by `pc_heal` only when `status_heal` flag has bit 2 (`pc.cpp:10669-10674`), Skills use flag 0 and potions flag 1, so they get `0x00b0`, not `0x0a27` [V]. **Captured correction:** natural regen ticks in `scripting_1.txt` arrive as plain `0x00b0` (HP every 6 s, SP every 8 s standing), never as `0x0a27`. The 13 `0x0a27` frames there are HP (type 5) followed by `0x080e`, one 0.54 s after a kill, so their trigger is not regen and is still unconfirmed (catalog `mechanics/stats.md` "Capture: scripting_1.txt (regen and `0x0a27`)").
* **Item-use ack order** (`pc.cpp:6525-6560`): `0x01c8` (amount = remaining) -> `pc_delitem(..., type 1)` (no `0x07fa`, but `SP_WEIGHT` `0x00b0` is sent, `pc.cpp:6122-6123`) -> `run_script` -> `pc_itemheal` -> `status_heal(...,1)` -> `0x00b0` SP_HP / SP_SP. `canuseitem_tick = tick + item_use_interval` is set before the script. Items with `delay_consume` ack with the amount unchanged. [V]
* The old forms `0x07fb` (25), `0x013e` (24), `0x011a` (15), `0x013d` (6), `0x0114` (31) are NOT used at 20260219.

## 6. `0x0110` failure causes and suppression

Enum `clif.hpp:405-450`; sender `clif.cpp:6008-6031`. [V]

| Condition | Where | `cause` | Sent by default? |
| :--- | :--- | :--- | :--- |
| Not enough SP | castbegin `skill.cpp:9254` | 1 `SP_INSUFFICIENT` | yes |
| Not enough zeny | castbegin `skill.cpp:9264` | 5 `MONEY` | yes |
| HP too low (HpCost) | `:9220` / castend `:9497` | 2 | yes |
| Wrong weapon | castbegin `:9246`, castend `:9502` | 6 `THIS_WEAPON` | yes |
| Missing item | castbegin/castend | 3 `STUFF_INSUFFICIENT` (with item id) | yes |
| Weight >= 90% (`SC_WEIGHT90`, set by `pc_updateweightstatus` `pc.cpp:3029-3045` when `pc_getpercentweight >= major_overweight_rate` = 90, `player.conf:77`) | castbegin `skill.cpp:8312`, before HP/weapon/SP/zeny | 9 `WEIGHTOVER` | yes |
| Skill interval (`canact` not reached) | `clif.cpp:12937` | 4 `SKILLINTERVAL` | **no** |
| Wrong target (Bash on a non-enemy, Heal never fails this way) | castend `skill.cpp:4789-4800`, sent `:4982-4985` | 0 `LEVEL` | yes |
| Out of range at cast begin | `unit.cpp:2362` | none (silent `return 0`) | no packet |
| Out of range at castend (only if `skill_add_range` > 0, default 0) | `skill.cpp:4997-5006` | 0 (default arg) | n/a at defaults |

* **`display_skill_fail: 2`** (`skill.conf:173`): bit 1 would disable all, bit 2 clear sets `state.showdelay` (`pc.cpp:2170`). Default value 2 has bit 2 SET, so `showdelay = 0` and `clif_skill_fail` returns early for `SKILLINTERVAL` (`clif.cpp:6012`). **A too-early cast is silent.** Only `@showdelay` toggles it. Bits 4 and 8 hide Snatcher / Envenom failures. [V] **[C]** 14 First Aid casts sent 0.4-0.6 s after the previous accepted one got no `0x0110` and no other reply (section 14.1).
* Level 0 learned, ground/passive skills, sitting, cannot-act, chat open: silent `return` before any packet (section 3).
* **Failed castend consumes nothing [V, [C] Pierce on a player]**: castend runs `unit_set_attackdelay(CASTEND)` first (`skill.cpp:4895`), which for players has no case, so `attackabletime` and `canact` are untouched (`unit.cpp:1873-1890`). Every failure `break` (`skill.cpp:4982-4985`) skips the `canact = max(tick + delayfix, canact - SECURITY_CASTTIME)` line (`:5076`) and the walk delay (`:5143`). So `canact` stays at the cast-begin value `T + amotion + 200` (the 100 ms security time is NOT removed). No SP spent; the fail packet comes after `0x0b1a`.
* `GM` bypass is `skill_unconditional`; the stock `Admin` group has it `false` (`conf/groups.yml:233`), so a GM test account still gets SP/zeny/weapon refusals.
* `skill_out_range_consume` is `no` in `skill.conf:72` (compiled default 1); only matters when `skill_add_range` > 0.

## 7. Heal: renewal formula, targets, packets

Source: `skill.cpp:518-758` (`skill_calc_heal`), `skills/acolyte/heal.cpp`, `status.cpp:2559-2600,6285-6330`. [V]

* Order inside `skill_calc_heal(src, target, AL_HEAL, lv, heal=true)`:
  1. `lv >= max_heal_lv (11)` -> `max_heal` 9999 (unreachable at lv 10).
  2. `hp = (BaseLv + INT) / 5 * 30 * lv / 10` — integer ops left to right (`/5` first, then `*30`, `*lv`, `/10`).
  3. `hp_bonus` (percent, additive): `HP_MEDITATIO` (Meditatio) `2 * lv`; other terms skippable: Power of Sea +10/+20, NV_BREAKTHROUGH `2*lv`, NV_TRANSCENDENCE `3*lv`, item `bHealPower` (caster), `bHealPower2` (target), Offertorium, target SC_INCHEALRATE / ANCILLA / ASSUMPTIO. `hp += hp * hp_bonus / 100` (int).
  4. **MATK term added after** the bonus: `min = base_matk_min`, `max = base_matk_max` (player: `INT + INT/2 + DEX/5 + LUK/3 + BaseLv/4 + 5*SPL`, min == max; `status.cpp:2559-2594`), plus weapon MATK `w = rhw.matk`, `variance = w * weaponlv / 10`, `min += w - variance`, `max += w + variance`; `overrefine` adds `min += 1`, `max += overrefine - 1`. If `max > min`: `hp += min + rnd() % (max - min)` (max excluded), else `hp += min`. Unarmed/non-magic weapons have `w = 0`, so MATK is deterministic. Status-window MATK extras (`matk_rate`, ematk, SC_MAGICPOWER) are NOT used; Heal recomputes base + weapon only (`skill.cpp:700-731`).
  5. `global_bonus` (1.0 unless Water Insignia / Critical Wound / Death Hurt / NORECOVER penalties): `hp = (int)(hp * global_bonus)`.
  6. `hp += hp * HPlus / 100` (`status_get_hplus`, 0 by default); `return max(1, hp)` for `heal = true`.
* Defaults for the slice: skip every term except step 2, the HP_MEDITATIO term (if skill 1 level learned) and step 4. Truncations: each `/` above is integer; the MATK add is the only random roll.
* Castend (`heal.cpp:16-60`): `heal` computed -> forced 0 for immune targets (Emperium, battlefield class), Berserk, Kaite handling, then `clif_skill_nodamage(src, target, AL_HEAL, heal)` (**0x09cb with the computed amount, before the HP change**), then `status_heal(target, heal, 0, 0)`. `status_heal` clamps to `max_hp - hp` and returns without any push when nothing changes (`status.cpp:1803-1850`). **At full HP: `0x09cb` carries the full computed heal, and no `0x00b0` HP follows.** [V]
* `0x09cb` is sent AREA of the target, so an observer sees it. The HP push (`0x00b0` SP_HP) is `SELF` of the target (`pc_heal` -> `clif_updatestatus`, `pc.cpp:10676-10681`).
* Undead target (`battle_check_undead`): player-cast Heal becomes holy damage (needs enemy relation, else `clif_skill_fail` cause 0) (`skill.cpp:4416-4428`). Deferred.
* **Heal on a non-undead mob is allowed**: Heal has no `NoTargetEnemy` flag, so `skill_castend_id_check` skips the relation check (`inf == 0`, `skill.cpp:4784-4800`), `status_check_skilluse` has no mob restriction, and `status_heal` -> `mob_heal` raises the mob's HP. Heal on another player needs no party/PvP. Heal EXP (`heal_exp` default 0) only for player targets. **[C]** captured on a Poring: `0x09cb` carries the full 64 (not clamped) and `0x0977` (mob HP) follows in the same millisecond, sent only to players in the mob's damage log (section 14).
* First Aid (`firstaid.cpp`): `0x09cb` value 5, then `status_heal(target, 5)`. No scaling.

## 8. Range, walking, auto-attack interplay

* **Out of range never walks the player and never refuses with a packet.** At cast begin `unit_skilluse_id2` checks `battle_check_range(src, target, range)` (`unit.cpp:2360-2363`, `battle.cpp:8215-8240`: circular client distance for players plus a line-of-sight `path_search_long` when `d >= 2`; `> AREA_SIZE` fails) and silently returns 0. The client is expected to walk into range itself, then re-send. Bash/Mammonite range is 1 (section 2). [V]
* **Cast clears a pending step action [V]**: `unit_stop_stepaction` (`unit.cpp:2342`, body `:2848-2867`) runs after castbegin passes and zeroes `stepaction`, `target_to`, `stepskill_*` and deletes `steptimer`. A queued attack click is lost: walk-then-First-Aid (self, so no new step stored) leaves nothing to fire; `unit_stop_attack` also zeroes `ud->target`. If castbegin fails (SP etc.) it returns first and the pending attack step action survives.
* The one server-side deferral: if the player is walking when `0x0438` arrives and the target is out of `range - 1` (or `ignore_range`), the request is stored as `ud->stepaction` with `stepskill_id/lv` and fired by the walk timer when the step ends (`unit.cpp:2343-2350`, `unit.cpp:2696+` for ground). It is not a pathing request: it only runs while an existing walk lasts. [V]
* **A skill cast stops the auto-attack**: `unit_stop_attack(src)` for every non-combo skill (`unit.cpp:2367`) clears `ud->target` and deletes `attacktimer` (`unit.cpp:2826-2842`). `attack_continue` is left set but nothing re-arms the timer, so the **server does not resume** the loop after the skill. **[C]** Confirmed: after an accepted Bash the loop stays stopped, and the client does not re-send `0x0437` on its own (also with `/noctrl`); only the user's next click does. (`combo` skills keep it, not in the slice.)
* **Walking at cast begin [V, [C]]**: `unit_skilluse_id2` first stores a step action only if `src` is a PC, `walktimer` is active and `!battle_check_range(src, target, range - 1)` (or `ignore_range`) (`unit.cpp:2345-2350`; not `check_distance`). Otherwise (target within `range - 1`, or a self-cast) it continues, and before `clif_skillcasting` calls `unit_stop_walking(src, USW_FIXPOS)` unless `ud->state.running` (`unit.cpp:2475-2477`). `USW_FIXPOS` (0x1) does nothing if not walking, else sets sx/sy = 8 and `clif_fixpos` = `0x0088` ZC_STOPMOVE to AREA (`unit.cpp:1732-1736`, `clif.cpp:2205-2213`), sent just before `0x0b1a`. Attack skills behave the same: in range, stop and cast at once.
* Cast begin also sets `attackabletime` so a re-issued attack waits for `T + adelay` (section 4).

## 9. Weapon skill damage (Bash, Mammonite)

Source: `battle.cpp:5387-5800` (`battle_calc_weapon_attack`), `3017-3075`, `3228-3315`, `2671-2745`, `4407+`. [V]

* **Lucky dodge** (`flee2`) is checked only when `!skill_id` (or `PA_SACRIFICE`) (`battle.cpp:5423-5424`), so skills never get dodged by it.
* **Crit**: `is_attack_critical` returns false for any skill without the `Critical` damage flag (`battle.cpp:3025`). Bash and Mammonite have no `DamageFlags`, so they never crit.
* **Hit check** (renewal): `hitrate = 0 + attacker.hit - target.flee`, agi-penalty on flee, `+AC_VULTURE` per level for players, then the skill `modifyHitRate`, then `cap(min_hitrate 5, max_hitrate 100)`; hit if `rnd() % 100 < hitrate` (`battle.cpp:3237-3315`). Perfect-hit and status-opt1 shortcuts skip it. **Bash `modifyHitRate`: `hit_rate += hit_rate * 5 * lv / 100`** (int math on the already-computed `hit - flee`, before the cap) (`bash.cpp:17-20`). Mammonite has none. Note this multiplies a possibly negative value, then clamps to 5.
* **Order in renewal** (player): base ATK pieces (`statusAtk + weaponAtk + equipAtk + percentAtk`) -> multiplied by `(100 + P.ATK) / 100` (floor) -> `+ masteryAtk` -> crit rate bits (skip) -> `ATK_RATE(skill ratio)` where the ratio is `100 + 30*lv` etc. from `calculateSkillRatio` (applied to the whole sum including mastery, `battle.cpp:5566-5567`) -> `+ constant addition` -> RES reduction -> DEF reduction (`battle_calc_defense_reduction`) -> element fix -> skill/post bonuses -> final modifiers. So **the skill ratio comes after P.ATK and mastery**, before DEF and element.
* **[C]** Verified end to end against five captured Bash/Mammonite hits (section 14.2): status ATK enters as `2 * ATK1` (the `0x00b0` var 41 value), the ratio multiplies it together with weapon and Sword Mastery, then `(4000+DEF)/(4000+10*DEF)` and the mob's soft DEF `(level+Vit)/2` (Vit default 1) are applied. Misses show as damage 0 with ddelay 0.
* **Range flag**: `battle_range_type` (`battle.cpp:2671-2745`): after the special cases, `skillrange_by_distance: 14` (`skill.conf:78`) covers only non-PC units, so players use `skill_get_range2(...) < 4 -> BF_SHORT`, else `BF_LONG`. With Range -1 and `skillrange_from_weapon: 0` that range is 1, so Bash and Mammonite are always `BF_SHORT`, bow or not. (Bash does not allow a bow; Mammonite does.) Auto-attacks use arrows -> `BF_LONG` instead (`battle.cpp:5316`).

## 10. Costs: SP and zeny pushes

Sources: `skill.cpp:9593-9680`, `pc.cpp:5795-5815`, `clif.cpp:3637-3740`, `pc.cpp:9684-9686`. [V]

* SP: `skill_consume_hpspap` -> `status_zap` -> `status_damage` -> `pc_damage` -> `clif_updatestatus(SP_SP)` = **`0x00b0` var 7 with the new SP**, sent at step 7 of the cast flow, i.e. before `0x0983`, `0x01de` and `0x09cb`.
* Zeny: `pc_payzeny` -> `clif_updatestatus(SP_ZENY)` = **`0x00b1` var 20 with the new zeny** (`longpar`, i32), after the SP push, still before the effect. It is spent on a miss as well; `Mammonite` has no "refund on miss". (`showzeny` message only if the char has it enabled.) Zeny at login/map entry is NOT pushed by the map server: the char server fills `info.money = p.zeny` in `char_mmo_char_tobuf` (`char_clif`/`char.cpp:1781`) for the `0x006b` char list, and `clif_parse_LoadEndAck` (`clif.cpp:10748-10916`) and `clif_initialstatus` send no `SP_ZENY`. `SP_ZENY` pushes exist only in `pc_payzeny/pc_getzeny` (`pc.cpp:5810,5844`), trade and mail. [V] [C] no `0x00b1` at entry or on map change.
* HP push after a heal: `0x00b0` var 5 (`pc_heal` flag-0 path). Natural regen also pushes `0x00b0`, not `0x0a27` (captured, section 5).

## 11. Item-origin casts (`itemskill`)

* `script.cpp:11043+` (`BUILDIN_FUNC(itemskill)`): sets `sd->skillitem`, `skillitemlv`, `skillitem_keep_requirement`, then `clif_item_skill` -> `0x0147` (`clif.cpp:7303`). The client answers with `0x0438` for that skill. [V]
* In `clif_parse_skill_toid`: if `sd->skillitem == skill_id` the level is forced to `skillitemlv`, the interval check is bypassed (`clif.cpp:12937`, `:12961`), and the cast goes straight to `unit_skilluse_id`. [V]
* `skill_check_condition_castbegin` (`skill.cpp:8283-8310`): for a skill-item it deletes the delay-consume item and returns true (no SP/zeny/weapon checks) unless `skillitem_keep_requirement`. `unit_set_attackdelay` returns early for item skills (`unit.cpp:1884-1890`), and castend applies `canact` only if the skill has a delay (`skill.cpp:5075-5076`). [V]

## 12. Mob skill casts

* `mob_skill_db.txt` (14514 data rows; fields MobID, State, SkillID, Lv, Rate/10000, CastTime, Delay, Cancelable, Target, Condition, val...). `mobskill_use` calls the same `unit_skilluse_id2` / `unit_skilluse_pos2` with the row's casttime (`mob.cpp:4436,4485`). [V]
* `skill_vfcastfix` returns the time unchanged for `BL_MOB` (`skill.cpp:10259`), so the row's cast time is used as is. No SP or requirements for mobs. Mob delays are applied at castend/cancel only (`unit.cpp:1888-1900`, `mob_set_delay`). [V]

## 13. Open items from the first capture: settled

All five items are answered by `scripting_1.txt`, see section 14: (1) `0x09cb` carries the full computed heal at full HP and no `0x00b0` HP follows (14.1, 14.4); (2) `0x0983` comes after `0x00b0` SP and before `0x09cb`, to caster and observer, Heal only (14.4); (3) the client never re-sends an attack after a skill (14.2); (4) the client walks itself, with `0x035f`, and sends `0x0438` while still walking (14.2); (5) `0x00b0` SP and `0x00b1` zeny come right after `0x0b1a` and before `0x01de` (14.2, 14.3). Still open: partial-zeny cast (zeny between 1 and the cost, never reached), Heal spam below `canact` (never reached), a cast on a target outside the area (not exercised).

## 14. Capture: scripting_1.txt (skills)

Source: [`captures/scripting_1.txt`](../captures/scripting_1.txt), rAthena `d4b8e7b`, `PACKETVER 20260219`, full payloads. Applies to 20260219. `#N` = proxy number (file line = N + 10). A = `skilltest2`, map session `#7`; B = `skilltest1`, session `#4`, watching. A: AID `0x1e8480` (2000000), B: `0x1e8481`. Mob ids `0x68ee...`. All values below are decoded from the payload; "+N ms" is proxy time.

**A and B are not partied** (no party packet anywhere). The `0x080e` (aid u32, hp u32, maxhp u32; 14 bytes) after every HP change goes to the player himself and to B as a non-party viewer, as in [mechanics/experience.md](../mechanics/experience.md).

**Caster stats** (from the `0x00bd`/`0x0141`/`0x00b0` pushes, `#175-#256`, `#1213-#1237`, `#1872-#1883`, `#6541-#6548`): STR/AGI/VIT/INT/DEX/LUK all 1 the whole time (the only bonus is the Swordman job-level STR, `0x0141` type 13, at job level 2, ATK1 4 to 5). `@blevel 12` is relative: BaseLv 1 to **13**. HIT 189, FLEE 114, MaxHP 101/SP 23 (Novice), 164/36 (Swordman), 137/49 (Merchant), SP 75 (Acolyte). Weapon: Knife (1201, dagger, ATK 17, wlv 1) until A takes it off at `#2433-#2438` (`0x00ab`/`0x099a`), after which ATK2 = 0. `SP_ASPD` (var 53) is amotion in ms: 590 Novice+Knife, 510 Swordman+Knife, 440 Swordman unarmed (kept as Merchant and Acolyte). Skills were all learned with `@allskill` (Bash 10, Sword Mastery 10, Mammonite 10, Heal 10), First Aid with `@questskill 142`.

### 14.1 First Aid (skill 142, lv 1, Novice, amotion 590)

* Request `0x0438`: payload `lv u16=1, skill u16=142, target u32=A's own AID` (the client sends the self id).
* Wire order on **A** (`#1666` request, not full HP): `0x0b1a` (`#1669`, +5 ms) -> `0x00b0` SP (`#1671`, 23 to 20, cost 3) -> `0x09cb` (`#1672`) -> `0x00b0` HP (`#1673`, 58+5 = 63) -> `0x080e` (`#1674`). **B** sees `0x0b1a` (`#1667`), `0x09cb` (`#1668`), `0x080e` (`#1670`, after the heal). Cross-session order in the proxy log is not fixed (A or B first); within one session it is.
* `0x0b1a`: src = dst = A, x = y = 0, skill 142, **element 0, delay 0, disposable 0, attackMT 0** (sent although there is no cast time). `0x09cb`: skill 142, **value 5**, target = src = A, result 1.
* **At full HP** (`#1821` request, HP 101/101): A gets `0x0b1a` (`#1822`), `0x00b0` SP (`#1823`, 3 to 0), `0x09cb` value **5** (`#1824`), then nothing: no `0x00b0` HP, no `0x080e`. B gets `0x0b1a` + `0x09cb` (`#1825-#1826`) only. No `0x0983`, no `0x0b1a` delay.
* Spam (`#1666-#1750`, 11 requests): the user clicked every 0.4-1.1 s. Accepted: 679, 722, 952 ms (and longer) after the previous accepted request; **silently dropped**: 388 ms and 602 ms after the previous accepted one (`#1715`, `#1726`: no packet at all, no `0x0110`). Matches `canact = T + amotion + 100` = 690 ms (the 679 ms accept was processed 57 ms late). The client does not debounce: it sent two requests 230 ms and 458 ms apart.
* **Not enough SP**: `#1760` (SP 0), `#1790` (SP 2), `#1798` (SP 2): `0x0110` payload `skill u16=142, btype i32=0, itemId u32=0, flag u8=0, cause u8=1` (`#1761`, `#1793`, `#1799`), 10-62 ms after the request, to the caster only. Nothing else is sent. No SP or HP change.
* Sitting (`0x0437` action 2, `#1804`) pushes `0x0983` efst **622** flag 1 total = remain = 9999 to both and `0x08c8`; standing sends `0x0196` efst 622. Not a skill packet, noted to avoid mistaking it for POSTDELAY.

### 14.2 Bash (skill 5; Swordman)

* `0x0438` payload: `lv u16, skill u16=5, target u32` (target = the mob id).
* **Wire order on A** (`#1981` request, lv 10, Poring): `0x0b1a` (`#1982`, +138 ms here, +5 ms typical) -> `0x00b0` SP (`#1983`, 36 to 21, cost 15) -> `0x01de` (`#1984`); B gets `0x0b1a` + `0x01de` (`#1985-#1986`). No `0x09cb`, no `0x0983`, no `0x00b1`. Same shape every time (`#2032-#2036`, `#2349-#2353`, `#2395-#2399`, ...).
* `0x0b1a`: src A, dst mob, x = y = 0, skill 5, **element 0, delay 0, disposable 0, attackMT 0**.
* `0x01de` (hit): `skill u16=5, src, dst, tick u32 (server tick), sdelay i32=510, ddelay i32=480, damage i32, lv i16, div i16=1, type u8=8`. **sdelay = A's amotion** (510 Knife, 440 unarmed), **ddelay = the target's damage motion** (Poring 480, Pupa 1). `type` 8 = `DMG_MULTI_HIT` (the `>= 20131223` rewrite of `DMG_SINGLE`, `clif.cpp` `clif_skill_damage`), always 8, also on a miss.
* **Miss**: same `0x0b1a`, SP spent, `0x01de` with **damage 0 and ddelay 0**, lv/div/type unchanged, then **no `0x0977`** (`#2588-#2592`, `#3830-#3834`). 2 misses in 31 Bash hits (`#2592`, `#3832`; both lv 1 unarmed), 2 in 14 Mammonite casts. Expected: HIT 189 vs a Poring's FLEE 102 is about 91% with Bash's +5%/lv.
* **Mob HP**: `0x0977` (mob id u32, hp u32, maxhp u32; 14 bytes) goes to A only (B never received one in the whole capture) **amotion + about 5-20 ms after `0x01de`**: 523 and 525 ms (sdelay 510, `#1987`, `#2039`), 530 ms (`#2401`), 445-463 ms (sdelay 440, `#2466`, `#2480`). Death is a `0x0977` with hp 0 in the same frame as `0x0acb`/`0x0acc`/`0x0add` (`#1987-#1995`), vanish type 1 about 250 ms later (`#1998`). HP lands after the swing delay, not at the cast.
* Damage fits the renewal formula exactly (section 9; sources `battle.cpp:3889-3925` status ATK doubled, `:2443-2495` weapon part, `:2338` Sword Mastery, `:4859-4871` DEF, `status.cpp:2646-2650` mob soft DEF): `T = 2*ATK1 + W + M`, `dmg = floor(floor(T * (100+30*lv)/100) * (4000+DEF)/(4000+10*DEF)) - softDEF`, softDEF of a mob = (level + Vit)/2 with mob Vit default 1 (`mob_db.yml:43`), W = rnd(atkmin..atkmax) of the weapon scaled by the size table (Dagger: Medium 75%, Small 100%), M = Sword Mastery 4*10 = 40 (dagger, learned 10), unarmed W = M = 0:

| Request | ATK1 | W | M | T | Ratio | Poring/Pupa DEF, softDEF | Computed | Captured |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| lv10, Knife, JL1, Poring (`#1984`) | 4 | 12 (16 or 17 x 75%) | 40 | 60 | 400 | 2, 1 | 237 | **237** |
| lv10, Knife, JL2, Poring (`#2352`) | 5 | 13 (18 x 75%) | 40 | 63 | 400 | 2, 1 | 249 | **249** |
| lv5, Knife, JL2, Pupa (`#2398`) | 5 | 16 (x 100%) | 40 | 66 | 250 | 24, 3 | 153 | **153** |
| lv1, unarmed, Poring (`#2462` and 20 more) | 5 | 0 | 0 | 10 | 130 | 2, 1 | 11 | **11** every time |
| Mammonite lv10, unarmed Merchant, Poring (`#5826`) | 4 | 0 | 0 | 8 | 600 | 2, 1 | 46 | **46** every time |

  Knife weapon range: `variance = 5*17*1/100 = 0.85`, `stat bonus = 17*STR/200`, so STR 1 gives 16-17, STR 2 gives 16-18 (`battle.cpp:2475-2480`). Unarmed is deterministic. Poring is Water 1, Pupa Earth 1: both 100% to a neutral hit. The first table row also pins that the ratio multiplies the mastery part. `W` of 13 / 16 is a possible roll, not the only one.
* **Not enough SP**: `0x0110` cause 1 (`#1957` at SP 3 for lv10, `#5322`, `#5325`, `#5331` at SP 0-1 for lv 1, 5, 10, `#3572`, ...): 4-62 ms after the request, caster only, no SP change. 23 such packets in the capture; no other cause appears for Bash.
* **Step 3, Bash in the middle of an auto-attack loop** (`#3075-#4674`; loop = one `0x0437` `target_id`, `action 7`, then the server swings every `adelay` = 880 ms with `0x08c8`, 8 damage each):
  * Accepted casts: `0x0b1a`/`0x00b0`/`0x01de` as above, and **the swings stop** (no `0x08c8` from A until the user's next `0x0437`; e.g. cast `#3323` at +16.05 s, next swing only after the user's `0x0437` `#3367` at +18.75 s). The server sends nothing to say the loop ended. The client never re-sends `0x0437` itself, also with `/noctrl` (`#3955-#4013`, `#4650`). The mob keeps attacking A.
  * In 4 of the 7 accepted mid-loop casts the client sent **`0x0118` (`CZ_CANCEL_LOCKON`, empty) first**, 250-550 ms before `0x0438` (`#3320/#3323`, `#3537/#3542`, `#3765/#3766`, `#3827/#3829`; Step 2 `#2930/#2931` too); the other three had none (`#4209`, `#4438`, `#4565`). Both forms stop the loop the same way. The server sends no reply to `0x0118`, and the client also sends it without any cast (`#3161`, `#3374`).
  * **Dropped mid-loop**: 9 `0x0438` requests got **no reply at all and the swings went on**: `#4066`, `#4297`, `#4305`, `#4316`, `#4336`, `#4420`, `#4428`, `#4534`, and `#4137` (a different mob, not adjacent). Each of the first eight arrived 1-22 ms *before* the next scheduled swing (`#4534`: 1 ms *after* one). Four of them had SP 2-3 (cost 8) and still got **no `0x0110`**, so they were dropped before the SP check, i.e. by `clif_parse_skill_toid`'s `canact` test with `canact = swing + amotion + 100` already moved forward, which means the server handled the swing timer before the packet. Three near-identical requests (`#4209`, `#4438`, `#4565`: 0 ms, 1 ms after and 5 ms before the due swing) were accepted and that swing did not happen. [I] race between the swing timer and the packet inside the same ~5 ms server step; the source gives no other reason.
* **Step 4, target out of reach** (`#4674-#5322`): the client **walks itself** with `0x035f` (`RequestMove`) to a cell next to the mob, re-sending every 400-450 ms as the mob moves (`#5002`, `#5008`, `#5013` to cell 172,358 / 173,357 / 174,357), and sends `0x0438` **while the last walk (`0x0087`) is still in progress**, 375 ms after the last `0x035f` (`#5016`). The server did not drop it: `0x0b1a` followed 174 ms after the request, cast tick = `0x0087` tick + 397 ms for a 2-cell walk (second sample `#5196-#5205`: 268 ms request, 387 ms after the walk start). [I] = the `stepaction` deferral of section 8, fired when the walk ended; the only direct proof is that the cast was not dropped and ran after the walk. The server never walked A and never sent a refusal. Without the walk (`#4137`) the request was silently dropped.
* **Step 5, not enough SP**: `#5321`, `#5324`, `#5330` (lv 1, 5, 10) at SP 0-1: one `0x0110` cause 1 each (4-58 ms).

### 14.3 Mammonite (skill 42, lv 10, Merchant, unarmed; amotion 440)

* Request `0x0438` lv 10, skill 42, target mob. Cost 5 SP + 1000 zeny (100*lv).
* **Wire order on A** (`#5820`): `0x0b1a` (`#5822`) -> `0x00b0` SP (`#5823`, 8 to 3) -> **`0x00b1` zeny** (`#5824`, var 20, **u32 value 4000**, was 5000 from `@zeny 5000`, `#5743`) -> `0x01de` (`#5826`, skill 42, sdelay 440, ddelay 480, damage 46, lv 10, div 1, type 8). B gets `0x0b1a` + `0x01de` only. `0x0b1a`: element 0, delay 0. `0x0977` +457 ms after `0x01de` (hp 9 of 55), next hit hp 0 (`#5914`).
* **Miss** (`#6162` and `#6206`): `0x0b1a`, SP, **`0x00b1` still pays** (899000, 896000...), `0x01de` damage 0, ddelay 0, no `0x0977`. Zeny and SP are spent on a miss.
* **Not enough zeny** (zeny 0 after five casts, SP 30 so SP is fine): `#6056`, `#6071`, `#6097`, `#6107`: `0x0110` skill 42, **cause 5**, 3-8 ms, nothing else, **SP not consumed**. **Not enough SP wins**: with SP 4 and 4000 zeny `#5844` gets cause 1 (`#5847`). Spacing: casts 901-1166 ms apart were all accepted.

### 14.4 Heal (skill 28, Acolyte, amotion 440, no cast time)

* Request `0x0438` lv, skill 28, target (own AID for self).
* **Wire order on A** (`#6703`, self, HP 89/137): `0x0b1a` (`#6708`, src = A, dst = A, x = y = 0, **element 6 (Holy)**, delay 0, disposable 0, attackMT 0) -> `0x00b0` SP (`#6709`, 75 to 35, cost 40 = 13 + 3*9) -> **`0x0983`** (`#6710`) -> `0x09cb` (`#6711`, skill 28, **value 64**, target = src = A, result 1) -> `0x00b0` HP (`#6712`, **137** = clamped) -> `0x080e` (`#6713`). B: `0x0b1a` (`#6704`), `0x0983` (`#6705`), `0x09cb` (`#6706`, same 64), `0x080e` (`#6707`).
* **`0x0983`** (29 bytes): `efst u16=46, AID u32 = the caster, flag u8=1, total u32=500, remain u32=500, val1/2/3 = 0`. Same 22 frames everywhere, in front of `0x09cb`, to caster and observer, `AID` = the caster even when B is the target. 500 = the Heal after-cast delay (`skill_db.yml`), `display_status_timers` on.
* **Self at full HP** (`#6751`, `#6776`): `0x0b1a`, SP, `0x0983`, `0x09cb` **value 64** (the unclamped heal), and **no `0x00b0` HP, no `0x080e`**. B: `0x0b1a`, `0x0983`, `0x09cb`.
* **On B** (B at 22/40, `#6912`): A gets `0x0b1a` (`#6913`), SP (`#6914`), `0x0983` (`#6915`), `0x09cb` (`#6917`, target B), then `0x080e` for B's new HP (`#6918`, 40/40), **no `0x00b0` HP**. B gets `0x0b1a`, `0x0983`, `0x09cb` (`#6916`, `#6919-#6920`), **its own `0x00b0` HP = 40** (`#6921`) and `0x080e` (`#6922`). **B at full HP** (`#6990`): both get `0x09cb` 64, nobody gets an HP push (`#6991-#6997`).
* **On a Poring** (`#7495`, mob had been damaged by A; hp then clamped to its 55 max): `0x09cb` target = mob, value 64 to A and B (`#7511-#7512`), then **`0x0977` hp 55 max 55 to A only** (`#7513`, same millisecond; A is in the mob's damage log; B never hit it and gets none, as in `mob_heal`, `mob.cpp:3873-3893`). No HP bar for a non-attacker [I]. `0x0b1a` dst = mob, `0x0983` as usual.
* **Amounts** (renewal formula of section 7, BaseLv 13, INT 1, DEX 1, LUK 1, no weapon, no Meditatio): lv 10: `(13+1)/5 = 2`, `2*30 = 60`, `*10/10 = 60`, MATK = `1 + 0 + 0 + 0 + 13/4 = 4` -> **64** (captured 64). lv 1: `60*1/10 = 6` + 4 -> **10** (captured 10, `#7059`; SP 75 to 62, cost 13). MATK is deterministic without a weapon. Same result on self, on B, on the mob.
* **Spacing**: lv 1 spam `#7055-#7094` accepted at 592-665 ms between requests (`canact = max(T+500, T+440+100)` = 540 ms, never undercut). Heal below its SP cost (lv 10 at SP 36-37 against 40, lv 1 at SP 10 against 13): `0x0110` cause 1, 9 frames (`#6729`, `#6760`, `#6889`, `#7003`, `#7005`, `#7008`, `#7015`, `#7104`, `#7106`); the capture has 36 cause-1 frames in all (Bash 23, Heal 9, First Aid 3, Mammonite 1). The client does not pre-check SP: it sends the request anyway.
* **`/noshift`**: "without" (`#7325-#7430`): with Heal selected, clicking the Poring made the client send only `0x035f`/`0x0437` (attack, `#7193`, `#7213`; `0x0139` once), never `0x0438`. "With" (`#7431-#7503`): `0x035f` (`#7484`), then `0x0438` lv 10 on the mob 578 ms later (`#7495`). So the client only issues a support-skill `0x0438` on a monster when the `/noshift` option is on.

### 14.5 Cross-checks against sections 3-7

* Order confirmed: `0x0b1a` -> SP -> (zeny) -> (`0x0983`) -> `0x09cb`/`0x01de` -> HP push; cast-begin and castend both happen in the request's tick, the first reply is typically **3-6 ms** after `0x0438` (median 5, p75 40, p90 104, max 302 of 105 replies; the long ones are server-loop delay, not game delay).
* `0x0b1a` is always sent, `delay` always 0 for these four skills. Element: First Aid / Bash / Mammonite 0 (Neutral), Heal 6 (Holy).
* `0x0110` seen with causes **1** (SP) and **5** (zeny) only; layout `skill u16, btype i32 = 0, itemId u32 = 0, flag u8 = 0, cause u8`, 14 bytes. No cause 4 anywhere (confirmed silent).
* `0x09cb` is AREA (caster and observer get it), `0x00b0`/`0x00b1`/`0x0110` SELF only, `0x0977` only to damage-log players.
* No `0x043d` (no Cooldown) and no `0x0147` in the capture.

## 15. Capture: scripting_2.txt (supplement)

Source: [`captures/scripting_2.txt`](../captures/scripting_2.txt), rAthena `d4b8e7b`, `PACKETVER 20260219`, full payloads, recorded 2026-11-01 to answer the effects plan's open questions. `#N` = proxy number (file line = N + 10). `skilltest2` = AID 2000000, map session `#3` (Acolyte, `@blevel 99`, INT 99 and AGI 99 raised, DEX/LUK 1, amotion 270 at the end). `skilltest1` = AID 2000001, `#6`, then `#8` after a restart; `@job knight` at `#849`, `@blevel 99`, `@skillall`.

* **Zeny at map entry (step 1, `#402-#758`).** `@zeny 5000` -> `0x00b1` var 20 value 5000 (`#414`), then the message. Restart to char select (`0x00b2` type 1, `#461`): the char list `0x006b`/`0x0b72` carries the 5000 in `CHARACTER_INFO.money` (u32 **@12**, after GID u32 and exp i64; `#468`, `#477-#481`). The whole map-entry burst (`#485-#599`) has **no `0x00b1`**, and neither has the portal to `prontera` and back (`#636`, `#683`). The client learns zeny from the char list, then from `0x00b1` changes only. [C] (source agrees, section 10)
* **Wrong weapon (step 2).** Bash can't be tested with a bow (a Swordman can't equip one), so Pierce (56, lv 10, requires a spear) was used with a Katana: the client sends `0x0438`, and the server answers `0x0110` skill 56, cause **6**, SELF, 5-60 ms later (`#1053/#1054`, `#1057/#1058`, `#1060/#1061`). **No `0x0b1a`, no SP push.** Three requests 830-840 ms apart all got cause 6, so the failure sets no gate the client could hit. With a spear: the normal `0x0b1a` -> SP -> `0x01de` (div 2 vs a Medium Poring, `#1130-#1136`).
* **Wrong target (step 3).** Pierce on the other player:
  * With `/noshift` on, clicking the player sends nothing ("nothing happens", `#1344-#1371`).
  * Holding shift (and twice before the step marker): `0x0438` -> **`0x0b1a` AREA** (dst = the player) -> **`0x0110` cause 0** SELF, and **no SP push** (`#1279-#1282`, `#1296-#1299`, `#1393-#1396`, `#1398-#1401`). So the target check fails at castend, after cast begin; nothing is consumed and the gate from cast begin stays (section 6).
* **Heal spam (step 4, `#1524-#1712`).** Heal lv 10 then lv 1 on self (and once on `skilltest1`), hotkey mashed: requests arrive 593-640 ms apart and **all 19 are accepted**. The client paces Heal itself at ~600 ms (the user's note: "skill cooldown is preventing spamming"), so a request below the server's `canact` (`max(T+500, T+amotion+100)`) was never seen. Amounts: BaseLv 99, INT 99, DEX 1, LUK 1, no weapon: lv 10 = `198/5 = 39`, `*30 = 1170`, + MATK `99 + 49 + 0 + 0 + 24 = 172` = **1342** (`0x09cb` `#1530`); lv 1 = `1170/10 = 117` + 172 = **289** (`#1670`).
* **Element of `0x0b1a` (step 5).** Bash lv 10 with Fireblend (13435, a Fire weapon, `#1944-#1949`) and with a Fire Converter endow active (`#2407-#2413`, `#2498-#2503`): `0x0b1a` element **0** every time. Weapon/Endowed/Random skills send 0 (source: section 5 note). The damage itself does change with the element (Katana vs Willow 489/518, endowed 589/602), which metaling defers.
* **`0x0088` before a cast while walking.** A request whose target is already in range while the caster walks gets `0x0088` ZC_STOPMOVE (caster's gid and cell) to AREA right before `0x0b1a`, same millisecond: Heal on self `#1525/#1529`, First Aid `#2777/#2780`, Heal on a Zombie `#2960/#2961`, `#3056`. Out of range while walking is the step action instead (section 8, 14.2 step 4).
* **Heal on Undead (bonus, `#2724-#3159`).** `@monster zombie 3`, shift-click Heal lv 1 on each: `0x0088` (if walking) -> `0x0b1a` element 6 -> SP -> `0x0983` (efst 46, 500) -> **`0x01de`** skill 28, sdelay 270 (amotion), ddelay 288, **damage 287**, lv 1, div 1, type 8 (`#2967`, `#3060`, `#3135`) -> `0x0977` about 280 ms later (hp 0 of 204: each Heal killed one), exp. No `0x09cb`. One request (`#2913`) got no reply at all [I: likely out of range]. Metaling drops Heal on Undead silently (deviation; deferred with magic damage).
* **`itemskill` (Fire Converter, `#2108-#2387`).** `@item 13891` (Fire_Converter_Box). Using the box: `0x01c8` amount 0 -> `SP_WEIGHT` -> `0x0b41` (10× 12114) -> `SP_WEIGHT`, and the observer gets the `0x01c8` too. Using a converter (12114): `0x01c8` with the amount **unchanged (10)** (delay-consume) -> **`0x0147`** (skill 492 `ITEM_ENCHANTARMS`, lv 4) -> the client's own `0x0438` lv 4 skill 492 on itself 5 ms later -> `0x0b1a` element 3 -> `0x0983` efst 64, total 1,200,000, val1 3 -> **`0x07fa`** (reason 0, index 9, amount 1) -> `SP_WEIGHT` -> `0x09cb` skill 492 value 4. No SP push. Unequipping the weapon ends it with `0x0196` efst 64 (`#2600`).
* **Mob casts.** Mobs emote through a skill: `0x0b1a` src = dst = mob, skill 197, element 0, then `0x00c0` emotion (`#2444/#2445`).
