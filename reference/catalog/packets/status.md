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

`CZ_STATUS_CHANGE` `0x00bb`: `var:u16` at offset 2 (`SP_STR`=13 .. `SP_LUK`=18), `amount:u8` at offset 4 (`parseable_packet(0x00bb, 5, clif_parse_StatusUp, 2, 4)`). Clients 2013-12-23 and newer send the real amount (old clients always 1); a negative `i8` only logs a debug message and then `pc_statusup` rejects it (`increase <= 0`).

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
* HP/SP clamps: when `max_hp`/`max_sp` shrinks below current, HP/SP are clamped and `SP_HP`/`SP_SP` are pushed (`status.cpp:6198-6225`).
* Weight is not part of this diff; `status_calc_weight` pushes `SP_WEIGHT` and `SP_MAXWEIGHT` itself only when they change (`status.cpp:3693-3697`), and `pc_additem`/`pc_delitem` push `SP_WEIGHT`.
* Level-up, job change, stat-up, equip/unequip, skill learn/passives, buffs/debuffs and many script `bonus` effects all call `status_calc_pc`. Level-up flow: [../mechanics/experience.md](../mechanics/experience.md#level-up).

## Where the status packets sit in the map-entry order

> Open findings: [PF-12](../../../PACKET_FINDINGS.md#pf-12).

Do not duplicate; see [map_entry_actions.md](../mechanics/map_entry_actions.md#map-entry-cz_notify_actorinit-clif_parse_loadendack) step list. Summary of what this file adds, in `clif_parse_LoadEndAck` order (`clif.cpp:10748-11110`):

1. Always: `0x00b0` `SP_WEIGHT`, `SP_MAXWEIGHT` (`10793-10794`), before the spawn and nearby units.
2. First login only (`state.connect_new`, `10904-10988`): `SP_BASEEXP`, `SP_NEXTBASEEXP`, `SP_JOBEXP`, `SP_NEXTJOBEXP` (`0x0acb`), `SP_SKILLPOINT` (`0x00b0`), then `clif_initialstatus` = `0x00bd` + the follow-up `0x0141`/`0x013a`/`0x00b0`/`0x00be` packets listed under ZC_STATUS. `SP_STATUSPOINT`, HP/SP, level and zeny are NOT in this block.
3. Map change (not first login, `10989-11003`): `0x0141` for STR..LUK and (renewal) POW..CRT only. No `0x00bd`, no exp, no weight beyond step 1, no HP/SP.
4. Changemap-only content later calls `status_calc_pc(SCO_NONE)` (`clif.cpp:11092`), so any changed derived value gets a diff push.

## Open points

* HP/SP/zeny/levels at login: `LoadEndAck` contains no `SP_HP`/`SP_ZENY`/`SP_BASELEVEL`; only the pre-ready diff push (above) sends HP/SP. Whether the 20260219 client needs `SP_ZENY`/levels pushed (rather than reading them from the `HC_ACCEPT_ENTER`/char-select data) should be checked with a capture. The proxy capture directory is not present in this checkout.
* `SP_ASPD` payload is `amotion`; client conversion `200 - amotion/10` is from long-standing client behaviour, not from rAthena source.
