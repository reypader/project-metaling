# Player Auto-Attack (Normal Attack) Against a Mob

Target client version: `PACKETVER = 20260219`, Renewal. Citations are `$RATHENA_PATH/src/map/...` unless noted. Source-derived only: **no proxy capture of an attack exists** (`tools/proxy/packet_captures/` is absent in this checkout), so every timing and packet-order claim below is unverified against the real client. Damage formulas: [combat.md](./combat.md). Packet layouts: [../packets/combat.md](../packets/combat.md) and [`0x08c8`](../packets/map_units.md#0x08c8-zc_notify_act-aka-zc_notify_act3). Request frame `0x0437`: [../packets/map.md](../packets/map.md#0x0437-cz_request_act).

## Overview of the model (who drives what)

* The **client drives the chase**. rAthena never walks a player toward its target. When the target is out of range the server sends `ZC_ATTACK_FAILURE_FOR_DISTANCE` (`0x0139`) and the client walks (`CZ_REQUEST_MOVE`) and re-sends `CZ_REQUEST_ACT` once close (`unit.cpp:3262`, `clif.cpp:8174`). `unit_walktobl` is used only for monsters (`unit.cpp:3266`).
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
5. Action 0 / 7: `pc_cant_act` (talking to NPC, vending, trading, etc.) returns silently. If `skill_delay_attack_enable = 0` (default, `battle.cpp:8264`) and no Snap/Free Cast skill, and `tick < ud.canact_tick`, send skill-fail `USESKILL_FAIL_SKILLINTERVAL` and return. Then `pc_delinvincibletimer` (login invincibility ends), idle-time reset, then `unit_attack(self, target_id, continuous = (action != 0))`.
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
* After the distance test, `battle_check_range` (`battle.cpp:8215-8240`) adds: same map; for a player the same client-distance test again; if `distance_bl < 2` it is OK without path check; if `distance_bl > AREA_SIZE (14)` fail; else a line-of-sight test `path_search_long(..., CELL_CHKWALL)` (no wall between the two cells).
* **Out of range (player):** `clif_movetoattack(sd, target)` = `ZC_ATTACK_FAILURE_FOR_DISTANCE` `0x0139` to self; the function returns 1 **without re-arming the timer**, so the attack ends there. The target remains in `ud.target`/`chaserange` but nothing fires until the client sends a new request. Fields: [packets/combat.md](../packets/combat.md#0x0139-zc_attack_failure_for_distance).
* **In range but no line of sight:** if `attack_continue` and `chaserange > 1` then `chaserange = max(1, chaserange - 2)` and `unit_walktobl` (a player-side walk, only reachable when `OFFICIAL_WALKPATH` is off); otherwise attack gives up (timer returns 0 -> `unit_unattackable`).
* `ZC_ATTACK_RANGE` `0x013a` is a different thing: it tells the client the player's weapon range (`SP_ATTACKRANGE`). See [packets/combat.md](../packets/combat.md#0x013a-zc_attack_range).

## 3. The attack timer loop

`unit_attack_timer` (`unit.cpp:3354`) -> `unit_attack_timer_sub(src, tid, tick)` (`unit.cpp:3187`). Result 0 means "cannot continue" and calls `unit_unattackable` (clears target + continue flag). Order for a player source:

1. Clear `ud.attacktimer` first. (For monsters the timer is routed through the monster AI instead; out of scope.)
2. Abort (return 0) if: src or target not on map; src or target dead; `battle_check_target` not enemy; `status_check_skilluse` fails; `unit_can_attack` fails; (optional) no straight path when `OFFICIAL_WALKPATH` is defined.
3. Abort if currently casting (`ud.skilltimer` active) unless Free Cast.
4. `canact_tick > tick` and no Free Cast: direct request (tid = INVALID) -> skill-fail message, return 0; in a repeating timer with `attack_continue`, push `attackabletime = max(attackabletime, canact_tick)` and re-arm; return 1.
5. Range test (section 2). Out of range: send `0x0139`, return 1 with no timer.
6. If the call came from the request path (`tid == INVALID_TIMER`) and the attacker is a player: `clif_fixpos` (`0x0088`) again. Net effect for an immediate first attack: **two `0x0088` packets precede the damage packet** (one in `unit_attack`, one here). Unverified.
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
* A newer action request deletes a previous pending stepaction.

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

1. Bow/gun weapons need matching ammo in the ammo slot, else `ATK_NONE` (arrow fail packet). Melee: skip.
2. Skills that intercept a normal swing (Auto Counter, Blade Stop, Triple Attack for Monks (`30%` Renewal), Sacrifice, Spell Fist, Poison React...) are all `sc`/skill dependent: defer.
3. `battle_calc_attack(BF_WEAPON, ...)` computes the result struct (miss, lucky dodge, damage, type, div, amotion, dmotion). See [combat.md](./combat.md). If `damage + damage2 < 1` the result is a **miss**: `dmg_lv = ATK_MISS`, `dmotion = 0`, `div = 1` for players (`battle.cpp:6765-6775`).
4. `clif_damage(src, target, tick, wd.amotion, wd.dmotion, damage, div, type, damage2, isspdamage)` is sent **at swing time** (`battle.cpp:7390`). The visible damage number appears immediately on clients.
5. `battle_delay_damage(tick, wd.amotion, ...)` (`battle.cpp:414-495`): **the real HP change is deferred by `amotion`** when `delay_battle_damage` is on (default `yes`, `battle.conf:153`). Caps: non-player attackers are capped to 1000 ms; if `amotion <= 1` damage is immediate. The deferred damage is skipped if the target died meanwhile, changed map, or moved farther than `distance_bl(src, target) + AREA_SIZE (14)` from the attacker (`snap_dodge` off) at fire time (`battle.cpp:384-393`).
6. When it fires: `battle_damage` (`battle.cpp:319`) computes the target walk delay, calls `status_fix_damage` -> `status_damage` which lowers HP, calls `mob_damage` for a mob target, and (if the mob survives) `unit_set_walkdelay(target, tick, delay, 0)`, otherwise `mob_dead`.

### 5.1 Mob flinch and death

* Mob "flinch" is client-side: the `0x08c8` packet carries `dmg_speed = dmotion` (mob_db `DamageMotion`) and the client plays the hurt animation for that long. Server side: `walkdelay = dmotion * damage_walk_delay_rate/100` (`battle.cpp:274-297`; `walk_delay_rate` 100 for non-PC, `pc_damage_walk_delay_rate` 20 for players; multi-hit adds `multihit_delay 200` per extra hit). With the default `damage_walk_delay: 0` (`battle.conf:40`) the legacy walk-lock is disabled, so `unit_set_walkdelay` just stops the monster on the nearest cell centre (`unit_stop_walking(USW_MOVE_FULL_CELL)`, `unit.cpp:2028`) and sets no `canmove_tick`.
* Damage does not stop an endure-ing target (`status_isendure`): no stop. Not relevant to normal mobs.
* Death: `status_damage` -> `mob_dead`; the mob is cleared with `ZC_NOTIFY_VANISH` type `CLR_DEAD` (1) **250 ms later** (`mob.cpp:3636`, `clif_clearunit_delayed(md, CLR_DEAD, tick+250)`). Player's next timer finds `target == nullptr`/dead and ends the loop.

### 5.2 Mob reaction (aggro on damage)

At the same moment as the HP change (`battle_damage`, `battle.cpp:345-362`), for a mob target that was hit (damage lv above block), a timer is added at **`tick + walkdelay`** (`= dmotion * 100%` for a mob) that calls `mob_attacked` -> `mob_set_attacked_id(attacker, mob)` (`mob.cpp:2257-2330`): `md.attacked_id = attacker.id` (pets/slaves resolve to master per `retaliate_to_master`), `md.state.aggressive = 0`, then the mob AI runs immediately and picks the attacker as target (subject to the mob's `Ai`/mode flags). So a Poring hit by a swing at `T` takes damage at `T + amotion` and starts retaliating at `T + amotion + 480`. A *missed* swing (`ATK_MISS/FLEE`) does not trigger this (`dmg_lv > ATK_BLOCK` check); a damaging hit always does, even passive mobs. Full AI (chase, mode `Ai: 02`, skills) is out of scope.

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

## Open questions / unverified

* No capture: packet order (`0x0088` twice before `0x08c8`), whether the 20260219 client sends action `0` or `7` on a plain click (assumed `7`), and `target_id` for sit are unverified.
* Whether the real client accepts the `check_distance_client` straight-line +1 behaviour (range 1 hits at 2 cells straight) is taken from a source comment only.
* rust-ro was not consulted for this topic.
