# Job Change: `@job` / `@jobchange` and `pc_jobchange` (renewal, PACKETVER 20260219)

Source: rAthena `$RATHENA_PATH` (checked-out HEAD, read 2026-10-25). Files: `src/map/atcommand.cpp` (`ACMD_FUNC(jobchange)` 1203-1264), `src/map/pc.cpp` (`pc_jobchange` 10846-11088, `pc_equiplookall` 11097, `pc_calc_skilltree` 2601, `pc_checkallowskill` 7434, `pc_isequip` 1850, `pc_unequipitem` 12398, `pc_unequipitem_sub` 12311), `src/map/clif.cpp` (`clif_updatestatus` 3637, `clif_changelook` 3965, `clif_unequipitemack` 4338, `clif_skillinfoblock` 5696), `conf/atcommands.yml:677`, `conf/groups.yml`, `conf/msg_conf/map_msg.conf`, `conf/battle/player.conf`. Applies to `20260219` renewal. Equipment-side checks (`pc_isequip`, mapid tables, `class_` concept): [equipment.md section 3a](./equipment.md). Exp/level concepts: [experience.md](./experience.md).

## 1. `@jobchange` atcommand

* Registered name `jobchange`, alias `job` (`conf/atcommands.yml:677-679`). Neither appears in any `Commands:` list of the default `conf/groups.yml`, so only groups with `all_commands` (Admin, id 99) can use it. Non-permitted players get the public-chat fallback (see [atcommands.md](./atcommands.md) step 8).
* Argument: `"%11d %11d"` -> `job` (numeric job id, `e_job`) and optional `upper` (1 = transcendent/`JOBL_UPPER`, 2 = baby/`JOBL_BABY`; any other value = no flag). If the first token does not parse as an int (or message empty), the whole message is matched case-insensitively (`strncmpi`, 16 chars) against `job_name(i)` for i in the basic range (`JOB_NOVICE`..`JOB_MAX_BASIC`), then `JOB_NOVICE_HIGH`..`JOB_MAX`. Upper stays 0 for name lookups (the name itself names the high/baby job). No match: shows the command help text, returns -1 (framework then prints msg 154 `"%s failed."`).
* Deny list (direct dummy jobs: `KNIGHT2`, `CRUSADER2`, `WEDDING`, `XMAS`, `SUMMER`, `HANBOK`, `OKTOBERFEST`, `SUMMER2`, the `*2` mounted variants of Lord Knight/Paladin/Baby/3rd/4th classes, `STAR_GLADIATOR2`, `STAR_EMPEROR2`, `SKY_EMPEROR2`, ...): msg 923 `"You can not change to this job by command."`, returns 0.
* Then `pcdb_checkid(job)` (valid job id) false: help text, return -1. Else `pc_jobchange(sd, job, upper)`: true -> msg 12 `"Your job has been changed."`; false -> msg 155 `"You are unable to change your job."` and return -1 (then msg 154 too).
* All messages go through `clif_displaymessage` (`0x008e`, see [packets/chat.md](../packets/chat.md)).

## 2. `pc_jobchange(sd, job, upper)` order of operations

