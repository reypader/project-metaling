# Player Auto-Attack (Normal Attack) Against a Mob

Target client version: `PACKETVER = 20260219`, Renewal. Citations are `$RATHENA_PATH/src/map/...` unless noted. Sections 1-7 are source-derived; **section 8** records what the real client and server did in [`combat_sequence_1.txt`](../captures/combat_sequence_1.txt) and **overrides** them where marked `CAPTURE`. Line numbers in section 8 are file lines of the capture (proxy `#` + 10). Damage formulas: [combat.md](./combat.md). Packet layouts: [../packets/combat.md](../packets/combat.md) and [`0x08c8`](../packets/map_units.md#0x08c8-zc_notify_act-aka-zc_notify_act3). Request frame `0x0437`: [../packets/map.md](../packets/map.md#0x0437-cz_request_act).

## Overview of the model (who drives what)

* The **client drives the chase**. rAthena never walks a player toward its target. When the target is out of range the server sends `ZC_ATTACK_FAILURE_FOR_DISTANCE` (`0x0139`) and the client walks (`CZ_REQUEST_MOVE`) and re-sends `CZ_REQUEST_ACT` once close (`unit.cpp:3262`, `clif.cpp:8174`). **CAPTURE ([section 8.3](#83-out-of-range-and-the-chase)):** for a far target the client walks first on its own and the server never sends `0x0139`; `0x0139` only answers a `0x0437` that arrives out of range or a loop swing that finds the target moved away. `unit_walktobl` is used only for monsters (`unit.cpp:3266`).
* The server keeps a per-unit attack timer (`ud.attacktimer`), a target (`ud.target`), `attackabletime` (next allowed swing), `canact_tick` (next allowed skill/attack-request) and `state.attack_continue`.
* Provenance for the whole file: `unit.cpp:2900-2980` (`unit_attack`), `3187-3365` (`unit_attack_timer_sub`/`unit_attack_timer`), `clif.cpp:11697-11830` (request parsing), `battle.cpp:7133+` (`battle_weapon_attack`).

## 1. Request: `CZ_REQUEST_ACT` (`0x0437`) action 0 / 7

> Open findings: [PF-13](../../../PACKET_FINDINGS.md#pf-13).

Frame: `0x0437`, 7 bytes, `target_id` at offset 2 (`u32`), `action` at offset 6 (`u8`) (`clif_packetdb.hpp`/`clif_shuffle.hpp:4749` `parseable_packet(0x0437,7,clif_parse_ActionRequest,2,6)` for `PACKETVER > 20180307`; applies to 20260219). Action ids (`clif.cpp:11808-11816`): `0` attack, `1` pick up, `2` sit, `3` stand, `7` continuous attack, `12` touch skill. `DMG_NORMAL = 0`, `DMG_REPEAT = 7` (`clif.hpp:691-699`).

Order inside `clif_parse_ActionRequest_sub` (`clif.cpp:11697-11745`):

1. Self dead: `clif_clearunit_area(self, CLR_DEAD)` (`0x0080` type 1) and return.
2. For actions other than 0/7 only (sit/stand/etc.): drop if `sc.cant.interact`, else `unit_stop_walking(USW_FIXPOS)`.
3. **Always** `unit_stop_attack(self)` (clears target and deletes the attack timer). So every new request first cancels the previous attack.
4. Negative `target_id` equal to `-self.id` (disguise) is mapped to self.
5. Action 0 / 7: `pc_cant_act` (talking to NPC, vending, trading, etc.) returns silently. If `skill_delay_attack_enable = 0` (default, `battle.cpp:8264`) and no Snap/Free Cast skill, and `tick < ud.canact_tick`, call `clif_skill_fail(sd, 1, USESKILL_FAIL_SKILLINTERVAL)` and return. **By default that sends no packet** (suppressed unless `@showdelay`; `display_skill_fail: 2`), so the refusal is silent; layout when enabled: [packets/combat.md `0x0110`](../packets/combat.md#0x0110-zc_ack_touseskill-skillattack-refusal-here-can-act-interval). Then `pc_delinvincibletimer` (login invincibility ends), idle-time reset, then `unit_attack(self, target_id, continuous = (action != 0))`.
6. Any other action byte falls to no case: only steps 2-3 happened.

### `unit_attack(src, target_id, continuous)` (`unit.cpp:2900-2980`)

| Step | Check / effect | Failure result |
| :--- | :--- | :--- |
| 1 | `map_id2bl(target_id)` must exist and not be dead (`status_isdead`) | `unit_unattackable` (clears continue flag, step flag, target); **no packet to client** |
| 2 | Target is a `BL_NPC` | `npc_click` instead (monster-style NPCs); no attack |
| 3 | `unit_can_attack`: no `PCBLOCK_ATTACK`, not on Warg rider, `sc.cant.attack` false | stop attack, keep walking |
| 4 | `battle_check_target(src, target, BCT_ENEMY) > 0` and `status_check_skilluse(src, target, 0, 0)` | `unit_unattackable` |
| 5 | Set `attack_continue = (continuous & 1)`, `step_attack = (continuous & 2)`, `ud.target = target_id` | - |
| 6 | If continuous: `ud.chaserange = status_get_range(src)` (weapon range) | - |
| 7 | If an attack timer already exists: just retarget and return (cannot happen from the client path, step 3 above cleared it) | - |
| 8 | If the player is mid-walk (`walktimer` active) **and** `!battle_check_range(src, target, range - 1)`: remember as `stepaction` (`target_to = target`) and return; executed at the next full step (see section 4) | - |
| 9 | `attackabletime > now`: `add_timer(attackabletime, unit_attack_timer)`. Else attack NOW: `clif_fixpos(src)` (`0x0088`) then call `unit_attack_timer(INVALID_TIMER, ...)` directly | - |

`status_check_skilluse` for a normal attack (`status.cpp:2040-2290`): src must have `MD_CANATTACK`, target not dead, target not in Trick Dead; a *player* source that is hidden (`OPTION_HIDE`) or chase-walking fails; vs a **mob** target, `OPTION_HIDE|OPTION_CLOAK|OPTION_CHASEWALK` on the target block the attack unless the attacker has `MD_DETECTOR` (a mob target is never "hidden" in normal play). `battle_check_target(..., BCT_ENEMY)` for player vs mob is true for any normal monster (guardian/emperium and ally-guild cases are out of scope).

## 2. Range check semantics

> Open findings: [PF-14](../../../PACKET_FINDINGS.md#pf-14).

* Range value: `status_get_range(src)` = `rhw.range` = **weapon range from the item (`Range` field of item_db) , minimum 1** (`status.cpp:4216`; bare hands = 1). Skills like Vulture's Eye add to it for bows (`status.cpp:4490-4512`).
* Inside the timer (`unit_attack_timer_sub`, `unit.cpp:3250-3270`): `range = rhw.range`, then **`range + 1`** if the target is currently walking (`unit_is_walking(target)`) or the attack came from a remembered step action (`state.step_attack`) and the target is not an ice-walled monster.
* For a **player** attacker the test is `check_distance_client_bl(src, target, range)` (`path.cpp:485-523`): `dx, dy` = coordinate differences, `d = floor(sqrt(dx*dx + dy*dy) - 0.1)` (never negative), in range iff `d <= range`. The "circular client" distance: with range 1, `(1,0)`, `(1,1)` AND straight `(2,0)` pass; `(2,1)`, `(2,2)` fail. With range 2: `(3,0)`, `(2,2)` pass. The source comment states straight lines are "one cell longer than expected". Note: unverified against a capture.
* For **monsters** the test is the square (Chebyshev) `check_distance_bl` : `max(|dx|,|dy|) <= range` (`CIRCULAR_AREA` is not defined, `config/core.hpp:39`).
* After the distance test, `battle_check_range` (`battle.cpp:8215-8240`) adds: same map; for a player the same client-distance test again; if `distance_bl < 2` it is OK without path check; if `distance_bl > AREA_SIZE (14)` fail; else a line-of-sight test `path_search_long(..., CELL_CHKWALL)` (no wall between the two cells); exact algorithm in [movement_aoi.md#5-line-of-sight-path_search_long](./movement_aoi.md#5-line-of-sight-path_search_long).
* **Out of range (player):** `clif_movetoattack(sd, target)` = `ZC_ATTACK_FAILURE_FOR_DISTANCE` `0x0139` to self; the function returns 1 **without re-arming the timer**, so the attack ends there. The target remains in `ud.target`/`chaserange` but nothing fires until the client sends a new request. Fields: [packets/combat.md](../packets/combat.md#0x0139-zc_attack_failure_for_distance).
* **In range but no line of sight:** if `attack_continue` and `chaserange > 1` then `chaserange = max(1, chaserange - 2)` and `unit_walktobl` (a player-side walk, only reachable when `OFFICIAL_WALKPATH` is off); otherwise attack gives up (timer returns 0 -> `unit_unattackable`).
* `ZC_ATTACK_RANGE` `0x013a` is a different thing: it tells the client the player's weapon range (`SP_ATTACKRANGE`). See [packets/combat.md](../packets/combat.md#0x013a-zc_attack_range).

## 3. The attack timer loop

`unit_attack_timer` (`unit.cpp:3354`) -> `unit_attack_timer_sub(src, tid, tick)` (`unit.cpp:3187`). Result 0 means "cannot continue" and calls `unit_unattackable` (clears target + continue flag). Order for a player source:

1. Clear `ud.attacktimer` first. (For monsters the timer is routed through the monster AI instead; out of scope.)
2. Abort (return 0), one `||` chain evaluated in this order (rAthena `unit.cpp:3219-3227`, `OFFICIAL_WALKPATH` defined by default at `config/core.hpp:26`): src/target null or not on map (`prev == nullptr`); `status_isdead(src)`; `status_isdead(target)`; `battle_check_target(BCT_ENEMY) <= 0`; `!status_check_skilluse(src, target, 0, 0)`; **`!path_search_long(nullptr, m, src.x, src.y, target.x, target.y, CELL_CHKWALL)`** (flag is **CHKWALL, not CHKNOPASS**: only raw-1 walls block, raw-5 cliffs do not; start and destination cells are not tested; no distance limit, no `d < 2` shortcut, so it runs even when adjacent but then there are no intermediate cells and it passes); `!unit_can_attack`. All failures `return 0` silently (no packet), and the caller `unit_attack_timer` then calls `unit_unattackable`. This LOS check comes **before** the cast check, `canact`, the client-distance/`0x0139` test and `battle_check_range`. The same timer function is used for the request path (`tid == INVALID_TIMER`, called from `unit_attack`), so it applies to the first swing too; `unit_attack` itself (section 1) has no path check. `CELL_CHKNOPASS` with `path_search_long` is only used for the walk-command `path_len > 14` check (`unit.cpp:268`, `864`, `3088`), not for attacks. Consequence: `battle_check_range`'s own `path_search_long(CELL_CHKWALL)` (step 5) is redundant for the `d > 1` case when `OFFICIAL_WALKPATH` is on.
3. Abort if currently casting (`ud.skilltimer` active) unless Free Cast.
4. `canact_tick > tick` and no Free Cast: direct request (tid = INVALID) -> skill-fail message, return 0; in a repeating timer with `attack_continue`, push `attackabletime = max(attackabletime, canact_tick)` and re-arm; return 1.
5. Range test (section 2). Out of range: send `0x0139`, return 1 with no timer.
6. If the call came from the request path (`tid == INVALID_TIMER`) and the attacker is a player: `clif_fixpos` (`0x0088`) again. Net effect for an immediate first attack: **two `0x0088` packets precede the damage packet** (one in `unit_attack`, one here). **CAPTURE: confirmed; three if the player's walk timer is still running (the third is `unit_stop_walking(USW_FIXPOS)`), see [8.2](#82-server-packet-sequences).**
7. If `attackabletime <= tick` (swing allowed):
   * `attack_direction_change` default is `0` (`battle.conf:128`) so the attacker does **not** turn toward the target on the server side.
   * If the walk timer is active: `unit_stop_walking(USW_FIXPOS)`.
   * `battle_weapon_attack(src, target, tick, 0)` (section 5). If it returns `ATK_NONE` (no ammo etc.) the loop ends here (return 1, no re-arm).
   * `unit_set_attackdelay(src, tick, DELAY_EVENT_ATTACK)` (section 3.1).
   * `attack_walk_delay` default is `0` (`battle.conf:33`): the attacker is **not** given a server-side walk lock during amotion (the client blocks movement itself).
8. If `attack_continue` and src alive: re-arm the timer at `ud.attackabletime`. A single attack (action 0) never re-arms: one swing only.
9. Return 1.

### 3.1 Delay variables set after each swing (player)

From `unit_set_attackdelay` (`unit.cpp:1869-1960`), `tick` = swing time, `amotion`/`adelay` from the attacker's status:

| Variable | Value | Meaning |
| :--- | :--- | :--- |
| `ud.attackabletime` | `max(old, tick + adelay)` | next time `battle_weapon_attack` may run; the repeat timer fires here |
| `ud.canact_tick` | `max(old, tick + amotion + pc_maxaspd/2)` | earliest next skill or fresh attack request; `pc_maxaspd/2` = 100 ms at default `max_aspd` 190 |

Because the player's `adelay = 2 * amotion` the swing interval is `adelay`, and a fresh request is only accepted after about `amotion + 100`. A re-requested attack with `tick < canact_tick` is rejected with the skill-interval failure message; a request in between `canact_tick` and `attackabletime` is deferred via `add_timer(attackabletime)` (step 9 of `unit_attack`).

### 3.2 ASPD, amotion, adelay, dmotion (player, Renewal)

Constants (`status.hpp:44-67`): `MIN_ASPD = 8000`, `AMOTION_DIVIDER_PC = 2`, `AMOTION_ZERO_ASPD = 2000`, `AMOTION_INTERVAL = 10`. `RENEWAL_ASPD` is defined (`config/renewal.hpp:68`).

* Base ASPD value (no buffs, no mount, no shield except as noted) (`status.cpp:2357-2397`): `aspd_base` = job table `BaseASPD[weapon]` from `db/re/job_aspd.yml` (default 2000; Novice Fist 40, 1hSword 57, Dagger 55). If a shield is worn add `BaseASPD[Shield]` (Novice 10). `temp = sqrt(dex*dex/5 + agi*agi*0.5) * 0.25 + 196` (for bow/instrument/whip/gun weapons `dex*dex/7` instead of `/5`). `ASPD = int(temp + (buff_bonus * agi / 200)) - min(aspd_base, 200)`; `buff_bonus` is 0 with no ASPD potions/skills. Example: Novice, Fist, AGI 1, DEX 1: `int(196.2) - 40 = 156`.
* `amotion = 2000 - ASPD*10` (`status.cpp:6410`), clamped to `[pc_maxaspd/2, 4000]`; `pc_maxaspd` = `(2000 - max_aspd*10)*2` with `max_aspd` default 190 (`player.conf:80`; third class 193) so the PC minimum amotion is **100 ms**. Example above: `amotion = 440`.
* `adelay = 2 * amotion` (`status.cpp:6417`): 880 ms in the example.
* `dmotion` (player) = `clamp(800 - agi*4, 400, 800)` (`status.cpp:4640-4645`), scaled by `player_damage_delay_rate` (default 100).
* Monsters: `amotion`, `adelay`, `dmotion` come directly from `mob_db.yml` `AttackMotion`, `AttackDelay`, `DamageMotion` (`mob.cpp:5370-5412`), e.g. Poring (1002): `AttackDelay 1872`, `AttackMotion 672`, `ClientAttackMotion 288`, `DamageMotion 480`. `monster_damage_delay_rate` default 100 (`monster.conf:127`).

## 4. Attack while walking (stepaction) and cancelling

* A walk request (`CZ_REQUEST_MOVE`, `unit_walktoxy`) calls `unit_stop_attack` (`unit.cpp:982`): the target is cleared and the attack timer deleted. Moving always cancels an attack in progress. The attack-stop paths are in the table below.
* If the client sends an attack request while the player is mid-step and the target is not within `range - 1`, `unit_attack` stores `stepaction`/`target_to` and returns without attacking (`unit.cpp:2958-2965`). On each completed walk step (`unit_walktoxy_timer`, `unit.cpp:723-733`) a `unit_step_timer` is armed for **half a step later** (diagonal: `speed*14/10/2`, straight: `speed/2`), which then calls `unit_attack(src, target, attack_continue + 2)` if the target is still visible (`unit.cpp:459-495`). The `+2` sets `step_attack` which gives `range + 1` in the timer.
* A newer action request deletes a previous pending stepaction. Exact timer, visibility check and clearing rules: [section 7.2](#72-step-action-attack-request-while-walking-exact).

| Event that stops the player's auto-attack | Mechanism |
| :--- | :--- |
| Target dies, leaves the map, or becomes untargetable (hidden, dead, not an enemy) | timer returns 0 -> `unit_unattackable`; no packet |
| Player sends `CZ_REQUEST_MOVE` | `unit_walktoxy` -> `unit_stop_attack` |
| Player sends `CZ_CANCEL_LOCKON` `0x0118` (2 bytes, not shuffled) | `clif_parse_StopAttack`: `unit_stop_attack` + `attack_continue = 0` (`clif.cpp:12579-12585`, `clif_packetdb.hpp:115`) |
| Another `CZ_REQUEST_ACT` (any action) | step 3 of request handling |
| Player dies, warps (`pc_setpos` stops attack, `pc.cpp:6257`), is stunned/silenced attack-blocking status | state-driven |
| Out of range at swing time | `0x0139` sent, loop ends (client must re-request) |
| Out of ammo / `ATK_NONE` | loop ends silently |

## 5. One swing (`battle_weapon_attack`)

Source: `battle.cpp:7133-7500`.

Normal player swing, no special statuses (everything below `sc`-specific is deferrable):

1. Bow/gun weapons need matching ammo in the ammo slot, else `ATK_NONE` (fail packet). Ammo is spent after the calc, before `clif_damage`. Melee: skip. Detail: [5.3](#53-ranged-normal-attacks-and-ammo).
2. Skills that intercept a normal swing (Auto Counter, Blade Stop, Triple Attack for Monks (`30%` Renewal), Sacrifice, Spell Fist, Poison React...) are all `sc`/skill dependent: defer.
3. `battle_calc_attack(BF_WEAPON, ...)` computes the result struct (miss, lucky dodge, damage, type, div, amotion, dmotion). See [combat.md](./combat.md). If `damage + damage2 < 1` the result is a **miss**: `dmg_lv = ATK_MISS`, `dmotion = 0`, `div = 1` for players (`battle.cpp:6765-6775`).
4. `clif_damage(src, target, tick, wd.amotion, wd.dmotion, damage, div, type, damage2, isspdamage)` is sent **at swing time** (`battle.cpp:7390`). The visible damage number appears immediately on clients.
5. `battle_delay_damage(tick, wd.amotion, ...)` (`battle.cpp:414-495`): **the real HP change is deferred by `amotion`** when `delay_battle_damage` is on (default `yes`, `battle.conf:153`). Caps: non-player attackers are capped to 1000 ms; if `amotion <= 1` damage is immediate. The deferred damage has exact skip rules, see [section 7.1](#71-delayed-damage-battle_delay_damage_sub-skip-rules).
6. When it fires: `battle_damage` (`battle.cpp:319`) computes the target walk delay, calls `status_fix_damage` -> `status_damage` which lowers HP, calls `mob_damage` for a mob target, and (if the mob survives) `unit_set_walkdelay(target, tick, delay, 0)`, otherwise `mob_dead`.

### 5.1 Mob flinch and death

* Mob "flinch" is client-side: the `0x08c8` packet carries `dmg_speed = dmotion` (mob_db `DamageMotion`) and the client plays the hurt animation for that long. Server side: `walkdelay = dmotion * damage_walk_delay_rate/100` (`battle.cpp:274-297`; `walk_delay_rate` 100 for non-PC, `pc_damage_walk_delay_rate` 20 for players; multi-hit adds `multihit_delay 200` per extra hit). With the default `damage_walk_delay: 0` (`battle.conf:40`) the legacy walk-lock is disabled, so `unit_set_walkdelay` just stops the monster on the nearest cell centre (`unit_stop_walking(USW_MOVE_FULL_CELL)`, `unit.cpp:2028`) and sets no `canmove_tick`.
* Damage does not stop an endure-ing target (`status_isendure`): no stop. Not relevant to normal mobs.
* Death: `status_damage` -> `mob_dead`; the mob is cleared with `ZC_NOTIFY_VANISH` type `CLR_DEAD` (1) **250 ms later** (`mob.cpp:3636`, `clif_clearunit_delayed(md, CLR_DEAD, tick+250)`). Player's next timer finds `target == nullptr`/dead and ends the loop.

### 5.2 Mob reaction (aggro on damage)

At the same moment as the HP change (`battle_damage`, `battle.cpp:345-362`), for a mob target that was hit (damage lv above block), a timer is added at **`tick + walkdelay`** (`= dmotion * 100%` for a mob) that calls `mob_attacked` -> `mob_set_attacked_id(attacker, mob)` (`mob.cpp:2257-2330`): `md.attacked_id = attacker.id` (pets/slaves resolve to master per `retaliate_to_master`), `md.state.aggressive = 0`, then the mob AI runs immediately and picks the attacker as target (subject to the mob's `Ai`/mode flags). So a Poring hit by a swing at `T` takes damage at `T + amotion` and starts retaliating at `T + amotion + 480`. A *missed* swing (`ATK_MISS/FLEE`) does not trigger this (`dmg_lv > ATK_BLOCK` check); a damaging hit always does, even passive mobs. Full AI (chase, mode `Ai: 02`, skills) is out of scope.

## 5.3 Ranged normal attacks and ammo

Source (rathena, Renewal, applies to 20260219): `battle.cpp:7163-7205` (swing-time check), `7382-7383` (consume), `2645-2670` (`battle_consume_ammo`), `2918-2945` (`is_skill_using_arrow`), `5314-5319` (BF_LONG), `3431-3447` (ammo ATK), `status.cpp:4042-4049` (ammo scan), `pc.cpp:6103-6125` (`pc_delitem`), `clif.cpp:4221-4237` (`0x013b`), `6008-6030` (`0x0110`). Packet fields: [packets/equipment.md](../packets/equipment.md). Items marked **(capture)** are recorded in packets/equipment.md from the user's proxy captures; everything else is source-only.

**Which weapons need ammo.** `state.arrow_atk` is recomputed at the top of every player swing: `weapon == W_BOW` or `W_REVOLVER(17)..W_GRENADE(21)` = revolver, rifle, gatling, shotgun, grenade launcher (`battle.cpp:7163`; enum `pc.hpp:965-982`). Every other type (instrument 13, whip 14, katar, huuma, knuckle, bare hands, melee) has `arrow_atk = 0`: no ammo check, no consumption, no ammo ATK.

**Swing-time check** (start of `battle_weapon_attack`, before any damage calculation; reached only when `attackabletime <= tick`, section 3 step 7):

| Condition | Packet to self | Result |
| :--- | :--- | :--- |
| ammo slot empty, bow | `0x013b` `ZC_ACTION_FAILURE` `type 0` (`ARROWFAIL_NO_AMMO`, client text "Please equip the proper ammunition first.") **(capture: `0x013b {00 00}`)** | `ATK_NONE` |
| ammo slot empty, any gun or grenade launcher (weapon in `(W_KATAR, W_HUUMA)` exclusive) | `0x0110` `ZC_ACK_TOUSESKILL`, skill 0, `flag 0`, `cause 84` (`USESKILL_FAIL_NEED_MORE_BULLET`, `clif.hpp:492`) **(capture: `combat_sequence_2.txt` #2138, after `0x0088` ×2)** | `ATK_NONE` |
| bow and ammo `SubType != Arrow` | `0x013b` type 0 | `ATK_NONE` |
| revolver/rifle/gatling/shotgun and ammo `SubType != Bullet` | `0x0110` cause 84 | `ATK_NONE` |
| grenade launcher and ammo `SubType != Bullet` (Renewal; pre-renewal `Grenade`) | `0x0110` cause 84 | `ATK_NONE` |

* `0x0110` is 14 bytes at this PACKETVER: `u16 id, u16 skill_id, i32 btype, u32 item_id, u8 flag, u8 cause` (`packets_struct.hpp:2448-2461`, `PACKETVER_MAIN_NUM >= 20181121`). Suppressed when `display_skill_fail & 1`; default `display_skill_fail = 2` (`conf/battle/skill.conf:173`) so it is sent. btype and item_id are 0. Not captured.
* Oddity: only `Bullet` passes the gun swing check, so a shotgun loaded with `Shell` subtype ammo cannot attack, although `pc_isequip` lets Shell be worn on a shotgun. Check the shipped item_db before relying on Shell.
* `ATK_NONE` ends the loop with no re-arm: `unit.cpp:3314-3321` returns 1 before `unit_set_attackdelay`, so no delay is set and there is **no `0x08c8`, no `0x0139`, no `unit_unattackable`** (`ud.target` stays). A fresh `0x0437` retries and fails the same way; only the failure packet is sent each time.
* Only the ammo item type is checked, not `amount` (an equipped stack always has `amount >= 1`).
* Arrows may stay worn with no bow (shield over a two-handed bow, [equipment.md](./equipment.md)); then `arrow_atk = 0` and no check runs.

**BF_LONG and the STR/DEX swap.**
* A normal attack is `BF_LONG` iff `is_skill_using_arrow` is true (`state.arrow_atk`, or `SC_SOULATTACK`), else `BF_SHORT` (`battle.cpp:5314-5319`). **Whips and instruments are `BF_SHORT`** (no ammo, no `arrow_atk`). For player vs ordinary mob BF_LONG only matters through cards/`bLongAtkRate`-style bonuses and mob `MD_IGNORERANGED` (`battle.cpp:2905`, not set on ordinary mobs): defer.
* The `str`<->`dex` swap applies to **bow, instrument, whip, revolver, rifle, gatling, shotgun, grenade** (confirmed): `status_base_atk` (`status.cpp:2436-2460`, flag set for exactly these eight) and the variance `base_stat_bonus` in `battle_calc_base_weapon_attack` (`battle.cpp:2452-2475`, `dex` for the same eight; `str` if the player has Soul Attack). It depends on the weapon type, not on ammo being loaded. ASPD's `dex*dex/7` (section 3.2) is the same set.

**Ammo ATK (Renewal).** Equipping ammo adds the item's `Attack` to `sd->bonus.arrow_atk` (`status.cpp:4045`; Arrow = 25, Bow = Range 5 / Attack 15 in `db/re/item_db_etc.yml` / `item_db_equip.yml`). In a normal attack it is part of **Equip ATK**, flat: `battle_calc_equip_attack` = `status.eatk + arrow_atk` when `is_skill_using_arrow` and the ammo is not a Cannonball (`battle.cpp:3431-3447`), then `equipAtk` gets the element fix with the right-hand element (`battle.cpp:3934-3938`; if the ammo script set `bonus.arrow_ele`, that element replaces the weapon element for a ranged right-hand attack, `battle.cpp:3476-3477`) and joins `percentAtk = (weaponAtk + equipAtk) * atk_rate / 100`. Ammo ATK is **not** in `rhw.atk`/`watk`, **not** in the variance roll (no ammo term in `battle_calc_base_weapon_attack`), **not** in `batk`, not randomised and not size-fixed. The random `rnd()%arrow_atk` path (`battle.cpp:2576-2578`, `BDMG_ARROW`) is pre-renewal only. See combat.md section 3.

**Consumption (`battle_consume_ammo(sd, 0, 0)`, `battle.cpp:2645-2670`).**
* Called once per swing at `battle.cpp:7382-7383` when `arrow_atk` is set: **after** `battle_calc_attack` and before `clif_damage` (`7400`). Not conditional on hit or miss, so a missed shot still spends ammo. Only target-side interceptions (Auto Counter, Poison React, Triple Attack returns at `7215-7290`) skip it; irrelevant to a normal mob.
* Quantity is 1 (skill id 0). Double Attack `div_` does not change it for a plain swing. Fear Breeze extra ammo is skill-only: defer.
* Gate: `arrow_decrement`, default `1` (`conf/battle/battle.conf:169`); 0 = never consume, 1 and 2 both consume for a normal attack.
* Call: `pc_delitem(sd, equip_index[EQI_AMMO], 1, type = 0, reason = 1, LOG_TYPE_CONSUME)` (`battle.cpp:2666`). Reason 1 is `ZC_DELETE_ITEM_FROM_BODY.deleteType` 1 ("skill"); type 0 sends `0x07fa` and `SP_WEIGHT` and recalcs on unequip. See [inventory.md](./inventory.md#4-pc_delitem).
* Packets, stack stays above 0: `0x07fa {index+2, 1, deleteType 1}` then `0x00b0` `SP_WEIGHT` (weight drops by the ammo's `Weight`). No `0x013c`, no equip packet, no status push. Source-only for the partial case.
* Packets, last item spent: `pc_unequipitem(n, 2|1)` first, then ammo `0x099a {index+2, 0x8000, 0}`, then `0x07fa {index+2, 1, 1}`, then `SP_WEIGHT`, no status push **(capture, packets/equipment.md "Arrows run out in combat")**. No `0x01d7`. The following swing then fails the no-ammo check.
* Packet order within a swing (source): ammo packets (`0x07fa`, `SP_WEIGHT`; plus `0x099a` first on the last item) are queued **before** the `0x08c8` of the same swing, since `clif_damage` runs after `battle_consume_ammo`. The consuming swing still deals its damage. Not verified against a capture (whether the relative order seen by the client matches).

**Range.** Bow range is the item `Range` (5 for Bow 1701), floored at 1 and raised to the other hand's range (`status.cpp:4216-4219`). Ammo adds none. Skill additions (Vulture's Eye `+lv` bow, Snake Eyes `+lv` gun, `status.cpp:4486-4498`) are out of scope. Range check and LOS as in section 2. For a normal attack ranged and melee differ only in the range value, `BF_LONG`, ammo check/consumption and the STR/DEX swap.

## 6. Defaults summary (battle config relevant to this flow)

| Config | Default | File | Effect |
| :--- | :--- | :--- | :--- |
| `delay_battle_damage` | yes | `battle.conf:153` | HP change delayed by amotion |
| `attack_walk_delay` | 0 | `battle.conf:33` | no server walk lock during attack |
| `damage_walk_delay` | 0 | `battle.conf:40` | legacy damage walk-lock off |
| `pc_damage_walk_delay_rate` / `damage_walk_delay_rate` | 20 / 100 | `battle.conf:45-46` | % of dmotion for the walk delay |
| `multihit_delay` | 200 | `battle.conf:55` | extra delay per extra hit |
| `attack_direction_change` | 0 | `battle.conf:128` | server does not turn attacker |
| `skill_delay_attack_enable` | 0 | `battle.cpp:8264` | attacks gated by `canact_tick` |
| `min_hitrate` / `max_hitrate` | 5 / 100 | `battle.conf:78-79` | hit chance clamp |
| `max_aspd` | 190 | `player.conf:80` | min amotion 100 ms |
| `synchronize_damage` | no | `battle.conf:161` | monster sdelay rewrite off |

## 7. Details settled in combat M0 (source only)

### 7.1 Delayed damage (`battle_delay_damage_sub`) skip rules

Source: `battle.cpp:384-412` (timer), `414-492` (scheduler); weapon swing call `battle.cpp:7437`. At 20260219 the weapon swing sends `wd.amotion` (attacker's amotion) as the delay.

* Scheduling (`battle_delay_damage`): `amotion` is replaced by `1` if `delay_battle_damage` is off; a non-PC attacker is capped at `1000`; `amotion <= 1` applies damage immediately with no timer and no checks. The timer stores **ids** (`src_id`, `target_id`) plus a snapshot `distance = distance_bl(src, target) + AREA_SIZE (14)` taken **at swing time** (`+10` instead if `snap_dodge`, default `no`, `battle.conf:195`). The PC attacker's `delayed_damage` counter is incremented (it holds back `status_calc_pc` until all pending hits resolve).
* At fire time both ids are re-resolved with `map_id2bl`. The hit is **applied** only if all hold: target exists and is **not dead**; **`src` still resolves (non-null)**; `target->m == src->m`; target is not a PC in login invincibility (irrelevant for mobs); `check_distance_bl(src, target, snapshot)` (square/Chebyshev `max(|dx|,|dy|) <= snapshot`, `path.cpp:436-447`, `CIRCULAR_AREA` undefined) at the **current** coordinates. Otherwise the damage is silently dropped (the `0x08c8` number was already shown; no correction packet).
* Not checked: the attacker being dead (a dead player still resolves while online, so the hit still lands), the attacker being sitting/stunned, the target having moved as long as it is within `snapshot` cells, and the attacker's `ud.target` (stopping the attack does not cancel pending hits; the timer is not deleted by `unit_stop_attack`). An attacker who **logged out** is no longer in the id table, so `src == nullptr` and the hit is dropped (the `CR_REFLECTSHIELD` fallback is the only `!src` path that still deals damage). An attacker who **warped** to another map fails `target->m == src->m`; one who warped within the same map beyond `dist_at_swing + 14` cells fails the distance test.
* Applied hit: `battle_damage(src, ...)` with the **current** `src` pointer and `tick` = timer tick (`battle.cpp:319-368`): `status_fix_damage` -> `status_damage` -> `mob_damage(md, src, dhp)` (`status.cpp:1656`). `mob_damage` writes the damage log (`mob_log_damage`, `mob.cpp:2672-2771`) keyed on `src`'s `char_id` at apply time, and then sends `0x0977` (`clif_monster_hp_bar`, `mob.cpp:2780-2805`) to **every damage-log member** whose `map_charid2sd` resolves and is within `AREA_SIZE` (Chebyshev, `check_distance_bl(md, sd, 14)`) of the mob **at apply time**. So `0x0977` and the damage log happen at `swing + amotion`, not at swing; the `0x08c8` number happens at swing. (`monster_hp_bars_info` and `PACKETVER >= 20120404` gate it.)

### 7.2 Step action (attack request while walking), exact

Source: `unit.cpp:432-496` (`unit_step_timer`), `700-745` (`unit_walktoxy_timer`), `2848-2870` (`unit_stop_stepaction`), `2874-2891` (`unit_unattackable`), `2955-2965` (`unit_attack`), `status.cpp:2292-2346` (`status_check_visibility`).

* **Armed** in `unit_attack` only if the attacker is a PC with an active `walktimer` and `!battle_check_range(src, target, range - 1)`: `stepaction = true`, `target_to = ud.target`, `stepskill_id = 0`; returns `USW_NONE` (keeps walking). Any earlier stepaction/steptimer is deleted first (`unit.cpp:2955-2957`).
* **Timer:** after each completed walk cell (`unit_walktoxy_timer`, before advancing `path_pos`), if `stepaction && target_to`: delete any old `steptimer`, then `add_timer(tick + d, unit_step_timer)` with `d = speed/2` (straight step) or `speed * MOVE_DIAGONAL_COST / MOVE_COST / 2` (diagonal, `unit.cpp:730-735`), `speed = status_get_speed(bl)` in ms per cell. So it fires half a cell later = the next full-cell moment. Only one step timer exists at a time; a new step re-arms it.
* **Fire** (`unit_step_timer`): requires `steptimer == tid`, `stepaction` true and `target_to != 0`; clears `steptimer`, `stepaction`, `target_to`. Then `tbl = map_id2bl(target_to)`; abort silently if null or `!status_check_visibility(src, tbl, false)`. That check = same map, **Chebyshev distance <= `AREA_SIZE` (14) for a player source** (view range), plus hidden/cloak/chasewalk/camouflage target states (a mob target is never hidden in normal play). Note: it is **not** the weapon-range check. Then `unit_attack(src, id, attack_continue + 2)`, which re-enters the normal path (it may re-arm a new stepaction if still not within `range - 1` and the player is still walking; `step_attack` makes the timer use `range + 1`).
* **Cleared by:** `unit_stop_stepaction` (flag, `target_to`, step skill fields, timer) from: a newer `unit_attack`/`unit_skilluse_*` (`unit.cpp:2342, 2678, 2956`), knockback (`unit.cpp:1319`), `unit_remove_map` i.e. warp/death (`unit.cpp:3605-3607`). A new **`CZ_REQUEST_MOVE`** (`unit_walktoxy(..., flag 4)`) calls `unit_unattackable` which zeroes `target_to`, clears `attack_continue`/`step_attack`/target (`unit.cpp:2874-2883`); the `stepaction` flag and an already-armed `steptimer` are left, but they become inert: `unit_walktoxy_timer` will not re-arm (needs `target_to`), and a pending `unit_step_timer` returns at the `target_to == 0` check. `CZ_CANCEL_LOCKON` (`0x0118`) only calls `unit_stop_attack` (target + attack timer) and does **not** touch `stepaction`/`target_to`, so a pending step action can still fire after a cancel (unverified edge; the client normally stops walking).

### 7.3 Wire effects of stopping an attack

* `unit_stop_attack` (`unit.cpp:2826-2842`) only clears `ud.target` and deletes the attack timer. **It sends no packet.** Likewise the walk (`unit_walktoxy`), `0x0118` (`clif_parse_StopAttack`), warp (`pc_setpos` -> `unit_stop_attack`, `pc.cpp:6257`) paths add no stop-attack packet; the client already knows because it initiated them. Pickup (`pc.cpp:11246`) and look change also call it silently. Out-of-range is the only attack-related notification (`0x0139`).
* `clif_fixpos` (`0x0088` `ZC_STOPMOVE`, 10 bytes: `u16 id, u32 aid, i16 x, i16 y`; `packets.hpp:728-734`) is sent with `AREA` (`clif.cpp:2205-2213`). `clif_send` `AREA` delivers to every PC in the 14-cell box **including the source itself** (only `AREA_WOS`/`AREA_WOC` variants exclude self, `clif.cpp:517-526`). So both `0x0088` calls in an immediate attack go to the attacker and observers. Position payload is the attacker's current `bl.x/bl.y`.

## 8. Capture: combat_sequence_1.txt (request, loop, range)

Source: [`../captures/combat_sequence_1.txt`](../captures/combat_sequence_1.txt), real client, rAthena, `PACKETVER 20260219`, 2026-10-30. Line numbers are **file lines** (proxy `#` + 10). Session `#3` = attacker A (account 2000000, Novice, `amotion` 590), `#6` = observer B (never attacks in this range). A fights Porings (mob ids `1100279xx`-`1100280xx`, `amotion` 672, `dmotion` 480). Ids below are A = 2000000. Everything here applies to `20260219`. Decoded by script from the hex payloads; mob positions come from the mob's `0x09fd`/`0x09ff`/`0x0088` and are approximate between packets. The user's chat markers were unreliable, so facts are derived from packets.

### 8.1 What the client sends

| Situation | Action byte | Follow-up | Lines |
| :--- | :--- | :--- | :--- |
| Plain click, `/noctrl` believed on (before the "disabling noctrl" marker, 1014) | `7` | nothing; loop runs on the server | 573, 663, 805, 933, 953 |
| Plain click after "disabling noctrl" (1014) | **`0`** (one swing, loop never starts) | nothing | 1044, 1378, 1424, 1674, 1759, 1788, 1822, 1918 |
| Held click (marker 1100) | `7`, **once** (no repeat for 3.45 s) | `0x0118` on release, 3453 ms later | 1105, 1122 |
| Ctrl click (markers 1154, 1234) | `7` | no `0x0118` | 1171, 1247, 1255, 1262 |
| Click on the target already being attacked / just attacked (marker "reclick", 1670, 1810) | `7` | **`0x0118` 73-108 ms later** (`CZ_CANCEL_LOCKON`, 2 bytes, no payload) | 1684/1688, 1691/1695, 1703/1707, 1714/1718, 1833/1838, 1845/1849 |
| Other `7` with no `0x0118` (cause unknown, probably ctrl/held) | `7` | loop runs to kill or `0x0139` | 1437, 1923 |

* **Held click does not make the client repeat `0x0437`.** The server's own loop produced the swings at 48.585, 49.775, 50.959 (lines 1112, 1115, 1118; interval = `adelay`); the client only sent `0x0118` on release (1122). Inferred: action `7` on mouse-down, `0x0118` on mouse-up.
* **The `7` + `0x0118` pair ends in exactly one swing.** The request swings immediately (8.2), then `0x0118` stops the loop: after 1687 (39.792) no swing follows at +1180 ms; the next swing is the next click (1694, 41.189). Same at 1706/1717/1836/1848. The server sends **no reply** to `0x0118` (1688-1690: only the delayed `0x00b0`/`0x080e`).
* The action byte therefore depends on client options and click type; a server must handle `0` (single), `7` (continuous) and a following `0x0118` for any target. **CORRECTS** the section 1 assumption "plain click sends 7".
* A ground click (`0x035f`) during an active continuous loop stops it: loop with swings through 616 (06.291); `0x035f` at 623 (07.165, no `0x0118`); the swing due at 07.471 never happens, the next A swing is 7 s later after a new request (669). Reply is only `0x0087` (625).
* Esc sent nothing (markers 1619/1643; no client packet between them except two ground-click `0x035f`, 1622 and 1631). Step 4 never produced a `0x0437` (the user never actually started an attack), so "click far mob then cancel" was not exercised beyond the above.

### 8.2 Server packet sequences

| Case | Server packets (to A and to observers in range, unless noted) | Lines |
| :--- | :--- | :--- |
| **Immediate first swing**, A standing | `0x0088`(A, A's cell) x2, then `0x08c8` (A -> mob), same millisecond, 3-8 ms after the request (outliers 45 and 57 ms: 1704, 1263) | 1684 -> 1685-1687; 1822 -> 1823-1825 |
| Immediate first swing, **A still walking** (step action fired mid-walk) | `0x0088` **x3**, then `0x08c8` | 805 -> 809-816; 953 -> 962-966; 1105 -> 1109-1112; 1171 -> 1175-1178 |
| First swing after a walk that has already ended before the step timer fires | `0x0088` x2 then `0x08c8` | 573 -> 576-581 |
| **Deferred** (request after `canact_tick`, before `attackabletime`) | **nothing at request time**; later a bare `0x08c8` with **no `0x0088`**, at exactly previous swing + `adelay`; loop continues | 1921 swing 49.052 (tick 154833266); 1923 request 50.077 (+1025 ms); 1924 `0x08c8` at 50.257 (tick 154834446 = +1180) |
| Deferred, **different target** | same; `adelay` is per attacker, not per target | 2958 (+1046 after swing 2943) -> 2960 at +1160 (`amotion` 580 there) |
| Each later loop swing | bare `0x08c8` (no `0x0088`) | 1112 -> 1115 -> 1118 (tick 154592799, 154593979, 154595159: +1180 each) |
| Hit | `0x08c8` `dmg_speed` = mob `dmotion` 480, damage > 0; then `0x0977` to **A only** at swing + 588-600 ms (= `amotion`) | 940 -> 942 (597 ms); 1049 -> 1050 (597 ms); 1677 -> 1679 (595 ms) |
| Miss | `0x08c8` `dmg_speed` 0, damage 0, `type` 0; **no `0x0977`** | 1115, 1687, 1762 |

* **`adelay = 2 * amotion` confirmed:** `src_speed` 590, swing interval 1180 ms by server tick (`0x08c8` tick field) every time (1112/1115/1118; 1921/1924/1926). Matches section 3.1 (`attackabletime = tick + adelay`).
* **`0x0977` delay = `amotion`** (588-600 ms by proxy time), i.e. the delayed HP change of section 5/7.1. A `0x0118` does not cancel a pending hit: the swing at 1836 (20:08.471) is followed by `0x0118` at 1838 (20:08.551) and its `0x0977` still arrives at 1839 (20:09.078).
* Observer B receives the `0x0088`s and `0x08c8` identically (e.g. 809-816) but **not** `0x0977`, `0x0139`, `0x00b0`.
* Request-to-first-swing when A is mid-walk (step action): 300 (573), 344 (805), 161 (933), 162 (953), 178 (1044), 157 (1105), 243 (1171), 112 (1674), 167 (1759) ms. Step diag cell = 210 ms, so this fits `fires at the next cell boundary + half a cell` (section 7.2): range about 105 .. 105 + one cell + latency. A stops where the timer fires, one or more cells short of its destination (805: dest (151,190), `0x0088` at (152,189) = after 6 of 7 diagonal cells, tick 154534009 - 154532638 = 1371 ms = 6.5 x 210 ms).
* `0x009c` never appears around a swing (`attack_direction_change 0`); the only `0x009c` in 540-1910 are the pickup direction changes (701, 724). No `0x0110`/`0x013b` anywhere in 540-1910.
* Mob retaliation (brief): mob `0x0088` (1 packet) + `0x08c8` (`src_speed` 672, `dmg_speed` = A's `dmotion` 796/792) appears about `dmotion` (480) after A's first hit lands, e.g. swing 581 at 01.555, HP change 01.555+590, mob attack 587-592 at 02.646 (= +1091 ms). Consistent with section 5.2.

### 8.3 Out of range and the chase

* **`0x0139` is sent.** Layout confirmed as [packets/combat.md](../packets/combat.md#0x0139-zc_attack_failure_for_distance) (14 payload bytes: target id, target x/y, my x/y, range 1). To A only.
  * Request path: `0x0088`(A) x1 then `0x0139`, 5 ms after the request. 1247 -> 1248 `0x0088`, 1249 `0x0139` (target (149,216), me (146,214)); 1255 -> 1256, 1257 (target (151,216), me (148,215)). One `0x0088`, not two: the range test comes before the second `fixpos` (section 3 step 5 vs 6, confirmed).
  * Loop path: no `0x0088`. The `0x0139` comes at the next `attackabletime` after the last swing when the target has moved away: 949 at 17.682 = swing 940 (16.500) + 1182 ms (target (149,192), me (150,189)); 1927 at 52.605 = swing 1926 (51.435) + 1170 ms. Loop ends, no more swings until a new `0x0437`.
* **Client reaction to `0x0139`:** `0x035f` 3-5 ms later (950 +3 ms, 1250 +3, 1258 +5, 1928 +14) to the cell **adjacent to the target on the approach side** (950: target (149,192) -> dest (149,191); 1250: (149,216) -> (148,215); 1258: (151,216) -> (150,216); 1928: (157,226) -> (158,225)). Then it re-sends `0x0437` (`7`): 166 ms later mid-walk (953), or after arriving (1262, 1167 ms after the `0x035f`). Successive requests while chasing were 1201 and 1209 ms apart (1247, 1255, 1262); 1928 got no re-request (the user clicked the ground).
* **Far target: the client walks first, no `0x0139`.** Pattern `0x035f` toward the mob, then `0x0437` **while still walking** once close: 570 -> 573 (+109 ms), 800 -> 805 (+1125 ms), 929 -> 933 (+241 ms), 1163 -> 1171 (+591 ms), 1035 -> 1044 (+925 ms, action 0), 1672 -> 1674 (+858 ms), 1787 -> 1788 (+99 ms). The server always accepted these as step actions (mid-walk, target not within `range - 1`) and swung at the next cell boundary (8.2); the swing came with A still short of the `0x035f` destination.
* **Near but out of range, A standing:** the client sends `0x0437` straight away and relies on `0x0139` (targets at (3,2) for 1247, A having just arrived, and (3,1) for 1255, A idle). So the client's own threshold for "walk first" is looser than the server's range 1; it is not known (no sample between 2 and 3 cells with a standing target).
* `0x035f` to the player's current cell is answered by a `0x0087` with src = dst (1254: (148,215)->(148,215)).

### 8.4 PF-14: range 1 at 2 cells

* **Settled later by `combat_sequence_2.txt`, see [8.6](#86-capture-2-combat_sequence_2txt-pf-14-gun-ammo-showdelay).** In this capture it was **not settled**. The straight and (2,1) attempts did not produce a clean sample, because Porings walk toward A and A's request is almost always sent mid-walk (step action, which uses `range + 1`):
  * "straight" (marker 1351): `0x0437` at 1378 while A was mid-walk (dest (141,219), `0x0087` 1370); the mob stood at (140,219) (its walk ended there, `0x09fd` 1354, next walk starts there at 1384). At the request A was about (142,219) = 2 cells straight, but A was walking, so this is a step action; the swing (1382) came 160 ms later with A stopped at (141,219) = **1 cell**.
  * "diagonal" (marker 1417): `0x0437` (action 0) at 1424 mid-walk (dest (153,228)); the mob reached A first and swung from (154,229) (1426-1428); A swung from (155,229) at 1429-1432: 1 cell.
  * One more hint: swing 940 (A at (150,189)) hit a Poring that was walking toward A, estimated at (150,191) = (0,2) straight; both step action and `unit_is_walking(target)` give `range + 1` there, so it does not test the plain rule either.
* **What is confirmed:** with range 1 a target at (3,2), (3,1), (1,3) is out of range (`0x0139`, 8.3), consistent with `floor(sqrt(dx^2+dy^2) - 0.1) = 3 > 1`. A's weapon range is 1 (`0x0139` range field = 1).
* **Still needed:** A standing still (no walk, no stepaction), target standing at exactly (2,0) and (2,1), `0x0437` sent. Expected by source: (2,0) swings, (2,1) gets `0x0139`.

### 8.5 Summary of corrections to sections 1-7

* Section 1 / PF-13: a plain click does **not** always send 7; see 8.1. `/noctrl` on: `7`; off: `0`, and `7` + `0x0118` on a click of the current target.
* Section 3 step 6 / 7.3: two `0x0088` confirmed; **three** when the walk timer is still active at swing time.
* Overview line / section 2: the chase is client-driven and starts with `0x035f` for far targets; `0x0139` only follows an out-of-range `0x0437` or loop swing.
* Section 3.1 / `canact_tick` refusal (`0x0110`): unexercised, see Open questions.
* Sections 5/7.1: `0x0977` at swing + `amotion` and survival of pending hits across `0x0118`: confirmed.

### 8.6 Capture 2: combat_sequence_2.txt (PF-14, gun ammo, `@showdelay`)

Source: [`../captures/combat_sequence_2.txt`](../captures/combat_sequence_2.txt), real client, rAthena `d4b8e7b8f`, `PACKETVER 20260219`, 2026-10-30. Proxy `#` (file line = `#` + 10). A = `#3` (`combattest1`), B = `#6` (`combattest2`, spawned the Pupas with `@monster pupa 10`). Pupas never move. No chat step markers; A's typed notes are at #1025 and #1465. Decoded by script from the hex payloads.

* **PF-14 settled.** A had the Knife (range 1 by `0x013a` #653, amotion 590) and stood still for more than 3 s before each request:

  | Request | A / Pupa | (dx,dy) | Client sent | Server |
  | :--- | :--- | :--- | :--- | :--- |
  | #774 | (153,173) / (153,175) | (0,2) | `0x0437` 7, no walk | `0x0088` ×2 at A's cell, `0x08c8` 46 (+5 ms) |
  | #899 | (157,173) / (157,175) | (0,2) | `0x0437` 7, no walk | the same (+52 ms) |
  | #819-824, #952-958 | 3 straight, (0,3) | (0,3) | `0x035f` to the adjacent cell, then `0x0437` 0 about 210 ms into the walk | swing on arrival, `0x0088` ×2 |
  | #1164-1178, #1244-1247 | (2,1) | (2,1) | `0x035f` to the adjacent cell, then `0x0437` 7 about 40 ms later | swing about 280 ms later, `0x0088` ×2 |
  | #1273-1275 | (2,2) | (2,2) | `0x035f`, then `0x0437` 0 | swing on arrival |

  So a range-1 player standing still hits at 2 cells straight with no move, as `check_distance_client` predicts. The **client** runs the same test: for (2,1), (2,2) and 3 straight it walks to an adjacent cell first and sends the request during the 1-cell walk. No `0x0139` was sent in the whole capture. Section 8.3's `0x0139` replies came from requests the client sent while the target was moving.
* **Request during a 1-cell walk:** the swing came when the walk ended, with `0x0088` ×2 (not ×3: the walk had already stopped). A ranged request during a long walk stopped A at once with `0x0088` ×3 (#1745, #2088), as in 8.2.
* **Gun with no bullets** (Gunslinger, Black Rose 13155, range 9 by `0x013a` #1992, amotion 940): after the Bullets came off (#2134: `0x00ab`, `0x099a` pos `0x8000`), three requests (#2138, #2162, #2211) each got `0x0088` ×2 then `0x0110` `10 01 | 00 00 | 00 00 00 00 | 00 00 00 00 | 00 | 54` (skill 0, flag 0, cause 84 `NEED_MORE_BULLET`). No `0x08c8`, no `0x013b`, nothing spent. B got the `0x0088`s only. The proxy has no `0x0110` row and misframes it (#2142-2153; PF-21). Compare a Bow with no arrows (#1558): `0x0088` ×2 then `0x013b {0,0}`. Section 5.3's gun row is now captured.
* **Gun shot with bullets** (#2088, A mid-walk): `0x0088` ×3, `0x07fa {idx 14, 1, reason 1}`, `0x00b0` weight 2623 → 2622, `0x08c8` 247 (`src_speed` 940). `0x0977` 0/51 at +943 ms (= amotion), then the kill (`0x0acb` ×2, `0x0acc` ×2, `0x0080`). B saw `0x0088` ×3 and `0x08c8`, not the ammo packets or `0x0977`. Same order as the bow in section 5.3. Only one bullet shot, so no run-out.
* **`@showdelay` (#1673-#1827):** no `0x0110`. The shortest swing-to-request gap in the capture was 1.84 s, so `canact_tick` was again never reached (see the open question below).
* **Confirmed again:** every action 7 was followed by `0x0118` 50-150 ms later and action 0 never was; a swing already due survives the `0x0118` (#1179 → #1183); a single swing (action 0) never repeats; `0x0977` lands at swing + amotion (598 ms Knife, 943 ms gun).

## Open questions / unverified

* Settled by `combat_sequence_1.txt` (section 8): packet order (`0x0088` x2 or x3 before `0x08c8`), action byte (0 or 7 depending on `/noctrl` and click type; **not** always 7), `0x0118` pairing, deferred swings, `0x0139` and the client chase.
* Still open: `target_id` the client sends for sit (not in this range).
* Settled by `combat_sequence_2.txt`: the `check_distance_client` straight-line +1 (PF-14). Range 1 hits at (2,0) with A standing; the client itself walks first for (2,1), (2,2) and (3,0). See [8.6](#86-capture-2-combat_sequence_2txt-pf-14-gun-ammo-showdelay).
* Still open: a re-request **inside** `canact_tick` (`amotion + 100 ms`): the shortest re-request seen was 1025 ms after a swing (proxy #1913; 60 requests measured), so `0x0110` and the silent refusal were never exercised. The user who recorded it observed that the client seems to debounce fast clicks on a monster (2026-10-30), so a stock client may never send one; the server-side drop then only guards against modified clients. Keep the source rule. `combat_sequence_2.txt` (with `@showdelay` on) also has no re-request inside the window: shortest gap 1.84 s.
* Still open: whether `0x0118` is what the client sends when it cancels with a ground click: it did not (8.5). The press/release meaning of the `7` + `0x0118` pair is inferred (8.1).
* rust-ro was not consulted for this topic.
