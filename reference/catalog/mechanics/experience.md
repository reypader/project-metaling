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

After the loop, in order: pet level update if `pet_lv_rate`; self packets `SP_STATUSPOINT`, `SP_TRAITPOINT`, `SP_BASELEVEL` (`0x00b0`), `SP_BASEEXP`, `SP_NEXTBASEEXP` (`0x0acb`); `status_calc_pc(SCO_FORCE)` (full recalculation; max HP/SP etc. change and are sent by the recalc); `status_percent_heal(100, 100)` = FULL HP and SP unconditionally (no config switch); Super Novice / Taekwon angel buffs; `0x019b` effect 0 (AREA); `OnBaseLvUp`-style NPC event; party level notify (`party_send_levelup`); equipment `elvmax` check (unequips items whose max-level limit is now exceeded); quest-info refresh.

### Job level-up

Source: `pc_checkjoblevelup`, `pc.cpp:8317-8364`.

Same loop with `job_exp`/`multi_level_up_job`; per level: `job_level++` and `skill_point += 1` (skill points are 1 per job level, no table). At max job level `job_exp = min(job_exp, 999999999)`. Packets: `SP_JOBLEVEL`, `SP_JOBEXP`, `SP_NEXTJOBEXP`, `SP_SKILLPOINT`; `status_calc_pc(SCO_FORCE)`; `0x019b` effect 1 (AREA). No heal on job level-up. Job exp caps differ by class group (Novice max job level 10, 1st classes 50, etc.); see exp_tables.md. When the player has already reached the class's max job level, job exp accrues only up to 999999999 and displays 0; there is no separate "novice cap" code, it is purely the `MaxJobLevel` of the job group.

### Direct set (script/GM)

`setparam` of `BaseLevel` (`pc.cpp:10412-10440`): caps to max base level, adds the status points for each skipped level, sets `base_exp = 0`, sends `NEXTBASEEXP`, `STATUSPOINT`, `TRAITPOINT`, `BASEEXP`, recalcs. GM `@baselevelup`/`@joblevelup` (relative change, resets exp, different point/heal rules): [atcommands.md#baselevelup--joblevelup](./atcommands.md#baselevelup--joblevelup). `BaseExp`/`JobExp` set uses `pc_gainexp(exp_flag=2)` or `pc_lostexp` as described above.

## Death exp penalty

Source: `pc.cpp:10005-10060`, `exp.conf:80-96`.

Applies unless the class is Novice (`MAPID_SECONDMASK == NOVICE`), the player has `cant.deathpenalty`, or the map has `MF_NOEXPPENALTY`/GvG. `death_penalty_type`: 0 none, 1 (default) lose `penalty/10000` of the NEXT-level bar (`death_penalty_base`/`_job` default 100 = 1%), 2 lose that fraction of CURRENT exp. Doubled in `pk_mode` when killed by a player. Clamped to current exp (cannot de-level). At max level the penalty is skipped unless `death_penalty_maxlv` bit 1 (base) / bit 2 (job) is set (default 0). A Token of Siegfried / `ITEMID_NEW_INSURANCE` item in inventory is consumed instead of losing exp. VIP uses `vip_exp_penalty_base/_job`. Applied via `pc_lostexp`, which sends `0x0acb` for the changed exp var then `0x0acc` with a negative amount (`pc.cpp:8540-8562`).

## Persistence

`char` columns (`sql-files/main.sql`, see [../db/sql_schemas.md](../db/sql_schemas.md)): `base_level`, `job_level` (smallint unsigned), `base_exp`, `job_exp` (bigint unsigned), `status_point`, `skill_point`, `trait_point` (int unsigned). All saved by the periodic/logout char save, not at level-up time.
