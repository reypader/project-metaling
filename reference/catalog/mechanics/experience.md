# Experience Gain and Level-Up (base and job)

Target client version: `PACKETVER = 20260219`, renewal (`RENEWAL` and `RENEWAL_EXP` both defined, `src/config/renewal.hpp:24,52`). Paths are under `$RATHENA_PATH`. Tables and structures: [../db/exp_tables.md](../db/exp_tables.md). General `SP_*` var table and `ZC_PAR_CHANGE`: [../packets/map_units.md#0x00b0-zc_par_change](../packets/map_units.md#0x00b0-zc_par_change). Char columns: [../db/sql_schemas.md](../db/sql_schemas.md).

Source-derived from rAthena only. No packet capture of an exp gain exists in `tools/proxy/packet_captures/` (directory absent at time of writing); verify against a capture when one exists. rust-ro has no usable level-up logic (only a `gain_base_exp` test with a rate modifier in `server/src/tests/character_service_tests.rs:2503-2540`), so it was not used.

## Wire packets

> Open findings: [PF-09](../../../PACKET_FINDINGS.md#pf-09).

PACKETVER 20260219 is not in the `PACKETVER_RE` window (`config/packets.hpp:19-24`), so `PACKETVER_MAIN_NUM = 20260219` and every `>= 20170830/20170906` branch is taken: exp values are 64-bit on the wire.

| Opcode | Name | Dir | Len | Layout (offsets from frame start) |
| :--- | :--- | :--- | :--- | :--- |
| `0x0acc` | `ZC_NOTIFY_EXP2` | map->client | fixed 18 | `0 op.W, 2 account_id.L, 6 amount.Q (signed), 14 var_id.W, 16 exp_type.W` |
| `0x0acb` | `ZC_LONGLONGPAR_CHANGE` | map->client | fixed 12 | `0 op.W, 2 var_id.W, 4 amount.Q (signed)` |
| `0x00b0` | `ZC_PAR_CHANGE` | map->client | fixed 8 | `0 op.W, 2 var_id.W, 4 value.L` |
| `0x019b` | `ZC_NOTIFY_EFFECT` | map->client | fixed 10 | `0 op.W, 2 account_id.L, 6 effect_id.L` |

Sources: `clif.cpp:18977-19004` (`clif_displayexp`, `0x0acc` chosen by `PACKETVER >= 20170830`; length 18 from `clif_packetdb.hpp:1886`), `packets_struct.hpp:399-404` and `clif.cpp:3623-3634` (`0x0acb`; struct is packed, 12 bytes), `packets_struct.hpp:353-358` (`0x00b0`), `packets.hpp:1119-1125` (`0x019b`). The legacy 32-bit `0x07f6` (14 bytes) is not used for this packetver.

### 0x0acc ZC_NOTIFY_EXP2 (`clif_displayexp`)

* `account_id` is the player's own `bl.id` (account id). `amount` is `client_exp(exp)` times -1 when `lost` (`clif.cpp:69-71`: clamp to `MAX_EXP` = `INT64_MAX`, cast to int64).
* `var_id`: `1` = `SP_BASEEXP`, `2` = `SP_JOBEXP`. `exp_type`: `0` normal, `1` quest (client shows purple). For job exp `exp_type` is always forced to 0 (`(quest && type != SP_JOBEXP) ? 1 : 0`, `clif.cpp:18999`).
* Sent to SELF only (`WFIFOSET` on the player's fd), not to the area.
* When sent: end of every `pc_gainexp` (`pc.cpp:8527-8530`) for each of base/job where that exp was non-zero on entry (flag bits 1/2 computed before rate modifiers and the max-level clamp), AFTER the `ZC_LONGLONGPAR_CHANGE`/level-up updates. If the player is at max base (job) level the displayed amount is forced to 0 (`(flag&4) ? 0 : base_exp`). Also sent from `pc_lostexp` with `lost=true` (death penalty, script `set BaseExp` lowering it).
* Gain by script `set BaseExp, n` / `set JobExp, n` goes through `pc_gainexp(..., exp_flag=2)`, which skips guild tax, pvp check, `pc_calcexp` bonuses and `max_exp_gain_rate` (`pc.cpp:10468-10480`).

### 0x0acb ZC_LONGLONGPAR_CHANGE (exp vars)

Var ids (`enum _sp`, `map.hpp:498-529`, positional): `SP_BASEEXP = 1`, `SP_JOBEXP = 2`, `SP_NEXTBASEEXP = 22`, `SP_NEXTJOBEXP = 23`. `clif_updatestatus` (`clif.cpp:3737-3749`) sends these four as `0x0acb` with values `client_exp(status.base_exp)`, `client_exp(status.job_exp)`, `client_exp(pc_nextbaseexp)`, `client_exp(pc_nextjobexp)`. Other vars used by level-up stay `0x00b0` (32-bit): `SP_STATUSPOINT = 9`, `SP_BASELEVEL = 11`, `SP_SKILLPOINT = 12`, `SP_JOBLEVEL = 55`, `SP_TRAITPOINT = 231` (`clif.cpp:3649-3667, 3808-3809`). Full SP table: [packets/status.md](../packets/status.md#sp_-var-ids).

* Max-level behaviour: when at max level `pc_nextbaseexp` returns `MAX_LEVEL_BASE_EXP = 99999999` and `pc_nextjobexp` returns `MAX_LEVEL_JOB_EXP = 999999999` (`config/const.hpp:73-75`, `pc.cpp:8640-8676`), so NEXT* sent is that constant, and exp itself is clamped to it.
* First login (`connect_new`) sends, in order: `SP_BASEEXP`, `SP_NEXTBASEEXP`, `SP_JOBEXP`, `SP_NEXTJOBEXP`, `SP_SKILLPOINT` (`clif.cpp:10912-10916`); see [map_entry_actions.md](./map_entry_actions.md).

### 0x019b ZC_NOTIFY_EFFECT (level-up visual)

`clif_misceffect(bl, type)` (`clif.cpp:4367-4375`): `effect_id` 0 = base level up (`NOTIFYEFFECT_BASE_LEVEL_UP`), 1 = job level up (`NOTIFYEFFECT_JOB_LEVEL_UP`), 2 refine fail, 3 refine success (`clif.hpp:805-812`). Target is `AREA`: every player within 14 cells INCLUDING the leveler. `account_id` = leveler's id.

## pc_gainexp flow

Source: `pc.cpp:8452-8535`.

Signature inputs: player, source block (mob or null), base_exp, job_exp (unsigned 64-bit), `exp_flag` (bit 1 = quest exp, bit 2 = "param exp": script/direct set).

1. Return if the player is not on a map (`prev == nullptr`) or is dead.
2. If `!(exp_flag&2)`: return if `pvp_exp` is off and the map has the PVP flag; if in a guild, `base_exp -= guild_payexp(...)` (guild tax comes off BASE exp only, before everything else).
3. Record flags: bit1 base given (non-zero), bit2 job given, bit4 at max base level, bit8 at max job level. These drive the later `clif_displayexp` calls.
4. If `!(exp_flag&2)`: `pc_calcexp` personal bonuses (below).
5. `nextb`/`nextj` = exp needed for the next level (at max level the 99999999 / 999999999 constants).
6. Max-level clamp: if at max base level, new base exp is capped so `base_exp_total <= 99999999` (gain reduced to the remainder, 0 if already there); same for job with 999999999.
7. If `!(exp_flag&2)` and `max_exp_gain_rate != 0`: per-kill gain is capped to `max_exp_gain_rate/1000` of the current next-exp bar, separately for base and job (default 0 = disabled).
8. Base: `base_exp_total = min(base_exp_total + gain, MAX_EXP)` (saturating); run `pc_checkbaselevelup`; if no level-up happened send `0x0acb SP_BASEEXP` (a level-up sends it inside the level-up burst instead). Then the same for job with `pc_checkjoblevelup` and `SP_JOBEXP`. Base is processed first, so on a double level-up the base burst precedes the job burst.
9. `0x0acc` for base then job (see above). If `@showexp` is on, an additional chat line.

### Rate modifiers and where they apply

* `base_exp_rate`, `job_exp_rate`, `mvp_exp_rate` (all default 100, `conf/battle/exp.conf:13-16,54`) are baked into the MOB DB at load time: `mob.base_exp = exp * base_exp_rate / 100`, likewise job and MVP (`mob.cpp:5077-5095`). They are NOT applied in `pc_gainexp`. `quest_exp_rate` (100) is applied by the script commands.
* Per-kill share (`mob.cpp:3073-3220`, mob death handler): for each damage-log entry, `per = dmg / total_damage` (`exp_calc_type 0`; type 1 uses `dmg / mob max_hp`; type 2 = type 0 with the first attacker's damage counted twice), clamped to <= 1. With `count > 1` attackers (excluding rebirth tap): `count` capped at `exp_bonus_max_attacker` (12) and `per += per * (count-1) * exp_bonus_attacker(25) / 100`. Optional `mob_size_influence` (default off): medium /2, big x2. Pet damage scaled by `pet_attack_exp_rate`.
* Then `exp = mob_exp * per`, times `bonus/100` where `bonus` starts at 100 and adds: Rich Mankind (renewal: the attacker's own SC `val2`), Star Gladiator feeling/hate (`10*lv` or `20*lv` for the sun/moon/star mob, `SC_MIRACLE` forces the star case), and `mobs_level_up_exp_rate * levels` above the mob's DB level; times map flags `MF_BEXP`/`MF_JEXP` (percent). Result per component is clamped to `[1, MAX_EXP]` unless the mob DB exp is 0 or `MF_NOBASEEXP`/`MF_NOJOBEXP`.
* Renewal level-difference penalty (`RENEWAL_EXP`): `rate = pc_level_penalty_mod(player, PENALTY_EXP, mob)` reads `db/re/level_penalty.yml` (`pc.cpp:13325-13354`); with `diff = mob_level - player_base_level` (mob level is the spawned level, `md->level`). If `rate != 100`, base and job exp are each multiplied by `rate/100` and clamped to `[1, MAX_EXP]` (`mob.cpp:3209-3217`). Applied per solo killer, and per member after the even split in `party_exp_share` (`party.cpp:1275-1291`). Skipped when the map has `MF_NORENEWALEXPPENALTY`. Table and fill rules: [../db/exp_tables.md#renewal-level-difference-exp-penalty](../db/exp_tables.md#renewal-level-difference-exp-penalty). MVP bonus exp uses `PENALTY_MVP_EXP` (no such type in the yml, so always 100%) and gets the attacker-count bonus, then `pc_gainexp(mvp_sd, md, mvp_exp, 0, 0)` (`mob.cpp:3445-3465`).
* Party (`party.exp` share on): members' shares for one party are summed, then `party_exp_share` divides base, job, zeny by the count of eligible members (same map as the mob, alive, not idle if `idle_no_share`), optional `party_even_share_bonus` (default 0) multiplies by `100 + bonus*(n-1)` percent, then each member goes through the level-penalty and `pc_gainexp`. Homunculus gets `homunculus_exp_gain` (10) percent of the base share in renewal (`mob.cpp:3199-3200`).
* Personal bonuses `pc_calcexp` (`pc.cpp:8366-8429`, skipped for `exp_flag&2`): additive percent `bonus` = `bonus expaddrace/expaddclass` item bonuses (race, class, and "all" variants) + 15 in `pk_mode` if the mob is >= 20 levels above the player + `SC_EXPBOOST` val1 (Battle Manual, base and job-share: it is added to the base bonus, and the same running `bonus` carries into job) + `SC_PERIOD_PLUSEXP_2ND` val1; `SC_JEXPBOOST` val1 is added only for job. VIP (mob sources only) adds `vip_base_exp_increase`/`vip_job_exp_increase` (default 50 each, `conf/battle/player.conf:208-212`). Formula: `exp = exp + exp * (bonus + vip) / 100`, result clamped to `[1, MAX_EXP]` when the component is non-zero. The running `bonus` is reused for job, so `expadd*` and `SC_EXPBOOST` also raise job exp, `SC_JEXPBOOST` adds on top, and `SC_PERIOD_PLUSEXP_2ND` val1 is added a second time for job (counts twice there).

## Level-up

> Open findings: [PF-09](../../../PACKET_FINDINGS.md#pf-09).

Source: `pc_checkbaselevelup`, `pc.cpp:8244-8315`.

Preconditions: next-exp != 0, `base_exp >= next`, not already max base level; otherwise return 0 and nothing happens.

Loop per level gained:
1. `base_exp -= next`.
2. Carry-over cap: if `multi_level_up` is off (default `no`) OR (`multi_level_up_base > 0` and current base level >= `multi_level_up_base`), then `base_exp` is clamped to `next - 1` (leftover beyond one bar is discarded, so at most one level per kill under defaults; excess exp is truncated just below the next level). If `multi_level_up` is on, leftover carries and further level-ups can chain.
3. `status_point += statpoint(base_level -> base_level+1)` = `Points[lv+1] - Points[lv]` from `statpoint.yml` (0 if not increasing); `trait_point +=` the analogous trait-point delta (renewal 4th-job classes; table only non-zero from base level 201). Then `base_level++`.
4. If now at max base level: `base_exp = min(base_exp, 99999999)` and stop. Otherwise continue while `next(new level) > 0 && base_exp >= next`.

After the loop, in order (all to SELF unless noted): pet level update if `pet_lv_rate`; then the tail `SP_STATUSPOINT`, `SP_TRAITPOINT`, `SP_BASELEVEL` (`0x00b0`), `SP_BASEEXP`, `SP_NEXTBASEEXP` (`0x0acb`); then `status_calc_pc(SCO_FORCE)`; then `status_percent_heal(100, 100)`; Super Novice / Taekwon angel buffs; `0x019b` effect 0 (AREA); NPC event `NPCE_BASELVUP` (always, no config switch); `party_send_levelup`; `pc_baselevelchanged`; achievements. Exact wire effect of each step is in the next section.

### Natural base level-up: exact packet order

Source: `pc.cpp:8266-8302`, `clif.cpp:3637-3911` (`clif_updatestatus`), `status.cpp:6558-6700` (`status_calc_bl_` diff), `status.cpp:1803-1895, 1905-1973` (`status_heal`, `status_percent_change`), `pc.cpp:10663-10682` (`pc_heal`), `clif.cpp:3535-3546, 8095-8144`, `party.cpp:1013-1040, 1075-1078`. Source-derived, then CONFIRMED by a real-client capture ([Capture: combat_sequence_1.txt](#capture-combat_sequence_1txt)); applies to 20260219.

Per `pc_checkbaselevelup` (all level steps of one call are applied first, then ONE burst is sent, so a multi-level gain sends one set of packets with final values):

1. `SP_STATUSPOINT` `0x00b0` (var 9), then `SP_TRAITPOINT` `0x00b0` (var 231), then `SP_BASELEVEL` `0x00b0` (var 11). Right after `SP_BASELEVEL`, if the player is in a party, `clif_updatestatus` sends `0x0abd` `ZC_NOTIFY_MEMBERINFO_TO_GROUPM` (`u16 op, u32 aid, u16 job, u16 level`, 10 bytes) to the whole party INCLUDING self (target `PARTY`; `clif.cpp:3887-3901, 8133-8144`). Solo: nothing.
2. `SP_BASEEXP` `0x0acb` (var 1), `SP_NEXTBASEEXP` `0x0acb` (var 22). These carry the final values before the recalc runs.
3. `status_calc_pc(SCO_FORCE)`: recomputes everything, then sends one `clif_updatestatus` per CHANGED value (old battle status vs new), in this fixed order (renewal): STR, AGI, VIT, INT, DEX, LUK as `0x0141` (only if the total changed; base stats do not change on level-up), `HIT`, `FLEE1`, `ASPD`, `SPEED`, `ATK1` (if `batk` changed), `DEF1` then `DEF2` (if `def` changed), `ATK2`, `DEF2` then `DEF1` (if `def2` changed), `FLEE2`, `CRITICAL`, `MATK2` then `MATK1` (if either changed), `MDEF1` then `MDEF2`, `MDEF2` then `MDEF1`, `ATTACKRANGE`, `MAXHP`, `MAXSP`, `HP`, `SP`, then the trait block (POW..CRT `0x0141`, `PATK`, `SMATK`, `RES`, `MRES`, `HPLUS`, `CRATE`, `MAXAP`, `AP`) each only if changed (`status.cpp:6576-6690`). Level-dependent renewal values: `HIT` and `FLEE1` (+1 per level), `DEF2` (level/2 term), `MDEF2` and `MATK` (level/4 terms), `ATK1` (batk), `MAXHP`, `MAXSP` (`status.cpp:2600-2670`). `SP_HP`/`SP_SP` in this diff fire only if the current HP/SP itself changed during the recalc (clamp to a lower max), which a level-up does not do. `clif_skillinfoblock` (`0x010f`) is sent first inside the recalc only if the skill table changed; base level does not change it. The recalc happens BEFORE the heal, so `MAXHP`/`MAXSP` carry the new max while `HP`/`SP` are still old.
4. `status_percent_heal(100,100)` is `status_percent_change(nullptr, sd, -100, -100, -0, 0)` which calls `status_heal(sd, max_hp, max_sp, 0, 0)`; each amount is clamped to the missing amount and `pc_heal(flag 0)` runs, so it sends: `SP_HP` `0x00b0` (var 5, value = new max HP) only if HP was below the new max (this is the normal case, because max HP just grew), immediately followed by `clif_update_hp`, then `SP_SP` `0x00b0` (var 7) only if SP was below the new max. `clif_update_hp` (`clif.cpp:3539-3546`) sends `0x080e` `ZC_NOTIFY_HP_TO_GROUPM` (`aid, hp, maxhp`, `clif.cpp:8095`; 14 bytes) to the OTHER party members in the area (`PARTY_AREA_WOS`, self excluded) when the player is in a party (`party_hp_mode` 0 default), plus the HP-meter packet to GMs with `PC_PERM_VIEW_HPMETER` when the map has a viewer. In source a solo player with no GM nearby gets no `0x080e`, but the capture shows `0x080e` to the player and to non-party viewers after every `SP_HP` (see the capture section). If the player was already dead (HP 0) or both are already full, the heal sends nothing (can't be dead while killing; both-full happens only if max values did not grow).
5. Super Novice (`MAPID_SUPER_NOVICE`) and Taekwon (first class) only: `sc_start` of the "angel" status effects (status-change packets, 120 s / 600 s); not for other classes.
6. `0x019b` effect 0 (AREA, includes self).
7. `NPCE_BASELVUP` NPC event (script-side effects only).
8. `party_send_levelup` = `intif_party_changemap(sd, 1)`: a char-server round trip; when it returns, `party_recv_movemap` runs `clif_party_info` and sends the full party list to all online members (a few ms later, not in this burst's position).
9. `pc_baselevelchanged`: for each equipped item whose `elvmax` is below the new level, `pc_unequipitem(..., 3)` (unequip packets, then a recalc diff), then `pc_show_questinfo` (quest-info icon packets for the new level).
10. Achievement updates `0x0a24` (one pair of objectives per level gained; none if the player has no matching achievement).

**Difference from the captured `@blevel`/`@blvl` order** ([../packets/status.md](../packets/status.md#captured-stat-up-level-and-job-sequences)): the command runs `calc diff`, heal (`SP_HP`, `SP_SP`), `0x019b`, achievements, message, THEN the tail (`SP_STATUSPOINT`, `SP_TRAITPOINT`, `SP_BASELEVEL`, `SP_BASEEXP` = 0, `SP_NEXTBASEEXP`), then `pc_baselevelchanged`. The natural path sends the tail FIRST, then the diff, heal and `0x019b`. It also differs by: no message (`msg 21`), `SP_BASEEXP` carries the leftover exp (not 0), `0x0abd` right after `SP_BASELEVEL`, angel buffs for Super Novice/Taekwon, `NPCE_BASELVUP` unconditional, `party_send_levelup` (delayed party info), the equip check after (not before) the achievements, and `0x0acc` follows at the end of `pc_gainexp`. The diff and heal packets themselves have the same shape (same fixed order, same changed-only rule). The command's `status_calc_pc` runs with the OLD client level, the natural one with the new level already sent, but both compute from the server's new `base_level`.

### Natural job level-up: exact packet order

Source: `pc.cpp:8328-8361`. Per call (one burst, final values):
1. `SP_JOBLEVEL` `0x00b0` (var 55), `SP_JOBEXP` `0x0acb` (var 2), `SP_NEXTJOBEXP` `0x0acb` (var 23), `SP_SKILLPOINT` `0x00b0` (var 12).
2. `status_calc_pc(SCO_FORCE)`: same fixed diff order as above, changed values only. A job level-up changes the job-bonus stats at some levels (`0x0141` for the stats whose total changed, trait block likewise). No heal follows, so there is no `SP_HP`/`SP_SP` unless a clamp happened. Often nothing is sent.
3. `0x019b` effect 1 (AREA, includes self).
4. Star Emperor / max job level with `SG_DEVIL`: permanent blind status-change packet (`EFST_DEVIL1`); `NPCE_JOBLVUP`; achievements `0x0a24`; `pc_show_questinfo`. No party notify, no `pc_baselevelchanged`.

Difference from the captured `@jlevel` order: the command sends `0x019b`, achievements, message, THEN `SP_JOBLEVEL`, `SP_JOBEXP`, `SP_NEXTJOBEXP`, `SP_SKILLPOINT`, then the diff. The natural path sends the four pushes first, then the diff, then `0x019b` (the `@jlevel` capture has the effect before the pushes).

### Same-kill base and job level-up (full order)

`pc_gainexp` (`pc.cpp:8511-8530`) runs base first and then job, and each is one complete burst:
1. Base burst, steps 1 to 10 of the base section (tail, `0x0abd` if partied, `SP_BASEEXP`/`SP_NEXTBASEEXP`, diff, `SP_HP`, [`0x080e`], `SP_SP`, `0x019b` effect 0, ...).
2. Job burst: `SP_JOBLEVEL`, `SP_JOBEXP`, `SP_NEXTJOBEXP`, `SP_SKILLPOINT`, diff (usually empty after the base burst's recalc, apart from job-bonus changes), `0x019b` effect 1.
3. `0x0acc` `ZC_NOTIFY_EXP2` base (var 1, amount = the gain after bonuses), then `0x0acc` job (var 2), each only if that gain was non-zero on entry (and showing 0 at max level). They come AFTER both bursts, last in the call.
4. If `@showexp` is on, the `pc_gainexp_disp` chat line is after that.

A kill with no level-up at all sends, in order: `SP_BASEEXP` `0x0acb`, `SP_JOBEXP` `0x0acb` (each only if that exp was gained, so a zero-exp side is skipped entirely), then the two `0x0acc`. A base-only level-up: base burst, then `SP_JOBEXP`, then both `0x0acc`. A job-only level-up: `SP_BASEEXP`, job burst, `0x0acc` x2. The kill's own packets (damage, mob vanish, drops) come before `pc_gainexp` is called from the mob death handler.

### Job level-up

Source: `pc_checkjoblevelup`, `pc.cpp:8317-8364`.

Same loop with `job_exp`/`multi_level_up_job`; per level: `job_level++` and `skill_point += 1` (skill points are 1 per job level, no table). At max job level `job_exp = min(job_exp, 999999999)`. Packets: `SP_JOBLEVEL`, `SP_JOBEXP`, `SP_NEXTJOBEXP`, `SP_SKILLPOINT`; `status_calc_pc(SCO_FORCE)`; `0x019b` effect 1 (AREA). No heal on job level-up. Job exp caps differ by class group (Novice max job level 10, 1st classes 50, etc.); see exp_tables.md. When the player has already reached the class's max job level, job exp accrues only up to 999999999 and displays 0; there is no separate "novice cap" code, it is purely the `MaxJobLevel` of the job group.

### Direct set (script/GM)

`setparam` of `BaseLevel` (`pc.cpp:10412-10440`): caps to max base level, adds the status points for each skipped level, sets `base_exp = 0`, sends `NEXTBASEEXP`, `STATUSPOINT`, `TRAITPOINT`, `BASEEXP`, recalcs. GM `@baselevelup`/`@joblevelup` (relative change, resets exp, different point/heal rules): [atcommands.md#baselevelup--joblevelup](./atcommands.md#baselevelup--joblevelup). `BaseExp`/`JobExp` set uses `pc_gainexp(exp_flag=2)` or `pc_lostexp` as described above.

## Death exp penalty

Source: `pc.cpp:10005-10060`, `exp.conf:80-96`.

Applies unless the class is Novice (`MAPID_SECONDMASK == NOVICE`), the player has `cant.deathpenalty`, or the map has `MF_NOEXPPENALTY`/GvG. `death_penalty_type`: 0 none, 1 (default) lose `penalty/10000` of the NEXT-level bar (`death_penalty_base`/`_job` default 100 = 1%), 2 lose that fraction of CURRENT exp. Doubled in `pk_mode` when killed by a player. Clamped to current exp (cannot de-level). At max level the penalty is skipped unless `death_penalty_maxlv` bit 1 (base) / bit 2 (job) is set (default 0). A Token of Siegfried / `ITEMID_NEW_INSURANCE` item in inventory is consumed instead of losing exp. VIP uses `vip_exp_penalty_base/_job`. Applied via `pc_lostexp`, which sends `0x0acb` for the changed exp var then `0x0acc` with a negative amount (`pc.cpp:8540-8562`).

## Persistence

`char` columns (`sql-files/main.sql`, see [../db/sql_schemas.md](../db/sql_schemas.md)): `base_level`, `job_level` (smallint unsigned), `base_exp`, `job_exp` (bigint unsigned), `status_point`, `skill_point`, `trait_point` (int unsigned). Level-up and exp gain do not trigger a save: `pc_gainexp`, `pc_checkbaselevelup`, `pc_checkjoblevelup` call nothing in `chrif_save` and set no dirty flag (`pc.cpp:8244-8364, 8452-8535`). The values are written by `chrif_save` (`chrif.cpp:277`, which first runs `pc_makesavestatus`) only when:

* the autosave timer fires: `pc_autosave` (`pc.cpp:13002-13040`) saves ONE player per tick (`CSAVE_INVENTORY|CSAVE_CART`), with the tick interval `autosave_time / (online + 1)` floored at `minsave_time` (`conf/map_athena.conf:91-96`: 300 s and 100 ms), so each player is saved about every `autosave_time` seconds (a 300 s cycle);
* the player quits or changes map server (`map.cpp:2329` `map_quit`, `CSAVE_QUIT`; `chrif.cpp:520, 752`);
* a `save_settings` event occurs (`conf/map_athena.conf:114` default 4095 = all: after trade, vending, storage close, pet, mail, auction, quest, bank, attendance; `CSAVE_NORMAL`);
* a job change (`pc_jobchange`, `pc.cpp:11076`, `CSAVE_NORMAL`) and a few GM/autotrade paths (`atcommand.cpp:967, 6861`).

Warps inside the same map-server, level-ups and kills do not save. A crash can lose up to one autosave interval of exp and levels. A new server should either mark the character dirty on level-up and save soon (a deliberate improvement) or follow the autosave plus logout model above.

## Mob exp at load and spawn level

Source: `mob.cpp:5072-5096` (`MobDatabase` loader), `mob.cpp:4940-4942`, `mob.cpp:3102-3103`, `mob.cpp:476-479`, `npc.cpp:5234, 5301-5315`, `status.cpp:2781-2793`, `pc.cpp:13325-13354`, `conf/battle/monster.conf:247-250`, `conf/battle/exp.conf:13-16`. Applies to 20260219.

* `BaseExp`, `JobExp`, `MvpExp` from `mob_db.yml` are stored once at load as `min(max(exp * rate / 100.0, 0), INT64_MAX)` with `base_exp_rate`, `job_exp_rate`, `mvp_exp_rate` (all default 100, so the stored value equals the file value). Without the key the value is 0 (zero-initialised). No other load-time exp adjustment exists: no level scaling, no cap, no MVP-specific change. (A player-clone mob created by `mob_clone_spawn` gets base and job exp 1, `mob.cpp:4624-4625`.)
* A mob with 0 base AND 0 job exp and no spawn is flagged at load (`mob.cpp:273`, a warning only).
* Spawn-line level (`mob_name,<level>` in a `monster` script line, `npc.cpp:5234`): valid `1..275` (0 or above 275 is an error, `npc.cpp:5301`), stored as `spawn_data.level` (`npc.cpp:5314`), copied to `md->level` (`mob.cpp:478`). On spawn `status_calc_mob_` keeps `md->level` if it is 1..275 and differs from the DB level, otherwise resets it to the DB `Level` (`status.cpp:2782-2786`).
* Effects of a custom level on exp: the DB exp VALUE does not change. Only (a) the renewal level-penalty difference uses `md->level`, not the DB level (`pc.cpp:13325-13354`: `diff = md->level - player base level`), and (b) when `mobs_level_up` is on (default `no`) the kill bonus gets `+ (md->level - db_level) * mobs_level_up_exp_rate` percent (default rate 1; `mob.cpp:3102-3103`) and the mob's base stats are recomputed for the higher level. With `mobs_level_up` off, a spawn-line level changes only the penalty difference (and level-keyed drop/zeny formulas that read `md->level`).

## Capture: combat_sequence_1.txt

Source: [../captures/combat_sequence_1.txt](../captures/combat_sequence_1.txt) (rAthena `d4b8e7b`, `PACKETVER 20260219`, real client; line numbers are FILE lines = proxy `#` + 10). Session `#3` = A (`combattest1`, fresh Novice, later `@job 3` Archer), `#6` = B (`combattest2`, fresh Novice, observer; attacker only in step 8). Not partied, so no `0x0abd`/party-info packets. Settles [PF-09](../../../PACKET_FINDINGS.md#pf-09) for solo play. Mob ids come from the spawn packets (`0x09fd/9fe/9ff`, `job` at payload byte 19): Poring 1002, Lunatic 1063, Pupa 1008, Marine Sphere 1142. Applies to 20260219.

### Confirmed facts (summary)

* Natural base level-up order is TAIL FIRST, then recalc diff, then heal, then `0x019b` effect 0, exactly as the source reading in "Natural base level-up: exact packet order". The natural job level-up is: four pushes, diff, `0x019b` effect 1. Double level-up = base burst, job burst (each with its own diff, so two diffs; the job one is often empty), then both `0x0acc`. All other predictions in those sections held.
* `0x0acc` is the LAST exp packet of a kill (after every level-up burst); `0x0acb` BASEEXP/JOBEXP precede it. `0x0a24` achievement packets (kill-count objectives of the mob, nothing to do with levels) come after `0x0acc` and before the drops; absent for Pupa/Lunatic/Marine Sphere kills.
* `multi_level_up` is off: a gain larger than the bar levels once and the leftover is clamped to (consumed bar - 1) (see the job table below).
* Amounts equal mob_db exp times the renewal level-penalty rate, floored at each step (formulas below).
* `0x0977` (mob HP, `u32 aid, u32 hp, u32 maxhp`, 12 bytes) with `hp = 0` is the first packet of the exp burst, but only for players in the mob's damage log (see "What the observer gets").
* NOT contradicting but refining: the heal's `SP_HP` push is followed by `0x080e` (`aid, hp, maxhp`) that A itself receives although A is in no party (e.g. 1192, 2183); B receives A's `0x080e` for every A HP change as a non-party observer (e.g. 645, 3279). The earlier statement "a solo player gets no `0x080e`" (Natural base level-up step 4) is wrong in practice: the map-wide HP bar (`hpmeter_visible` path) is on. Same as [../packets/status.md](../packets/status.md#captured-stat-up-level-and-job-sequences) notes.

### 1. Kill without level-up (A, file 1285-1296, Poring 150/40)

| Line | Packet | Value |
| ---: | :--- | :--- |
| 1285 | `0x0977` MONHP | mob aid 110028067, hp 0, max 55 (kill tick, `.528`) |
| 1286 | `0x0acb` BASEEXP (var 1) | 202 (running total) |
| 1287 | `0x0acb` JOBEXP (var 2) | 67 |
| 1288 | `0x0acc` | base, amount 150 (`aid=A, i64 150, var 1, exp_type 0`) |
| 1289 | `0x0acc` | job, amount 40 (var 2) |
| 1290 | `0x0a24` | achievement count update (Poring kills), noise |
| 1291-1295 | `0x0add` x5 | drops, same ms |
| 1296 | `0x0080` | mob vanish type 1, +251 ms (`.779`) |

No `0x00b0`, no `0x019b`. A one-hit kill (2769-2774: `0x08c8` 104 dmg, then `0x0977` hp 0, `0x0acb` x2, `0x0acc` x2, `0x0add`, vanish +250 ms) has the same shape; with no earlier hits there is no preceding `0x0977`. Damage packets are sent at swing time; the HP is applied about 587 ms later (B's hits at 3096/3107/3128 produce `0x0977` 37/22/0 at +592/+587/+587 ms), so the kill tick is ~0.59 s after the killing blow's `0x08c8`.

**What the observer (B, not in the damage log) gets** (686-708): only `0x08c8` swings, the drop `0x0add`, `0x0080` vanish (+250 ms) and, on a level-up, one `0x019b` (aid = A, effect 0/1 per level kind, 683/864/982 etc.). No `0x0977`, `0x0acb`, `0x0acc`, `0x00b0`, `0x0141`. A sees B's level-up the same way (`0x019b` aid 2000001, 3133). Every player in the damage log (A and B in step 8) gets `0x0977` hp 0 (3130 B, 3131 A) followed by their own exp burst; mid-fight `0x0977` (hp 37, 22) went only to the attacker whose hit applied (B).

### 2. Level-up bursts, packet by packet (A)

Natural JOB-only level-up (kill 1, Poring at job 1; file 686-700). All captured job-only ups share this shape; the diff content varies with the Novice job bonus:

| Line | Packet | Value |
| ---: | :--- | :--- |
| 686 | `0x0977` | hp 0 |
| 687 | `0x0acb` BASEEXP | 150 |
| 688 | `0x00b0` JOBLEVEL (var 55) | 2 |
| 689 | `0x0acb` JOBEXP | 9 (40 - 10 = 30, clamped to 10 - 1) |
| 690 | `0x0acb` NEXTJOBEXP (var 23) | 18 |
| 691 | `0x00b0` SKILLPOINT (var 12) | 1 |
| 692-695 | diff | `0x0141` LUK `base 1, bonus 1`; `0x00b0` ATK1=2, FLEE2=1 (var 51), CRIT=1 (var 52) |
| 696 | `0x019b` | aid A, effect 1 |
| 697-698 | `0x0acc` x2 | 150 base, 40 job |
| 699-700 | `0x0a24` x2 | achievements |
| 701+ | `0x0add` | drops |

No heal, no `0x080e`, no `0x0abd`. Job-bonus diffs seen: job 2 LUK (+ATK1, FLEE2, CRIT), job 3 DEX (`0x0141` then HIT; 857-858), job 4 none (987-991), job 5 AGI (+FLEE1; 1199-1200), job 6 VIT (+DEF2=10, DEF1=2; 1455-1457), job 7 none (2186-2190), job 8 STR (+ATK1; 2825-2826), job 9 INT (`0x0141`, MATK2, MDEF2, MDEF1; 3146-3152). Job-level skill points: SKILLPOINT is the new TOTAL (1, 2, 3, 4, 5, 6, 7, 8 across job 2-9).

Natural BASE + JOB double level-up, base 1 to 2 and job 4 to 5 (Poring at 600/67 total; file 1179-1204):

| Line | Packet | Value |
| ---: | :--- | :--- |
| 1179 | `0x0977` | hp 0 |
| 1180 | `0x00b0` STATUSPOINT (var 9) | 51 (unspent total) |
| 1181 | `0x00b0` TRAITPOINT (var 231) | 0 (sent even when 0) |
| 1182 | `0x00b0` BASELEVEL (var 11) | 2 |
| 1183 | `0x0acb` BASEEXP | 52 (600 - 548) |
| 1184 | `0x0acb` NEXTBASEEXP (var 22) | 894 |
| 1185-1190 | recalc diff | `0x00b0` HIT=179, FLEE1=103, MDEF2=0 (var 48), MDEF1=2 (var 47), MAXHP=45, MAXSP=12 |
| 1191 | `0x00b0` SP_HP | 45 (new max) |
| 1192 | `0x080e` | A 45/45 (to A, and to nearby non-party viewers) |
| 1193 | `0x00b0` SP_SP | 12 |
| 1194 | `0x019b` | aid A, effect 0 |
| 1195 | `0x00b0` JOBLEVEL | 5 |
| 1196-1197 | `0x0acb` | JOBEXP 27, NEXTJOBEXP 91 |
| 1198 | `0x00b0` SKILLPOINT | 4 |
| 1199-1200 | job diff | `0x0141` AGI `1+1`, `0x00b0` FLEE1=104 |
| 1201 | `0x019b` | aid A, effect 1 |
| 1202-1203 | `0x0acc` x2 | base 150, job 40 (last, after both bursts) |
| 1204 | `0x0a24` | achievement |

The second double (Lunatic, base 2 to 3, job 6 to 7; file 2170-2192) is identical: STATPOINT=5, TRAITPOINT=0, BASELEVEL=3, BASEEXP=58, NEXTBASEEXP=1486, diff HIT=193, FLEE1=105, MDEF2=0, MDEF1=5, MAXHP=51, MAXSP=13, `SP_HP`=51, `0x080e`, `SP_SP`=13, `0x019b` 0, JOBLEVEL=7, JOBEXP=45, NEXTJOBEXP=205, SKILLPOINT=6, NO job diff, `0x019b` 1, `0x0acc` 150 / 50. So: one diff per burst (two total); the base diff sends only changed values and the fixed order `HIT, FLEE1, MDEF2, MDEF1, MAXHP, MAXSP` (no ATK1/DEF/MATK change at those levels; HP/SP pushed by the diff are only the max values); heal comes after the diff and before `0x019b`; tail comes before the diff. HP and SP are both below the new max in all cases, so both pushes were sent. `0x0141` appears only for the job-bonus stat, never on a base level-up. No base-only level-up occurred in this capture (all base ups coincided with a job up), so "base burst alone, then `JOBEXP`, then `0x0acc`" stays source-derived.

Differences from the captured `@blevel 10` (3352-3376): `@blevel` sends diff (HIT, FLEE1, ATK1, DEF2, DEF1, CRIT, MATK2, MATK1, MDEF2, MDEF1, MAXHP, MAXSP), `SP_HP`, `0x080e`, `SP_SP`, `0x019b` 0, THEN `STATPOINT`=46, `TRAITPOINT`=0, `BASELEVEL`=13 (relative: 3 + 10), `BASEEXP`=0, `NEXTBASEEXP`=6106 (the Lv13 bar). `@jlevel 10` (3387-3403): `0x019b` 1 first (3389), then `JOBLEVEL`=11 (relative), `JOBEXP`=0, `NEXTJOBEXP`=2680 (Archer bar for Lv11), `SKILLPOINT`=18, diff (`0x0141` STR `11+1`, INT `1+1`, DEX `14+1`, HIT, ATK1, MATK2, MATK1, MDEF2, MDEF1). Natural is the reverse of both: pushes first, effect last. Neither command sends `0x0acc`.

### Job-exp leftover clamp (confirms `multi_level_up` off)

| Kill | Job exp before | + gain | Bar consumed | Raw leftover | Sent JOBEXP |
| :--- | ---: | ---: | ---: | ---: | ---: |
| 1 (686) | 0 | 40 | 10 | 30 | 9 (clamped to bar - 1) |
| 2 (851) | 9 | 40 | 18 | 31 | 17 (clamped) |
| 3 (984) | 17 | 40 | 28 | 29 | 27 (clamped) |
| 4 (1179) | 27 | 40 | 40 | 27 | 27 (below clamp 39, unchanged) |

The clamp uses the bar just consumed (the pre-level bar). Bars match `job_exp.yml` (Novice job 1-9: 10, 18, 28, 40, 91, 151, 205, 268, 340; base 1-3: 548, 894, 1486; Archer job 1: 60, job 11: 2680; base 13: 6106).

### 3. Max job level

NOT exercised. A reached Novice job 9 (JOBEXP 11 of 340, 3136-3141) and then `@job 3` (job level 1, `JOBLEVEL`=1, `JOBEXP`=0, `NEXTJOBEXP`=60, 3222-3229; base level/exp untouched), so no kill happened at Novice job 10, and Archer (max 50) was at job 11. Whether `0x0acc` shows 0 / `JOBEXP` clamps at the max stays source-derived.

### 4. Two attackers on one mob (step 8, Pupa 157/55, file 3087-3158)

Hits (`0x08c8` damage field, offset 20): B 0 and 0 (3092-3095, two zero-damage swings; the first swing of an attack is a 0 in the solo kills too, 666/669), B 14 (3096), B 15 (3107), A 107 (3119, applied at +587 ms = the kill), B 14 (3128, sent before the kill applied, so never applied). Mob HP max 51 (37, 22, 0). The log holds the RAW 107 (not the 22 effective): `mob_log_damage` gets the unclamped damage (`mob.cpp:2778`, `status.cpp:1656`).

Verification (`mob.cpp:3128-3160`): damage log A = 107, B = 29, total 136, `count = 2`:

| | per = dmg/total | per x (1 + 25%) | base: floor(157 x per) | job: floor(55 x per) | level rate | sent |
| :--- | ---: | ---: | ---: | ---: | :--- | :--- |
| A (base Lv3, diff +1) | 0.78676 | 0.98346 | 154 | 54 | 100 | 154 / 54 (`0x0acc` 3156, 3158) |
| B (base Lv1, diff +3) | 0.21324 | 0.26654 | 41 | 14 | 105 | 43 / 14 (`0x0acc` 3153, 3155) |

So the formula and the 25% two-attacker bonus hold, the double is cast to an integer (truncated) BEFORE the penalty, and the penalty is then integer `x rate / 100` (41 x 105 / 100 = 43, 14 x 105 / 100 = 14). Rejected alternatives: B = 43 (all three hits) gives 58/19; effective damage (22/29) gives 84/116. Combined 197 base from a 157-exp mob (the bonus inflates the total). B's burst: `0x0acb` BASEEXP=43, JOBLEVEL=2, JOBEXP=4, NEXTJOBEXP=18, SKILLPOINT=1, `0x0141` LUK `1+1`, ATK1=2, FLEE2=1, CRIT=1, `0x019b` 1, `0x0acc` 43, `0x0acc` 14 (3132-3155); A's: BASEEXP=1311, JOBLEVEL=9, JOBEXP=11, NEXTJOBEXP=340, SKILLPOINT=8, `0x0141` INT `1+1`, MATK2=39, MATK1=0, MDEF2=0, MDEF1=6, `0x019b`, `0x0acc` 154 / 54 (3135-3158). Sessions interleave in the log by thread; read each session separately.

### Exp amounts vs mob_db and level penalty

`$RATHENA_PATH/db/re/mob_db.yml` (the project's `crates/ro-db/data/mob/mob_db.yml` drops exp fields): Poring Lv1 150/40, Lunatic Lv3 150/50, Pupa Lv4 157/55, Marine Sphere Lv51 426/373. `rate` from `level_penalty.yml` ([../db/exp_tables.md](../db/exp_tables.md#renewal-level-difference-exp-penalty)); `exp = trunc(db x per)` then `exp x rate / 100` integer.

| Kill (file line) | Player base Lv | diff | rate | Expected | Sent |
| :--- | ---: | ---: | ---: | :--- | :--- |
| Poring (697-698, 1202, ...) | 1-2 | 0 / -1 | 100 | 150 / 40 | 150 / 40 |
| Lunatic (2139-2140) | 2 | +1 | 100 | 150 / 50 | 150 / 50 |
| Lunatic (2191-2192) | 2 (before ding) | +1 | 100 | 150 / 50 | 150 / 50 |
| Pupa (2772-2773 ...) | 3 | +1 | 100 | 157 / 55 | 157 / 55 |
| Pupa (3447-3448 ..., Archer) | 13 | -9 | 95 | 149 / 52 (149.15, 52.25) | 149 / 52 |
| Marine Sphere (4116-4117, 4429-4430) | 13 | +38 | 40 | 170 / 149 (170.4, 149.2) | 170 / 149 |

Running totals add up exactly (no exp lost or bonus): base 150, 300, 450, then level 2 with 52 after the 4th Poring, 202, 352, 502, 652, 802, level 3 with 58, then +157 each (215, 372, 529, 686, 843, 1000, 1157), +154 (1311); after `@blevel`/`@jlevel` both 0, then +149 x7 (149 ... 1043), +170 (1213), +170 (1383); job 9, 17, 27, 27 (lvl up 5), 67, 16 (lvl 6), 56, 96, 146, 45 (lvl 7), 100, 155, 5 (lvl 8), ..., 225, 11 (lvl 9), then Archer 52 ... 364, 513, 662. The Novice kills happened at base Lv1-3 against Lv1-4 mobs, so no penalty applied until the Archer part.

### 5. Relog and persistence

A sent `0x00b2` Restart type 1 (back to character select, 4639) then re-entered the char server (session `#7`, file 4644-4660); the capture ends without a map re-entry, so exp at map login after the relog is not captured. The char list (`0x006b` 4647 and `0x0b72` 4656-4660, 175-byte `CHARACTER_INFO` entries: `i64 exp` at entry offset 4, `i64 jobexp` at 16, `i32 joblevel` at 24, `i16 statpoint` at 48, `i16 level` at 92) shows A as: base exp 1383, job exp 662, job level 11, base level 13, class 3 (Archer), status points 46, HP 149/149, SP 36/36. These equal A's last in-game values (4427-4430 and 3372-3394), so exp and levels were saved on the logout-to-select (`CSAVE_QUIT`, as in "Persistence"). The fresh chars show exp 0/0, job level 1, base level 1, status points 48, HP 40/40, SP 11/11 (`0x0b6f` at 37 and 251).

### 6. Fresh-char login burst (A, file 60-136)

Exp-related part, in order: `0x0acb` BASEEXP=0 (102), NEXTBASEEXP=548 (103), JOBEXP=0 (104), NEXTJOBEXP=10 (105) (real Novice table values, not zero), then `0x00b0` SKILLPOINT=0 (106), then `0x00bd` (107). Preceding recalc diff (60-81): LUK `0x0141`, HIT=177, FLEE1=102, ASPD=590, ATK1=1, DEF1=1, DEF2=10, ATK2=17, DEF2=10, DEF1=1, FLEE2=1, CRIT=1, MATK2=1, MATK1=0, MDEF2=0, MDEF1=1, `0x013a`, MAXHP=40, MAXSP=11, HP=40, SP=11, WEIGHT=500. After `0x00bd`: `0x0141` STR..LUK (all `1+0`), `0x013a`, ASPD, trait block (`0x0141` 219-224, `0x00b0` 225-230, TRAITPOINT=0, 232, 233), `0x00be` 0x00f7-0x00fc.

`0x00bd` payload (file 107, 44 bytes incl. opcode): `point` 48; STR/AGI/VIT/INT/DEX/LUK each `base 1, cost 2`; `attPower` 1 (batk), `refiningPower` 17, `max_matt` 0, `min_matt` 1, `itemdef` 1, `plusdef` 10, `mdef2` 1, `plusmdef` 0, `hit` 177, `flee` 102, `flee2` 1, `crit` 1, `aspd` 590, `plusASPD` 0. Layout: [../packets/status.md](../packets/status.md#0x00bd-zc_status). (`attPower`/`refiningPower` assignment follows that doc; the capture only fixes the values.)

### Still open

* Max job/base level behaviour (`0x0acc` amount, exp clamp): not captured.
* Base-only level-up (no simultaneous job up): not captured (source order stands).
* Party members: `0x0abd`, `0x080e` to others, delayed party info: no party in this capture.
* Map re-entry exp values after relog (only the char-select copy was captured).
* `@showexp` chat line and `0x0a24` objective details: ignored here.