Failure returns (false, nothing changed): `job < 0`; `pc_jobid2mapid(job)` invalid; `pc_mapid2jobid(mapid | upper flag, sex)` invalid (this also swaps Bard<->Dancer to the character's sex); resulting mapid equals current `sd->class_` ("nothing to change"); job id not present in `job_db`.

Steps (after validation):
1. Record class-change job levels in char-global vars: to 4th: `change_level_4th = job_level`; else to 3rd: `change_level_3rd`; else 1st->2nd (`JOBL_2` newly set): `change_level_2nd` (stored via `pc_setglobalreg` as `JOBCHANGE4TH_VAR`/`JOBCHANGE3RD_VAR`/`JOBCHANGE2ND_VAR`). These feed skill-point reconciliation in `pc_calc_skilltree` (`pc.cpp:2894-2975`).
2. Reset Plagiarism/Reproduce clones (`pc_skill_plagiarism_reset`).
3. If the second-class mapid group changed (`MAPID_SECONDMASK`): `changeSkillTree` ends every active SC belonging to skills of the **current** class's skill tree (`status.cpp:1103`).
4. Leaving Star Gladiator lineage: `pc_resetfeel`, `pc_resethate`.
5. `status.class_ = job`; `status.body = class_` (reset body style); `sd->class_ = new mapid`.
6. **`job_level = 1`, `job_exp = 0`** (always).
7. If `base_level > pc_maxbaselv(new job)`: base_level capped to that max, `base_exp = 0`, `pc_resetstate` (stat reset), packets `SP_STATUSPOINT`, `SP_TRAITPOINT`, `SP_BASELEVEL`, `SP_BASEEXP`, `SP_NEXTBASEEXP`. Otherwise **base level, base exp, stats and status points are untouched**.
8. Transcendent points: non-T -> T (also primary 4th) adds `transcendent_status_points` (default 52, `player.conf:125`) to `status_point` and sends `SP_STATUSPOINT`; T -> non-T subtracts it, or if fewer points than that remain, `pc_resetstate`.
9. Always sends `SP_USTR, SP_UAGI, SP_UVIT, SP_UINT, SP_UDEX, SP_ULUK` (stat-raise cost indicators).
10. Trait jobs (4th-class lineage): non-trait -> trait adds `trait_points_job_change` (default 7, `player.conf:305`), sends `SP_TRAITPOINT` + `SP_UPOW/USTA/UWIS/USPL/UCON/UCRT`; trait -> non-trait zeroes `trait_point` (or `pc_resetstate` if already spent) and sends the same.
11. Sends `SP_JOBLEVEL`, `SP_JOBEXP`, `SP_NEXTJOBEXP` (`0x00b0` for level, `0x0acb` for the two exp vars; var ids in [experience.md](./experience.md)). **Skill points are not modified and `SP_SKILLPOINT` is not sent here** (it is only reduced/zeroed by `pc_calc_skilltree` side effects, if at all; leftover skill points carry over).
12. **Equipment re-check**: loop `i in 0..EQI_MAX`; for each `equip_index[i] >= 0` with `pc_isequip(sd, idx) != ITEM_EQUIP_ACK_OK` call `pc_unequipitem(sd, idx, 2)`. `pc_isequip` is the standard check (level, sex, job, class; GM `use_all_equipment` bypass; see equipment.md 3a), run against the **new** `sd->class_`. Flag 2 = force unequip: skips the "cannot unequip" status gate (`sc.cant.unequip`), no immediate `status_calc_pc` (flag 1 not set; calc runs at step 21), but `pc_checkallowskill` still runs (flag 4 not set) and the ammo-follows-weapon rule applies. Per item the normal unequip packets go out (section 3). A two-handed or multi-slot item appears in several `equip_index` entries; the second visit is a no-op because `equip` is already 0 and the index is -1.
13. If disguised, undisguise (`pc_disguise(sd, 0)`).
14. `status_set_viewdata(sd, job)`; then look packets (`0x01d7` ZC_SPRITE_CHANGE2, `AREA`, see [packets/equipment.md](../packets/equipment.md#0x01d7-zc_sprite_change2-look-change)) in this order: `LOOK_BASE` (= new class id; suppressed if invisible job; internally also refreshes weapon/shield look cache), `LOOK_HAIR` (PACKETVER >= 20151001, always for us), `LOOK_CLOTHES_COLOR`, `LOOK_BODY2`. The source comments that the sprite update is intentionally after the unequip loop to avoid client crashes with incompatible equipment.
15. `pc_calc_skilltree` (rebuilds `status.skill[]` from the new job's skill tree; keeps learned levels; drops skills not in the tree; with `player_skillup_limit: yes` (default, `player.conf:35`) and `player_skillfree: no`, skills for a tree are only exposed if the spent points justify the class, via `pc_calc_skilltree_normalize_job`), then `clif_skillinfoblock`: full skill list `ZC_SKILLINFO_LIST` **`0x010f`** (37-byte name-form entries; the `0x0b32` form is gated `PACKETVER_RE_NUM >= 20190807`, false for our client, and the login capture shows `0x010f`: [packets/settings.md](../packets/settings.md#0x010f-skill-entry-37-bytes-name-form)), dynamic length.
16. Delete elemental; close vending / buying store; kill clones (`jobchange_killclone`).
17. Option cleanup: strip Riding (needs `KN_RIDING`), Falcon (`HT_FALCON`), Dragon (`RK_DRAGONTRAINING`), Warg rider and Warg (`RA_WUGMASTERY`), Mado gear (always removed), cart (needs `MC_PUSHCART`; NEW_CARTS uses `SC_PUSH_CART` -> `pc_setcart(0)`), checked against the **new** skill levels; if the option bits changed, `pc_setoption` (sends option update packet). Homunculus vaporized if no `AM_CALLHOMUN`; Sprite Mable/Soul Attack SC ended without their skill; `SC_SPIRIT` ended.
18. If `manner < 0`: `clif_changemanner` + `SP_MANNER`.
19. `status_calc_pc(sd, SCO_FORCE)`: full recalc, sends changed params (max HP/SP, stats, weight/max weight, ASPD, etc.) by diff. **HP/SP are not healed**; current HP/SP are only clamped down if above the new max (`status.cpp:6215`). Weight is recomputed (item weights unchanged; `MaxWeight` of the job in `job_stats.yml` changes max weight and sends `SP_MAXWEIGHT`).
20. `pc_checkallowskill` (ends weapon/shield-requiring SCs now invalid), then `pc_equiplookall` (re-sends `LOOK_WEAPON` with 0, `LOOK_SHOES` 0, `HEAD_BOTTOM`, `HEAD_TOP`, `HEAD_MID`, `ROBE`; `LOOK_WEAPON`/`SHIELD` look is rebuilt from `vd->look`). Then `pc_show_questinfo`, achievement `AG_JOB_CHANGE`.
21. Party: member `class_` updated and `clif_party_job_and_level`.
22. **`chrif_save(sd, CSAVE_NORMAL)`**: immediate full char save to char-server (see section 4). Then fame list rebuild if fame class changed.

No `0x019b` level-up effect is sent by job change (that is only for level-ups). No heal, no `SP_HP` forced send beyond the recalc diff.

## 3. Unequip packet flow for a forced item (`pc_unequipitem(sd, n, 2)`)

Order inside (`pc.cpp:12398-12525`): clear `equip_index` entries for the item's slots; by slot: weapon (`EQP_HAND_R`) resets `weapontype1`, recomputes `status.weapon`, sends `LOOK_WEAPON`; shield (`EQP_HAND_L`) sets shield 0, sends `LOOK_SHIELD` (`clif_changelook` for weapon/shield sends one `0x01d7` carrying both weapon and shield view, `val2` = shield); shoes send `LOOK_SHOES` 0; then **`clif_unequipitemack` -> `0x099a` ZC_REQ_TAKEOFF_EQUIP_ACK** (`index`, `wearLocation = pos`, `flag`; for PACKETVER >= 20110824 the wire flag is inverted so success = 0; see [packets/equipment.md](../packets/equipment.md#0x099a-zc_req_takeoff_equip_ack-v5)); `pc_set_costume_view`; remove statuses flagged `REMOVEONUNEQUIP`; for weapons with `ammo_unequip` battle config (flag 4 not set) the ammo is unequipped with its own `0x099a`; then `pc_unequipitem_sub`: clears `equip`, `pc_checkallowskill`, combo removal, runs unequip scripts. Headgear looks (`HEAD_*`, `ROBE`) are re-sent afterwards by `pc_equiplookall` in step 20 (head views are updated by `pc_set_costume_view`/equiplookall, not by the unequip function itself). So per forced item: optionally `0x01d7` (weapon/shield/shoes) then `0x099a`; no `0x0999`.

## 4. Persistence

* `pc_jobchange` ends with an immediate `chrif_save(sd, CSAVE_NORMAL)` (full `mmo_charstatus` struct, packet `0x2b01` map->char; not a login-time-only save). Changed fields that matter in the `char` table: `class` (= `status.class_`), `base_level`, `base_exp`, `job_level` (reset to 1), `job_exp` (0), `status_point`, `skill_point`, str..luk, `trait` points (when applicable), `body` (reset to class), equipment (`equip` flags in `inventory`), skills (`skill` table rows; tree-derived temporary skills flagged `SKILL_FLAG_TEMPORARY` are not saved), `hp`/`sp` current values.
* Char-global vars `jobchange_level` style values (`JOBCHANGE2ND_VAR`, `JOBCHANGE3RD_VAR`, `JOBCHANGE4TH_VAR`) are `char_reg_num` entries (`pc_setglobalreg`), written with the char save.
* Autosave also persists later, as usual; nothing job-specific.

## 5. Concepts used

* `pc_jobid2mapid(job)` / `pc_mapid2jobid(mapid, sex)`: job id <-> `MAPID_*` bits (`JOBL_UPPER` 0x1000, `JOBL_BABY` 0x2000, `JOBL_2/3/4`, `MAPID_FIRSTMASK`, `MAPID_SECONDMASK`); tables in [equipment.md 3a](./equipment.md#job-id---mapid-sdclass_).
* `sd->class_` is the mapid; `sd->status.class_` is the job id sent to clients as `LOOK_BASE`/class.
* `pc_maxbaselv`/`MaxJobLevel` come from `job_exp.yml` groups ([db/exp_tables.md](../db/exp_tables.md)).
* Metaling reuse note: a minimal `@job <id>` can skip steps 1-4, 8, 10, 15-18, 21 initially, but must do steps 5-7, 9, 11, 12, 14, 19-20 and save.

## Open points

* **Resolved by capture** ([captures/attribute_sequence.txt](../captures/attribute_sequence.txt), table in [packets/status.md](../packets/status.md#captured-stat-up-level-and-job-sequences)). Section 2's order holds, including steps 7, 8 and 10 and the forced unequip in step 12 (two `0x01d7` for a two-hander, then `0x099a`). Also confirmed:
  * `SP_SKILLPOINT` is not sent.
  * `0x010f` is re-sent after the looks.
  * `SP_HP` appears only as a clamp push, when a `pc_resetstate` lowered MaxHP.
  * No heal and no `0x019b`.
  * The success message comes last.
* Observer's view of the area `0x01d7` packets: captured in capture 2 (below).

**Capture 2** ([captures/attribute_sequence_2.txt](../captures/attribute_sequence_2.txt), write-up in [packets/status.md](../packets/status.md#capture-2-observer-stat-reset-failure-and-4th-class-jobs)). Applies to 20260219.

* **Observer** (lines 532-556, 1204-1213, 1263-1288, 1349-1358): the area gets ten `0x01d7`: `LOOK_BASE`, `LOOK_HAIR`, `LOOK_CLOTHES_COLOR`, `LOOK_BODY2` (step 14), then the six `pc_equiplookall` looks (step 20), all in the order listed. The observer gets nothing else (no skill list, no status packets, no message).
* **Trans points** (step 8): Novice High (4001) adds 52 (`SP_STATUSPOINT` 64 → 116, line 1214); Super Novice (23) from Novice High subtracts 52 (116 → 64, line 1264) with no reset because 64 points remained.
* **Trait points** (step 10): Hyper Novice (4307) from Super Novice gives `SP_TRAITPOINT` 7 and the six trait `0x00be` (lines 1365-1371) but **no** `SP_STATUSPOINT` (Hyper Novice is not a primary 4th class). The order is step 9's six `0x00be`, then `SP_TRAITPOINT`, then the trait `0x00be`.
* **Wrong name** (`@job supernovice`, line 1134): usage and job list as 59 `0x008e` lines, the failure message, nothing else sent, no state change.
* **No forced unequip** (step 12) was exercised: the Tsurugi was off before the 4001/23/4307 changes. Merchant took it fine (equip at 611 after the Knight → Merchant change).
* **MaxHP/MaxSP** after each job (diff only, no heal, no `0x00bd`): Merchant 2400/300 (B96), Novice High 82/20 (B6), Super Novice 65/16 (B6), 40/11 (B1), Hyper Novice 99/11 (B1). Derivations and the Hyper Novice data gap (no +2000/+4000 Super Novice HP in `job_db.yml`) are in [stats.md](./stats.md#capture-2-values).
* **Hyper Novice stat bonus**: `0x0141` bonuses 11/11/10/10/10/10 at job level 1 (lines 1381-1386) are the `BonusStats` level-1 entry plus the Super Novice +10 rule (`status.cpp:4268`), which applies to the whole Super Novice line with `JOBL_THIRD`.
