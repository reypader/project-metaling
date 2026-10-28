# Character Status Packets (ZC_STATUS, ZC_PAR_CHANGE family, stat-up)

Target `PACKETVER = 20260219`, renewal. Provenance: `rathena` at `$RATHENA_PATH` (commit `d4b8e7b8f`, 2026-10-06), `src/map/clif.cpp`, `packets_struct.hpp`, `packets.hpp`, `clif_packetdb.hpp`, `map.hpp`. `PACKETVER = 20260219` is neither `PACKETVER_RE` nor `ZERO`, so `PACKETVER_MAIN_NUM = PACKETVER` (`src/config/packets.hpp:28`) and every `PACKETVER_MAIN_NUM >= 20170906 / 20200916` gate below is TRUE. No proxy capture of these packets exists in the repo (`tools/proxy/packet_captures/` is absent); everything here is source-derived and unverified against the real 20260219 client except the weight pair noted in [map_units.md](./map_units.md#0x00b0-zc_par_change). Formulas for the values: [../mechanics/stats.md](../mechanics/stats.md#derived-stat-formulas). Map-entry order: [../mechanics/map_entry_actions.md](../mechanics/map_entry_actions.md#map-entry-cz_notify_actorinit-clif_parse_loadendack).

All structs are packed little-endian; all are Map to Client except `0x00bb` and `0x0b24`. None of the client-to-map opcodes below appear in the `PACKETVER > 20180307` branch of `clif_shuffle.hpp` (the one that applies, see [PF-05](../../../PACKET_FINDINGS.md#pf-05)), so `clif_packetdb.hpp` applies unchanged.

## Packet summary

| Opcode | Name | Frame | Layout (after the 2-byte opcode) | Source |
| :--- | :--- | :--- | :--- | :--- |
| `0x00bd` | `ZC_STATUS` | 44 | see below | `packets.hpp:909-939`, `clif.cpp:4118-4160` |
| `0x00b0` | `ZC_PAR_CHANGE` | 8 | `var:u16, value:i32` | `packets_struct.hpp:354-359`, `clif.cpp:3550` |
| `0x00b1` | `ZC_LONGPAR_CHANGE` | 8 | `var:u16, value:i32` (same shape as `0x00b0`, different opcode) | `packets_struct.hpp:361-366`, `clif.cpp:3562` |
| `0x0acb` | `ZC_LONGLONGPAR_CHANGE` | 12 | `var:u16, value:i64` | `packets_struct.hpp:399-404`, `clif.cpp:3624` |
| `0x0141` | `ZC_COUPLESTATUS` | 14 | `var:u32, base:i32, bonus:i32` | `packets_struct.hpp:390-396`, `clif.cpp:3611` |
| `0x00be` | `ZC_STATUS_CHANGE` | 5 | `var:u16, value:u8` | `packets_struct.hpp:368-373`, `clif.cpp:3574` |
| `0x013a` | `ZC_ATTACK_RANGE` | 4 | `range:i16` | `packets_struct.hpp:384-388`, `clif.cpp:3600` |
| `0x00bc` | `ZC_STATUS_CHANGE_ACK` | 6 | `var:u16, ok:u8, value:u8` | `packets_struct.hpp:4219-4225`, `clif.cpp:4285-4300` |
| `0x00bb` | `CZ_STATUS_CHANGE` (client to map) | 5 | `var:u16, amount:u8` | `clif_packetdb.hpp:65`, `clif.cpp:12716-12735` |
| `0x0b24` | `CZ_ADVANCED_STATUS_CHANGE` (client to map) | 6 | `var:i16, amount:i16` | `packets.hpp:185-190`, `clif_packetdb.hpp:1986` |

Frame lengths are `sizeof` of the packed structs (opcode included). `0x00b0` and `0x00b1` carry an `i32` (signed) value; `0x0acb` an `i64`.

## `0x00bd` ZC_STATUS

> Open findings: [PF-10](../../../PACKET_FINDINGS.md#pf-10), [PF-16](../../../PACKET_FINDINGS.md#pf-16).

Sent by `clif_initialstatus` (`clif.cpp:4123`). Fixed 44 bytes.

| Offset | Size | Type | Field (rAthena name) | Value |
| ---: | ---: | :--- | :--- | :--- |
| 0 | 2 | u16 | `packetType` | `0x00bd` |
| 2 | 2 | u16 | `point` | `min(status_point, 32767)` |
| 4 | 1 | u8 | `str` | `min(status.str, 255)`: allocated points only, no job/equip bonus |
| 5 | 1 | u8 | `standardStr` | stat-point cost to raise STR by 1 (`pc_need_status_point(STR,1)`; 0 at max) |
| 6 | 1 | u8 | `agi` | as STR |
| 7 | 1 | u8 | `standardAgi` | cost |
| 8 | 1 | u8 | `vit` | |
| 9 | 1 | u8 | `standardVit` | |
| 10 | 1 | u8 | `int_` | |
| 11 | 1 | u8 | `standardInt` | |
| 12 | 1 | u8 | `dex` | |
| 13 | 1 | u8 | `standardDex` | |
| 14 | 1 | u8 | `luk` | |
| 15 | 1 | u8 | `standardLuk` | |
| 16 | 2 | i16 | `attPower` | renewal: status ATK (`batk`) |
| 18 | 2 | i16 | `refiningPower` | renewal: `watk + watk2 + eatk` (weapon ATK incl. refine/overrefine + equip ATK) |
| 20 | 2 | i16 | `max_mattPower` | renewal: right side = `rhw.matk + lhw.matk + ematk + consumable + pseudobuff` MATK |
| 22 | 2 | i16 | `min_mattPower` | renewal: left side = status MATK |
| 24 | 2 | i16 | `itemdefPower` | renewal: `def2` (SOFT DEF) |
| 26 | 2 | i16 | `plusdefPower` | renewal: `def` (HARD DEF) |
| 28 | 2 | i16 | `mdefPower` | renewal: `mdef2` (SOFT MDEF) |
| 30 | 2 | i16 | `plusmdefPower` | renewal: `mdef` (HARD MDEF; negative allowed in renewal) |
| 32 | 2 | i16 | `hitSuccessValue` | `hit` |
| 34 | 2 | i16 | `avoidSuccessValue` | `flee` |
| 36 | 2 | i16 | `plusAvoidSuccessValue` | `flee2 / 10` (perfect dodge) |
| 38 | 2 | i16 | `criticalSuccessValue` | `cri / 10` |
| 40 | 2 | i16 | `ASPD` | `amotion` (raw, ms-scale; client shows `200 - amotion/10`) |
| 42 | 2 | i16 | `plusASPD` | always 0 |

Notes:

* Left/right sides come from the `pc_leftside_*` / `pc_rightside_*` macros (`pc.hpp:1239-1253` renewal branch). The packet names `max_matt`/`min_matt` are swapped relative to what the macros return: field at offset 20 gets the RIGHT side, offset 22 the LEFT.
* The struct has no HP/SP/weight/exp/zeny fields; those travel in `0x00b0`/`0x00b1`/`0x0acb`.
* Bytes above 255 are saturated (`min(.., UINT8_MAX)`); a stat over 255 is repaired by the `0x0141` that follows (see below).
* Immediately after the `0x00bd` frame `clif_initialstatus` sends, in this order (`clif.cpp:4166-4197`), via `clif_updatestatus`: STR, AGI, VIT, INT, DEX, LUK (`0x0141` each), ATTACKRANGE (`0x013a`), ASPD (`0x00b0`), then the renewal block: POW, STA, WIS, SPL, CON, CRT (`0x0141`), PATK, SMATK, RES, MRES, HPLUS, CRATE (`0x00b0`), TRAITPOINT, AP, MAXAP (`0x00b0`), UPOW, USTA, UWIS, USPL, UCON, UCRT (`0x00be`). So trait stats ARE sent unconditionally at login for every class when the server is built renewal and PACKETVER >= 20200916 (true here), even though non-4th classes have all-zero values.

## SP_* var ids

Source: `enum _sp`, `map.hpp:498-541`, positional.

Ids as sent in `var` of the PAR_CHANGE family. Columns "Packet" = what `clif_updatestatus` (`clif.cpp:3637-3900`) emits and "Value sent".

| Id | Name | Packet | Value sent |
| ---: | :--- | :--- | :--- |
| 0 | `SP_SPEED` | `0x00b0` | `battle_status.speed` (walk delay ms, default 150) |
| 1 | `SP_BASEEXP` | `0x0acb` | `client_exp(base_exp)` (i64) |
| 2 | `SP_JOBEXP` | `0x0acb` | `client_exp(job_exp)` |
| 3 | `SP_KARMA` | `0x00b0` | `status.karma` |
| 4 | `SP_MANNER` | `0x00b0` | `status.manner` |
| 5 | `SP_HP` | `0x00b0` | `hp`; if `hp == 0`: `1` (non-Novice) or `max_hp/2` (Novice branch) |
| 6 | `SP_MAXHP` | `0x00b0` | `max_hp` |
| 7 | `SP_SP` | `0x00b0` | `sp` |
| 8 | `SP_MAXSP` | `0x00b0` | `max_sp` |
| 9 | `SP_STATUSPOINT` | `0x00b0` | `status.status_point` |
| 11 | `SP_BASELEVEL` | `0x00b0` | `base_level` |
| 12 | `SP_SKILLPOINT` | `0x00b0` | `skill_point` |
| 13-18 | `SP_STR, AGI, VIT, INT, DEX, LUK` | `0x0141` | `var=id, base=status.<stat>, bonus=battle_status.<stat> - status.<stat>` |
| 20 | `SP_ZENY` | `0x00b1` | `status.zeny` (`i32`) |
| 22 | `SP_NEXTBASEEXP` | `0x0acb` | `client_exp(pc_nextbaseexp)` |
| 23 | `SP_NEXTJOBEXP` | `0x0acb` | `client_exp(pc_nextjobexp)` |
| 24 | `SP_WEIGHT` | `0x00b0` | `weight` (0.1 units); also runs `pc_updateweightstatus` |
| 25 | `SP_MAXWEIGHT` | `0x00b0` | `max_weight` |
| 32-37 | `SP_USTR, UAGI, UVIT, UINT, UDEX, ULUK` | `0x00be` | `var=id, value=u8 stat-point cost to raise that stat by 1` |
| 41 | `SP_ATK1` | `0x00b0` | left ATK = `batk` (status ATK) |
| 42 | `SP_ATK2` | `0x00b0` | right ATK = `watk + watk2 + eatk` |
| 43 | `SP_MATK1` | `0x00b0` | RIGHT MATK (weapon+extra) |
| 44 | `SP_MATK2` | `0x00b0` | LEFT MATK (status MATK) |
| 45 | `SP_DEF1` | `0x00b0` | `def2` (soft DEF) |
| 46 | `SP_DEF2` | `0x00b0` | `def` (hard DEF) |
| 47 | `SP_MDEF1` | `0x00b0` | `mdef2` (soft MDEF) |
| 48 | `SP_MDEF2` | `0x00b0` | `mdef` (hard MDEF, may be negative in renewal) |
| 49 | `SP_HIT` | `0x00b0` | `hit` |
| 50 | `SP_FLEE1` | `0x00b0` | `flee` |
| 51 | `SP_FLEE2` | `0x00b0` | `flee2 / 10` |
| 52 | `SP_CRITICAL` | `0x00b0` | `cri / 10` |
| 53 | `SP_ASPD` | `0x00b0` | `amotion` |
| 55 | `SP_JOBLEVEL` | `0x00b0` | `job_level` |
| 99 | `SP_CARTINFO` | `0x0121` | cart count/weight (not `0x00b0`) |
| 219-224 | `SP_POW, STA, WIS, SPL, CON, CRT` | `0x0141` | base `status.<trait>`, bonus `battle - status` |
| 225-230 | `SP_PATK, SMATK, RES, MRES, HPLUS, CRATE` | `0x00b0` | `battle_status.<field>` |
| 231 | `SP_TRAITPOINT` | `0x00b0` | `status.trait_point` |
| 232 | `SP_AP` | `0x00b0` | `battle_status.ap` |
| 233 | `SP_MAXAP` | `0x00b0` | `battle_status.max_ap` |
| 247-252 | `SP_UPOW, USTA, UWIS, USPL, UCON, UCRT` | `0x00be` | trait-point cost (always 1; 0 at max) |
| 1000 | `SP_ATTACKRANGE` | `0x013a` | `rhw.range` (weapon range in cells) |

Unused ids in this block (`SP_0a`=10, `SP_CLASS`=19, `SP_SEX`=21, 26-31, 38-40, 54, 56-60) have no `clif_updatestatus` case for the status window (falls to default). `SP_BASEEXP`/`JOBEXP`/`NEXT*EXP` use `0x0acb` because `PACKETVER >= 20170830`; below that gate they would use `0x00b1`. Exp semantics and `client_exp`: [../mechanics/experience.md](../mechanics/experience.md).

## Stat-up request and acknowledgement

> Open findings: [PF-10](../../../PACKET_FINDINGS.md#pf-10).

`CZ_STATUS_CHANGE` `0x00bb`: `var:u16` at offset 2 (`SP_STR`=13 .. `SP_LUK`=18), `amount:u8` at offset 4 (`parseable_packet(0x00bb, 5, clif_parse_StatusUp, 2, 4)`). Clients 2013-12-23 and newer send the real amount (old clients always 1). Source reading said a negative `i8` is rejected (`increase <= 0`), but **captured:** `0xe7` (231) raised STR to the cap (capture 1 line 4508), so the amount behaves as unsigned.

`pc_statusup` (`pc.cpp:8872-8932`) on success, in order:

1. stat += n, `status_point -= cost` where cost is the sum of per-point costs (see [stats.md](../mechanics/stats.md#stat-point-costs-and-limits)); `status_calc_pc(SCO_NONE)` (this pushes every changed derived value, see [Recalculation pushes](#recalculation-pushes) and always includes the stat itself as `0x0141`).
2. `0x00be` with `var = SP_USTR + (type - SP_STR)` and the new cost of the NEXT point of that stat only (other five stats' costs are not resent).
3. `0x00b0` `SP_STATUSPOINT`.
4. `0x00bc` ack: `var = type`, `ok = 1`, `value = min(new_stat, 255)`.
5. If the new stat > 255: `0x0141` again (after the ack, so it overrides the truncated value).

On failure (type out of 13..18, `increase <= 0`, increase capped to affordable/max is 0, would exceed class max, not enough points) only `0x00bc` is sent with `ok = 0`, `value = 0`. The requested amount is silently capped to what the points allow before the failure checks (`pc_maxparameterincrease`), so asking for more than affordable raises as many as possible.

Trait stats: `CZ_ADVANCED_STATUS_CHANGE` `0x0b24` (`var:i16` in `SP_POW`..`SP_CRT` = 219-224, `amount:i16`) goes to `pc_traitstatusup` (`pc.cpp:9034+`; same sequence with `SP_UPOW`+offset, `SP_TRAITPOINT`, then ack): 1 trait point per stat point, fails with the same `0x00bc` (`ok=0`) form. Only trait jobs (4th classes) have a non-zero max (see stats.md).

## Recalculation pushes

`status_calc_pc` = `status_calc_bl_(sd, all SCB flags, opt)` (`status.hpp:3684`). `status_calc_bl_` (`status.cpp:6525-6700`) snapshots the battle status, recomputes, then sends one update per field that CHANGED, in this fixed order (renewal):

STR, AGI, VIT, INT, DEX, LUK (`0x0141` each), HIT, FLEE1, ASPD, SPEED, ATK1 (if `batk` changed), DEF1 then DEF2 (if `def` changed; renewal sends both), ATK2 (if `watk/watk2/eatk` changed), DEF2 then DEF1 (if `def2` changed; so a double send is possible), FLEE2, CRITICAL, MATK2 then MATK1 (if min or max changed), MDEF1 then MDEF2 (if `mdef` changed), MDEF2 then MDEF1 (if `mdef2` changed), ATTACKRANGE, MAXHP, MAXSP, HP, SP, POW, STA, WIS, SPL, CON, CRT, PATK, SMATK, RES, MRES, HPLUS, CRATE, MAXAP, AP.

Rules worth copying:

* Diff-based: an unchanged field sends nothing. Comparison is against the pre-call battle status.
* The first calculation at login happens when the inventory table arrives from the char server (`intif.cpp:3505`: `status_calc_pc(SCO_FIRST|SCO_FORCE)`, then `status_calc_weight(CALCWT_ITEM|CALCWT_MAXBONUS)`), at which point `sd->prev` is null. The diff-push still runs against a zeroed snapshot, so (source reading, unverified by capture) HP/SP/MaxHP/MaxSP and all derived values are pushed before the client sends `0x007d`. Zeny, base/job level and the job/char identity fields the client already has from the character-select entry; `LoadEndAck` itself only (re)sends exp, skill points, `0x00bd` block (see below).
* `SCO_FORCE` bypasses the "hold recalc while delayed damage is pending" queue (`status.cpp:6527-6537`).
* HP/SP clamps: when `max_hp`/`max_sp` shrinks below current, HP/SP are clamped and `SP_HP`/`SP_SP` are pushed (`status.cpp:6198-6225`). **Captured:** the clamp push comes *before* the diff, and the diff sends the clamped field again after MAXHP/MAXSP (see [Captured sequences](#captured-stat-up-level-and-job-sequences)).
* Weight is not part of this diff; `status_calc_weight` pushes `SP_WEIGHT` and `SP_MAXWEIGHT` itself only when they change (`status.cpp:3693-3697`), and `pc_additem`/`pc_delitem` push `SP_WEIGHT`. **Captured:** a changed `SP_MAXWEIGHT` goes out *before* the diff's first `0x0141`.
* **Captured:** the diff compares rAthena's internal units, so `FLEE2` (`flee2`, 1/10) and `CRITICAL` (`cri`, 1/10) are re-sent on every LUK change even when the displayed `/10` value is unchanged, and the pairs (DEF1+DEF2, MATK2+MATK1, MDEF2+MDEF1) always go out together.
* Level-up, job change, stat-up, equip/unequip, skill learn/passives, buffs/debuffs and many script `bonus` effects all call `status_calc_pc`. Level-up flow: [../mechanics/experience.md](../mechanics/experience.md#level-up).

## Where the status packets sit in the map-entry order

> Open findings: [PF-12](../../../PACKET_FINDINGS.md#pf-12).

Do not duplicate; see [map_entry_actions.md](../mechanics/map_entry_actions.md#map-entry-cz_notify_actorinit-clif_parse_loadendack) step list. Summary of what this file adds, in `clif_parse_LoadEndAck` order (`clif.cpp:10748-11110`):

1. Always: `0x00b0` `SP_WEIGHT`, `SP_MAXWEIGHT` (`10793-10794`), before the spawn and nearby units.
2. First login only (`state.connect_new`, `10904-10988`): `SP_BASEEXP`, `SP_NEXTBASEEXP`, `SP_JOBEXP`, `SP_NEXTJOBEXP` (`0x0acb`), `SP_SKILLPOINT` (`0x00b0`), then `clif_initialstatus` = `0x00bd` + the follow-up `0x0141`/`0x013a`/`0x00b0`/`0x00be` packets listed under ZC_STATUS. `SP_STATUSPOINT`, HP/SP, level and zeny are NOT in this block.
3. Map change (not first login, `10989-11003`): `0x0141` for STR..LUK and (renewal) POW..CRT only. No `0x00bd`, no exp, no weight beyond step 1, no HP/SP.
4. Changemap-only content later calls `status_calc_pc(SCO_NONE)` (`clif.cpp:11092`), so any changed derived value gets a diff push.

## Captured stat-up, level and job sequences

Source: [captures/attribute_sequence.txt](../captures/attribute_sequence.txt) (rAthena `d4b8e7b`, `PACKETVER 20260219`; line numbers are file lines). Session `#3` = `test` (Bard, base 99, job 1); `#5` and `#7` = `testtesttest` (Novice base 1 at login, then Swordman, Knight, Lord Knight, Rune Knight, Shadow Cross, Knight). No observer client, so area delivery is still source-only. Values per step are in [mechanics/stats.md](../mechanics/stats.md#captured-status-values). `0x080e` (party HP bar) follows each `SP_HP` push because the character is in a party; it is not a status packet.

**Proxy misframing:** the proxy used for this recording did not know `0x00bc`. Only the first ack frames correctly (line 162). Later acks show as a short `0x00bc`, a bogus `0x0100`, and a bogus frame that swallows the first byte of the next packet (`0x410c`, `0xb00d`, ...). The bytes are `var`, `ok = 1`, `value`, as in the layout above. `table.rs` has had the row since (`stats_stat_up_rows`).

**Stat-up** (lines 157-245, 454-1696, 4508-4541). Every captured request succeeded. Order, matching `pc_statusup`:

1. The recalc pushes: `SP_MAXWEIGHT` first when STR changed, then the stat's `0x0141`, then the changed derived values in the recalc order.
2. `0x00be` with the next-point cost of that stat (0 at the cap: INT 99 at line 243).
3. `SP_STATUSPOINT`.
4. `0x00bc` ok 1 with the new stat.

An over-affordable `amount` raises as far as points and the cap allow:

* INT +99 from 3 stops at the cap 99 (line 236).
* STR +231 stops at 99 (line 4508).
* DEX +99 with 17 points stops at 9, leaving 1 (line 4531).

Capture 1 has no failure (`ok = 0`); [capture 2](#capture-2-observer-stat-reset-failure-and-4th-class-jobs) does (line 987) and confirms the source form.

**`@blvl +n`** (lines 401-423; also 1062, 1360, 1842, 2956, 3052, 3173). Source order confirmed:

1. The diff (no HP/SP: nothing was clamped).
2. The heal: `SP_HP`, `SP_SP`.
3. `0x019b` effect 0.
4. The message.
5. `SP_STATUSPOINT`, `SP_TRAITPOINT` (0), `SP_BASELEVEL`.
6. `0x0acb` `SP_BASEEXP` 0, `SP_NEXTBASEEXP`.

**`@blvl -n`** (lines 679-703, level 11 to 6 with 52 unspent points against 21 to remove, so no reset):

1. The message first.
2. Clamp pushes `SP_HP` 66 and `SP_SP` 18.
3. The diff, ending `MAXHP`, `MAXSP`, `HP`, `SP`.
4. The same tail as level up. No heal, no effect.

The stat-reset branch was not exercised in capture 1; capture 2 has it (see below).

**`@jlvl +n`** (lines 428-450, 722-735, 1769):

1. `0x019b` effect 1.
2. The message.
3. `SP_JOBLEVEL`, `0x0acb` `SP_JOBEXP` 0, `SP_NEXTJOBEXP`, `SP_SKILLPOINT`.
4. The diff (job-bonus `0x0141`s, derived values). No heal.

**`@jlvl -n`** (lines 705-718): the message, the same four pushes, then the clamp push (`SP_SP` 17), then the diff (ending `MAXSP`, `SP`).

**`@job`**. Source order ([job_change.md](../mechanics/job_change.md#2-pc_jobchangesd-job-upper-order-of-operations)) confirmed step for step:

| Lines | Change | What the capture adds |
| :--- | :--- | :--- |
| 1714-1764 | Novice → Swordman | Plain case: `0x00be` ×6, `SP_JOBLEVEL`, `SP_JOBEXP`, `SP_NEXTJOBEXP`, looks `LOOK_BASE`, `LOOK_HAIR`, `LOOK_CLOTHES_COLOR`, `LOOK_BODY2`, **`0x010f` skill list** (step 15), the diff (`SP_MAXWEIGHT` first, `0x0141` ×6 for the job bonus, derived values, `MAXHP`, `MAXSP`; no HP/SP push, no heal), `pc_equiplookall` (`LOOK_WEAPON`, `LOOK_SHOES`, `LOOK_HEAD_BOTTOM`, `LOOK_HEAD_TOP`, `LOOK_HEAD_MID`, `LOOK_ROBE`), achievement packets, then the message. No `SP_SKILLPOINT`. |
| 2010-2042 | Swordman → Knight | Same shape. |
| 2102-2161 | `@job lrd_knight` | Unknown name: usage and the job list as `0x008e` lines, then the failure message. Nothing else. |
| 2238-2269 | Knight → Lord Knight (4008) | Step 8: `SP_STATUSPOINT` (+52) before the `0x00be` ×6. |
| 2619-2700 | Lord Knight → Rune Knight (4054) | Step 8 with fewer than 52 points left: `pc_resetstate` first. It sends `0x0141` ×6 (base 1, bonus = the **old** battle value − 1), trait `0x0141` ×6, `0x00be` ×6, trait `0x00be` ×6, `SP_STATUSPOINT`, `SP_TRAITPOINT`, then its own recalc (`SP_MAXWEIGHT`, the `SP_SP` clamp, the diff). Then step 9's `0x00be` ×6 and the usual tail. |
| 3697-3761 | Rune Knight → Shadow Cross (4254) | Step 10 grants 7 trait points (`SP_TRAITPOINT` 7, trait costs 1). **Forced unequip** of a worn Katana (step 12) right after the job-exp pushes: `0x01d7` `LOOK_WEAPON` 0 **twice** (two-hander, one per hand slot), then `0x099a` (pos `0x22`), then the looks. The diff (`ATK2` 0, `ASPD`) carries the unequip's effect. Skill deletes `0x0441` and a second `0x010f` sit between `SP_MAXWEIGHT` and the `0x0141`s. |
| 4352-4494 | Shadow Cross (base 170) → Knight (max 99) | Step 7: `pc_resetstate` (pushes as above, then its recalc: `SP_MAXWEIGHT`, skill deletes, `0x010f`, the `SP_HP`/`SP_SP` clamp, the diff, `SP_STATUSPOINT` 1273), then `SP_BASELEVEL` 99, `SP_BASEEXP`, `SP_NEXTBASEEXP`. Step 8 subtracts 52 (`SP_STATUSPOINT` 1221). Step 9's `0x00be` ×6. Step 10 runs a second `pc_resetstate` (back to 1273). The forced unequip of a katar (`0x01d7` ×2, `0x099a`) follows the job-level pushes. The final diff shows `ASPD` 1250 → 440, because the reset's recalc ran while the katar was still worn. |

**Relog** (session `#7`, lines 4568-4640): the same login pattern as session `#3`. That is the pre-ready diff (`SP_MAXWEIGHT`, `0x0141` ×6, every derived value with the DEF pair twice, `0x013a`, `MAXHP`, `MAXSP`, `HP`, `SP`, `SP_WEIGHT`) before `0x007d`, then the `0x00bd` block. This confirms the source-only "diff against a zeroed snapshot" bullet under [Recalculation pushes](#recalculation-pushes).

## Capture 2: observer, stat reset, failure and 4th-class jobs

Source: [captures/attribute_sequence_2.txt](../captures/attribute_sequence_2.txt) (rAthena `d4b8e7b`, `PACKETVER 20260219`; file lines; applies to 20260219). The proxy already had the `0x00bc` row, so every frame is correct. `#3` = `testtesttest` (Knight, job 7, base 99 at login; Merchant, Novice High, Super Novice, Hyper Novice later), `#6` = new Novice `testera`. Both stand in `prontera` and observe each other. `0x080e` (HP bar, `aid:u32, hp:u32, maxhp:u32`) goes to both clients after every `SP_HP`/`SP_SP` change of either, as in capture 1; ignore it below. Decoded values are in [mechanics/stats.md](../mechanics/stats.md#capture-2-values).

**Stat-up** (49 requests: 626-1015 and 1445-1495). Every success has exactly this wire order, with the right framing:

1. `SP_MAXWEIGHT` only when STR changed (line 627).
2. The stat's `0x0141` (`type`, `base` = allocated, `bonus` = battle - allocated).
3. The changed derived `0x00b0` values in the recalc order, including `MAXHP` (VIT, line 678) and `MAXSP` (INT, line 975). No `HP`/`SP` push, so no heal.
4. `0x00be` for that stat only, `var` = `SP_U*`, value = cost of the next point.
5. `SP_STATUSPOINT` (`0x00b0` var 9).
6. `0x00bc`: `var:u16` (stat id 13..18), `ok:u8` = 1, `value:u8` = the new stat (line 632: STR 2).

One request with `amount` > 1 produces one sequence, not one per point: INT +99 (line 969) goes from 4 to 99 in one `0x0141` and one `0x00bc` (value 99, 622 points spent). AGI +99 (992) stops at 87 with 3 points left, `0x00be` value 10 (cost at 87). INT +99 with 42 points (1495) stops at 18 with 1 left. `0x00be` is 0 at the cap (line 976).

**Failure** (line 986-987): INT +99 sent while INT is already 99 (the client's `+` was not yet disabled). The only reply is `0x00bc` with bytes `10 00 00 00`: `var` 16, `ok` 0, `value` 0. No `0x0141`, `0x00be` or `SP_STATUSPOINT`. This is the first captured failure.

No stat above 99 occurs, so the trailing `0x0141` for a stat above 255 is still source-only.

**Observer** (`#6`), area packets for `#3`:

* `@blevel 10` by `#6`, seen by `#3`: `0x080e` (line 431) and `0x019b` `aid, effect 0` (line 434). Nothing else: no stat, level or message packets.
* `@blevel -n`, stat-ups and stat resets by `#3`: only `0x080e` for `#6`.
* `@job` by `#3`: `#6` gets ten `0x01d7`: `LOOK_BASE` (job id), `LOOK_HAIR`, `LOOK_CLOTHES_COLOR`, `LOOK_BODY2` (job id), then the six `pc_equiplookall` looks (weapon, shoes, bottom, top, mid, robe). Lines 532-556 (Merchant), 1204-1213 (4001), 1263-1288 (23), 1349-1358 (4307). No `0x010f`, `0x00b0`, `0x00be` or message.
* Equip of a Tsurugi (611-616): `#3` gets `0x0999` (result 0), one `0x01d7` `LOOK_WEAPON` (view 1119), `ASPD`, `ATK2`; `#6` gets only the `0x01d7` (615). Take-off (1109-1114): `#3` gets `0x01d7` weapon 0, `0x099a`, `ASPD`, `ATK2`; `#6` the `0x01d7` (1114). No `0x019b`.
* Restart of `#3` (1515-1519): `#6` gets `0x0080` type 2 (logout) and `0x0196` (state 0x27 off); the relog shows `0x0983` (state 0x27 on) and `0x09fe` (1579, 1581, 1589).

**`@blevel -n` with the stat-reset branch** (`-3` at 464-527 from base 99; `-90` at 1023-1104 from base 96; the unspent points were 0 both times). Order:

1. `0x0141` ×12 (STR..LUK then POW..CRT) with `base` 1 (or 0) and `bonus` = the stale old battle value - 1 (lines 466-478, 1024-1039; e.g. STR 4 gives base 1, bonus 3).
2. `0x00be` ×12: `SP_USTR..SP_ULUK` then `SP_UPOW..SP_UCRT` (479-490, 1040-1051). Not six.
3. `SP_STATUSPOINT` = the table value of the old level (1273 at 99, 1207 at 96), `SP_TRAITPOINT` 0.
4. The reset's own recalc: `SP_MAXWEIGHT` (493), the clamp pushes if any (1055-1057), the `0x0141` of the stats that changed (494-496: only STR, AGI, DEX; all six at 1058-1063), the derived values, `MAXHP`/`MAXSP`/`HP`/`SP` when they changed.
5. The message (507, 1081).
6. The level change's recalc: clamp pushes (`SP_HP` 7671, `SP_SP` 300 at 508-510; 73 and 28 at 1082-1084), the diff ending `MAXHP`, `MAXSP`, `HP`, `SP`.
7. The tail: `SP_STATUSPOINT` (1207 at 523; 64 at 1100, the table value of the new level), `SP_TRAITPOINT`, `SP_BASELEVEL`, `0x0acb` `SP_BASEEXP` 0, `SP_NEXTBASEEXP`.

Without a reset (`-99` from base 6 to 1 with 64 unspent against 16 to remove, 1314-1337): message, clamp pushes `HP` 40 and `SP` 11, the diff, then the tail with `SP_STATUSPOINT` 48. No `0x0141` or `0x00be`. This is the shape capture 1 saw.

**`@job` sequences.** Order for every successful change: `0x00be` ×6, (trait jobs: `SP_TRAITPOINT`, `0x00be` ×6 trait), `SP_JOBLEVEL`, `0x0acb` job exp and next, the four looks, `0x010f`, the recalc (`SP_MAXWEIGHT` if changed, clamp pushes, `0x0141` and derived diff), the six equip looks (all view 0 here because the Tsurugi was off), achievement `0x0a24`s, the message. `SP_SKILLPOINT` is never sent. `0x010f` is re-sent every time; its length depends on the job: 152 bytes (Merchant, 4 skills), 41 (Novice High, `NV_BASIC` only), 929 (Super Novice, 25 skills), 1521 (Hyper Novice, 41 skills).

| Lines | Change | Notes |
| :--- | :--- | :--- |
| 531-572 | Knight B96 → Merchant (5) | `HP` clamp 2400 first, then `0x0141` VIT (lost the +1 bonus), DEF pair, `MAXHP` 2400. No `MAXSP` push (300). No `SP_STATUSPOINT`. |
| 1134-1193 | `@job supernovice` | Unknown name (rAthena names use underscores: `Super_Novice`). Only 59 `0x008e` lines (usage and the job list) and the failure message. Nothing else is sent. |
| 1203-1242 | Base 6 → Novice High (4001) | `SP_STATUSPOINT` 64 → 116 (+52) first (1214), `SP_JOBEXP`/`NEXTJOB` 11, `SP_MAXWEIGHT` 20300, `SP` clamp 28 → 20, `MAXHP` 82, `MAXSP` 20, `SP` 20. No `HP` push (73 < 82). No `0x0141`. |
| 1262-1308 | Novice High → Super Novice (23) | `SP_STATUSPOINT` 116 → 64 (-52, enough left so no reset, 1264). `HP` clamp 65 and `SP` clamp 16, `0x0141` STR base 1 bonus 1 (job bonus), `ATK1` 4, `MAXHP` 65, `MAXSP` 16. |
| 1348-1406 | Base 1 → Hyper Novice (4307) | No `SP_STATUSPOINT` (not a primary 4th class: no +52). `SP_TRAITPOINT` 7 (1365) and the trait `0x00be` ×6 (value 1). `SP_MAXWEIGHT` 40300, `0x0141` ×6 with bonus 11/11/10/10/10/10, derived values, `MAXHP` 99 only (`MAXSP` stays 11). |

No forced unequip happened in this capture: the Tsurugi was already off (1109-1111) when 4001, 23 and 4307 ran, and the re-equip at 1115 was refused (`0x0999` result 1, level) because `@blevel -90` does not recheck the minimum level (the Tsurugi stayed on at base 6 until the player took it off). So `0x010f`, trans points and trait points are captured; a forced unequip on a 4th class is not.

**Map change and map entry** (`@go prontera`, `#3` at 300-348, `#6` at 350-400). After `0x0091`, `0x007d` and the inventory lists: `SP_WEIGHT`, `SP_MAXWEIGHT`, `0x099b`, the `0x09ff` units, then `0x0141` ×12 (STR..LUK, POW..CRT; lines 326-337 and 378-389), then `0x02c9`, `0x02da`, `0x02d9` ×4, `0x0b8d`, and `0x0b1b` after the client's `0x0447`. No `0x00bd`, exp, HP/SP or `0x010f`. First login (lines 38-146, `#3`): the pre-ready diff before `0x007d` (`SP_MAXWEIGHT`, `0x0141` ×6 only at 47-52, derived, `0x013a`, `MAXHP`, `MAXSP`, `HP`, `SP`, `SP_WEIGHT`), then after `0x007d`, the inventory and unit block (80-90): `0x010f` (91), `0x0b20` ×2, exp ×4, `SP_SKILLPOINT`, `0x00bd` (99), `0x0141` ×6, `0x013a`, `ASPD`, `0x0141` ×6 trait, nine trait `0x00b0` (`PATK`..`MAXAP`), `0x00be` ×6 trait (123-128), the config group, `0x0b8d`, and then a late `SP_ASPD` push (270 → 230 at line 136; 420 → 410 at line 1651). The late push also exists in capture 1 (its Knight row is the pre-ready 270). Cause: the Berserk Potion buff the recorder had active (user, 2026-10-26): a status effect restored after the first calc, so the pre-ready value is the unbuffed one.

## Capture 3: elvmax unequip, Hyper Novice, @jlvl observer

Source: [captures/attribute_sequence_3.txt](../captures/attribute_sequence_3.txt) (rAthena `d4b8e7b`, `PACKETVER 20260219`, `PROXY_FULL_PAYLOAD=1`; file lines = proxy `#` + 10; applies to 20260219). `#6` = account 2000000, new Novice `testero` (slot 7, the actor, login block 176-285); `#5` = account 2000001 `testera` (the observer, logged in at 58). Both in `prontera` after `@go prontera` (289). **Cross-session order is the proxy's read order, not the send order**: both sockets are written in the same millisecond, so only per-session order below is reliable. The recorder's own `@blevel` steps differ from the plan (a Novice caps at 99), so the forced unequips happen on a Hyper Novice.

| Lines | Command | Effect |
| :--- | :--- | :--- |
| 369-393 | `@blevel 96` | Novice B1 → 97, nothing worn. Standard `@blvl +n` order. |
| 403, 415 | `@item Mobile_BeginnerMark` (19256, top, View 1348, `EquipLevelMax` 99) and `@item Mana_Boots` (22012, shoes, max 100) | `0x0b41`, `SP_WEIGHT`, message. |
| 422-430 | Equip boots (`index 6`, pos `0x40`), then Mark (`index 5`, pos `0x100`) | No recalc diff (neither item has a stat bonus; both only add exp/drop bonuses). |
| 443-460 | `@blevel 1` → 98 | Nothing comes off. |
| 465-495 | `@blevel 2` → **99** (clamped: Novice max) | Nothing comes off (99 > 99 is false). |
| 505, 547, 560 | `@job 1`, `@job 1` again (two messages), `@job` (usage list) | Swordman at B99: MaxHP 4038, MaxSP 210 (`3999 × 1.01`, `208 × 1.01`). Worn items stay (job change does not recheck `elvmax`). |
| 632-691, 761-820 | `@job 4307` (from Swordman; `@job 1` back at 696-755 in between) | Hyper Novice at B99, J1. |
| 825-849 | `@blevel 1` → **100** | **Mark comes off.** |
| 859-881 | `@blevel 1` → **101** | **Boots come off.** |
| 897-921 | `@blevel -2` → 99 | Nothing re-equips; no looks. |
| 930-973 | `@jlevel 51` → J52 | Observer sees `0x019b` effect 1. |
| 998-1025 | `@blevel 51` → 150 | |
| 1030-1059 | `@blevel 51` → 201 | |

**Forced unequip order (`@blevel +n`).** `pc_baselevelchanged` is the **last** thing the command does (`atcommand.cpp:1732`), after the tail, so the unequip packets are the last packets of the command. Server to actor, in order:

1. Recalc diff of the level change (the item has no stat effect, so there is no second diff after the unequip; `pc_unequipitem` flag 3 recalcs, and the recalc finds nothing to send).
2. Heal: `SP_HP`, `0x080e` (HP bar to self), `SP_SP`.
3. `0x019b` effect 0, the `0x0a24` achievement packets (2 at 100, none at 101), the `0x008e` message.
4. Tail: `SP_STATUSPOINT`, `SP_TRAITPOINT`, `SP_BASELEVEL`, `0x0acb` `SP_BASEEXP`, `SP_NEXTBASEEXP`.
5. The unequip, which differs by slot (`pc_unequipitem`, `pc.cpp:12398-12470`):
   * **Head top (B100, lines 848-849):** `0x099a` (`index 5`, position `0x100`, result 0) **then** `0x01d7` `LOOK_HEAD_TOP` (4) with view 0. The head looks are sent by `pc_set_costume_view` after the ack.
   * **Shoes (B101, lines 880-881):** `0x01d7` `LOOK_SHOES` (9) view 0 **then** `0x099a` (`index 6`, position `0x40`, result 0). The shoes look is sent before the ack.

   `0x099a` payload is `index:u16, position:u32, result:u8` (7 bytes).

Values (actor `#6`). **B100 (826-849):** diff `HIT` 289, `FLEE1` 214, `CRITICAL` 5, `MATK2` 46, `MATK1` 0, `MAXHP` 10588, `MAXSP` 392, then `HP` 10588, `SP` 392; tail `SP_STATUSPOINT` 1295, `SP_TRAITPOINT` 7, `SP_BASELEVEL` 100, `SP_NEXTBASEEXP` 1354202. **B101 (863-881):** `HIT` 290, `FLEE1` 215, `ATK1` 43, `DEF2` 10, `DEF1` 58, `MAXHP` 10691, `MAXSP` 399, `HP`, `SP`; tail 1318 / 7 / 101. The diff holds only changed internal units, so `ATK1` moves at 101 (not 100) and `MATK2` at 100 (both follow `level*10/4` truncation). B98 (443-460) and B99 (465-495) are exactly the captured `@blvl +n` order of [capture 1](#captured-stat-up-level-and-job-sequences): diff (`HIT`, `FLEE1`, `ATK1`/`DEF` pairs, `MAXHP`, `MAXSP`), `SP_HP`, `0x080e`, `SP_SP`, `0x019b`, (B99: ten `0x0a24` at 480-489), message, tail. No looks, no `0x099a`.

**Observer `#5` for the same commands** (it sees area packets only):

* Every `@blevel +n`: `0x080e` (`aid, hp, maxhp` full) and `0x019b` `aid, effect 0` (lines 444-445, 467/469, 826/829, 860-861, 1024-1025, 1031-1032). No `0x0977`, no stat or level packet.
* Mark off: one `0x01d7` `LOOK_HEAD_TOP` (4) value 0 (831). No `0x099a`.
* Boots off: one `0x01d7` `LOOK_SHOES` (9) value 0 (862). No `0x099a`.
* Equip: boots show as `0x01d7` type 9 view 22012 (425), Mark as type 4 view 1348 (428). No `0x0999` and no `0x019b`.
* `@blevel -n` (898, 901): only `0x080e` (twice: HP clamp and the second HP push).
* `@job` (506-515, 633-659, 697-720, 762-771): ten `0x01d7`, nothing else (as in capture 2). Job 4307 sends `LOOK_BASE`/`LOOK_BODY2` view 4307 (`0x10d3`).

**`@jlevel 51`** (930; J1 → J52). Observer `#5`: `0x019b` `aid, effect 1` (931) and nothing else. It does not get the job-level or skill-point pushes. Actor `#6`: `0x019b` effect 1 (932), four `0x0a24` (933-936), message (937), `SP_JOBLEVEL` 52, `0x0acb` `SP_JOBEXP` 0, `SP_NEXTJOBEXP`, `SP_SKILLPOINT` 51 (941), then the diff (942-973): `0x0141` ×6 for STR..LUK (bonus 20/15/16/20/15/16), `HIT` 311, `FLEE1` 234, `ASPD` 560, `ATK1` 99, `DEF`/`MATK`/`MDEF` pairs, `CRITICAL`, `MAXHP` 11050, `MAXSP` 419, then the six trait `0x0141` (POW..CRT bonus 9/4/4/8/8/3, 962-967), then `SP_PATK`..`SP_CRATE` (`0xe1`-`0xe6`, 968-973: 4, 3, 9, 9, 3, 1). The order matches capture 1's `@jlvl +n` plus the achievements; the trait block comes last. There is no HP/SP push, so the observer's HP bar keeps the old max until the next regen `0x080e` (979).

**Hyper Novice jobs.** `@job 4307` at B99 (632-691): `0x00be` ×6 (cost 2), `SP_TRAITPOINT` 7, trait `0x00be` ×6 (value 1), `SP_JOBLEVEL` 1, job exp 0 and next 4700000, four looks, `0x010f` (1521 bytes), `SP_MAXWEIGHT` 40300, `0x0141` ×6 (base 1, bonus 11/11/10/10/10/10), diff (`ASPD` 570, `MAXHP` 10483, `MAXSP` 385), six equip looks (Mark and boots still worn: top view 1348, shoes 22012), message. Back to Swordman (`@job 1`, 696-755): `0x00be` ×6, `SP_TRAITPOINT` 0, trait `0x00be` ×6 (value 0), `SP_JOBLEVEL` 1, `SP_MAXWEIGHT` 28300, `0x0141` bonus 0, `MAXHP` 4038, `MAXSP` 210.

**No buff.** No `0x0983`, `0x043f`, `0x0196` or `0x02c2` anywhere in the capture, and no `SP_ASPD` push outside a recalc diff (`@job` 531, 673, 737, 802; `@jlevel` 950). The Super Novice angel buffs run only in the exp-driven `pc_checkbaselevelup` (`pc.cpp:8279-8283`), not in `@blevel`, so none is expected.

**Other facts.**

* `0x0446` (`ZC_QUEST_NOTIFY_EFFECT`: `npc_id:u32, x:u16, y:u16, effect:u16, color:u16`) appears at login (279-280, two NPCs) and once at B150 (1023, NPC at (121,77), after the tail, from `pc_show_questinfo` at the end of `pc_baselevelchanged`). It is not sent to the observer. Out of scope for the stats feature.
* `0x00b0` `SP_MAXAP` (233, `0xe9`) = 200 arrives in the B201 diff (1045), right after `MAXHP` and `MAXSP` and before the HP push; `SP_AP` is not pushed (the heal's AP rate is 0). It is absent at B150.
* `SP_TRAITPOINT` in the tail is the running total (7 from the job change; 10 at B201, +3 from the trait table).
* `SP_STATUSPOINT` at 100/101/150/201 is 1295 / 1318 / 2545 / 4099, the `statpoint.yml` totals.
* Over-the-cap `@blevel +n` (465) sends one `+1` (98 → 99), the normal order, one message.

## Open points

* HP/SP/zeny/levels at login: `LoadEndAck` contains no `SP_HP`/`SP_ZENY`/`SP_BASELEVEL`; only the pre-ready diff push (above) sends HP/SP. Whether the 20260219 client needs `SP_ZENY`/levels pushed (rather than reading them from the `HC_ACCEPT_ENTER`/char-select data) should be checked with a capture. The proxy capture directory is not present in this checkout.
* `SP_ASPD` payload is `amotion`; client conversion `200 - amotion/10` is from long-standing client behaviour, not from rAthena source.
