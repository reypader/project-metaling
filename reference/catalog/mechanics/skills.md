# Skills (learn-only): skill-up, skill list, reset, job change, persistence (renewal, PACKETVER 20260219)

Source: rAthena `$RATHENA_PATH` at `d4b8e7b8f`. Files (all under `src/map/` unless noted): `pc.cpp` (`pc_skillup` 9129, `pc_allskillup` 9183, `pc_resetskill` 9428, `pc_calc_skilltree` 2601, `pc_check_skilltree` 2802, `pc_clean_skilltree` 2877, `pc_calc_skilltree_normalize_job_sub` 2893, `pc_calc_skilltree_normalize_job` 2979, `pc_calc_skillpoint` 2518, `pc_grant_allskills` 2539, job level-up 8325), `clif.cpp` (`clif_skillinfoblock` 5696, `clif_deleteskill` 5810, `clif_skillup` 5832, `clif_parse_SkillUp` 12757, LoadEndAck 10905), `status.cpp` (`status_calc_pc_` 3755-3769, diff block 4973-4983), `atcommand.cpp` (`joblevelup` 1745, `allskill` 3805, `reset` 7666, `resetskill` 9658), `src/char/char.cpp` (save 418-455, load 1168-1195), `sql-files/main.sql:1045`, `conf/battle/player.conf:28-44`, `conf/atcommands.yml`, `conf/msg_conf/map_msg.conf`. Packet layouts (`0x010f`, `0x010e`, `0x0441`): [packets/settings.md](../packets/settings.md#packet-reference). Tree/DB schemas (`skill_db.yml`, `skill_tree.yml`, Inherit, Requires, MaxLevel): see `db/skills.md` (written separately). Related: [job_change.md](./job_change.md), [experience.md](./experience.md), [atcommands.md](./atcommands.md) (`@jlvl`). All rules apply to `20260219` unless noted; not packet-version dependent except the list/update opcode forms.

## 1. Data model (rules we need)

* Per skill slot: `id` (0 = not in the visible tree), `lv`, `flag`. Flags: `0` PERMANENT (learned by points), `1` TEMPORARY (item/status-granted, never saved), `2` PLAGIARIZED, `3` PERM_GRANTED (script-granted, saved with flag 3), `>= 10` REPLACED_LV_0+N (a temporary grant overshadows a learned level N). (`src/common/mmo.hpp:383-391`)
* `id` is the "visible/learnable now" bit. `lv` can stay > 0 while `id == 0` (skills of a tree you no longer qualify for keep their level in memory but are hidden; see section 7 for the save consequence).
* Config defaults (`player.conf`): `player_skillfree: no`, `player_skillup_limit: yes`, `quest_skill_learn: no`, `quest_skill_reset: no`.
* Skill points: `status.skill_point` (unspent). +1 per job level gained (`pc.cpp:8335`, level-up; `@jlvl` +n adds n). Spent points = sum of `lv` over permanent skills (`pc_calc_skillpoint`: skills with `id != 0 && lv > 0`, excluding quest skills (unless `quest_skill_learn`) and wedding skills, flag 0 counts `lv`, flag >= 10 counts `flag - 10`; NV_BASIC included since the loop starts at index 1).

## 2. `0x0112 CZ_UPGRADE_SKILLLEVEL` -> `pc_skillup(sd, skillId)`

Request: `0x0112 skillId:u16` (4 bytes; `clif_packetdb.hpp:110`, offset 2). No other gating: `clif_parse` only requires an active session; **there is no dead/casting/vending/trading/sitting check** in the handler.

Validation, in order (`pc.cpp:9129-9160`). Any failure is **silent**: no packet, no message, no state change (only the first rejects log an error server-side).
1. `skill_get_index(skillId) != 0` (unknown id: ignored; id 0 ignored).
2. Guild skills (`SKILL_CHK_GUILD`) diverge to `guild_skillup`; homunculus skills (only if a homunculus is out) to `hom_skillup`. (Out of scope here.)
3. `skill_point > 0`.
4. `status.skill[idx].id != 0`: the skill is currently in the visible tree. This single bit encodes prerequisites, base/job level requirements, class membership, and the `skillup_limit` rule (all computed by `pc_calc_skilltree`/`pc_check_skilltree`, section 3). `pc_skillup` itself does **not** re-check prerequisites or levels.
5. `flag == PERMANENT`: refuses while the skill is temporarily/permanently granted (item, script, plagiarism).
6. `lv < skill_tree_get_max(skillId, status.class_)`: max level from the class's own tree entry (`MaxLevel` in `skill_tree.yml`, inherited trees included), falling back to the global `skill_get_max` if the class tree has no entry.

Success (`pc.cpp:9140-9180`), exact order:
1. `skill.lv += 1`, `skill_point -= 1`.
2. Recalc, exactly one of: (a) skill is passive (`skill_get_inf == 0`) or Summoner Power of Land/Sea >= 20: `status_calc_pc(SCO_NONE)`; (b) else if `skill_point == 0` and Taekwon Ranker: `pc_calc_skilltree`; (c) else `pc_check_skilltree` (silent; may set `id` on newly unlocked skills, repeating until stable).
3. Packets to the client only:
   * (passive case only, from inside `status_calc_pc`): stat diff packets, if the passive changes stats/derived values (e.g. `0x0141` bonus + `0x00b0` HIT/ATK/etc. for Owl's Eye); then, **only if the visible tree changed** (a `memcmp` of the skill array taken at the start of that recalc, which already includes the incremented level), a full `0x010f` re-send (and `0x0441` per vanished skill at `PACKETVER >= 20190807`, true here).
   * `0x010e ZC_SKILLINFO_UPDATE` (`skillId, newLv, sp = skill_get_sp(id,lv), range2, upFlag = lv < max`).
   * `0x00b0` `SP_SKILLPOINT` (var `0x0c`) with the new unspent points.
   * (`GN_REMODELING_CART` only: `SP_CARTINFO`.) (`SG_DEVIL` known + max job level: a status icon.)
   * Full `0x010f` skill list, **unless the character has `all_skill` permission**. So for normal players every accepted skill-up ends with a full re-send.
4. No save, no `chrif_save` (section 8).

So the per-skill-up wire order is: `[stat diff packets] [0x010f if tree grew/shrank, passive only] 0x010e, 0x00b0(0x0c), 0x010f`. Active-skill ups never produce the first two groups: newly unlocked skills first appear in the trailing `0x010f` (`pc_check_skilltree` sends nothing itself). `upFlag` in the list is `flag == PERMANENT && lv < max`.

## 3. How the visible tree is computed (what makes `id != 0`)

`pc_calc_skilltree` (`pc.cpp:2601`), run at login (`SCO_FIRST`), on every `status_calc_pc` (`status.cpp:3769`: job change, level-ups, `@jlvl`, equip changes, ...), allskill, etc.:
1. Compute the *effective* job for tree lookup via `pc_calc_skilltree_normalize_job` (only when `player_skillup_limit` is on and no `all_skill` permission). Uses spent points `P = pc_calc_skillpoint` **as currently marked visible**:
   * Non-Summoner: if `P < Novice max_job_level - 1` (9 with the default 10-level novice) the effective job is **Novice**: only NV_BASIC (and quest-gated novice skills) can be levelled until 9 points are spent. Else `P -= 9`.
   * Second class (not Super Novice): `change_level_2nd` = job level at the moment of the 1st->2nd change (char var `JOBCHANGE2ND_VAR`; default = 1st class `max_job_level` if unset). If `P < change_level_2nd - 1` the effective job is the **1st class**; else `P -= change_level_2nd - 1`. Same pattern for 3rd (`change_level_3rd`) and 4th (`change_level_4th`) class. Upper/Baby bits are re-added when the resulting job exists. Primary 4th jobs may use Trans trees.
2. Clear `id` of every slot except plagiarized/perm-granted; restore original levels of overshadowed (flag >= 10) skills; drop Taekwon Ranker bonus skills.
3. Iterate the effective job's tree (Inherit-expanded) to a fixed point: a skill becomes visible (`id` set) when every `Requires` entry is met (required skill learned at a *real* permanent level >= N), and `base_level >= BaseLevel`, `job_level >= JobLevel` of the entry (these two checks are skipped when `player_skillfree: yes`). Quest skills (`inf2 QUEST`), wedding skills, and Spirit skills (without `SC_SPIRIT`) are **not** made visible unless they already have `lv > 0`.
4. `pc_check_skilltree` is the lightweight version used after an active skill-up: same fixed-point loop, only *adds* visibility (does not clear), checks prerequisites + base/job level, no normalization.

## 4. Capture decode: `equip_sequence_4.txt` (session `#3`)

[captures/equip_sequence_4.txt](../captures/equip_sequence_4.txt) (rAthena trace, 20260219). Numbers below are the capture's own line labels (first column), not file line numbers. Session `#3` = account 2000001, female. At line 1868 it did `@job dancer` (its tree then resolves to Dancer: only Dancer has `DC_DANCINGLESSON`). It had 9 unspent skill points at line 2006.

| Lines | Client | Server |
| :--- | :--- | :--- |
| 2006-2014 | 9 x `0x0112 skill 1` (NV_BASIC), same millisecond | 9 accepted, one reply group each. Points shown by `SP_SKILLPOINT`: 8,7,6,5,4,3,2,1,0 |
| 2114-2130 | 17 x `0x0112` (323, 304, 325 x3, 327 x11, 306, 312) | **No reply at all** (`skill_point == 0`; the ids are also not visible) |
| 2147-2153 | 7 x `0x0112` (323 x3, 324 x4) | **No reply** (0 points) |
| 2073-2082 | `@jlvl` (chat) | job level 1 -> 50: `SP_JOBLEVEL`... then `SP_SKILLPOINT` 49 (line 2082), no `0x010f` |
| 2166-2176 | 11 x `0x0112`: 43 x3, 44, 45, 46 x5, 47 | 11 accepted (points 49 -> 38). Requests are processed in order, so an unlock earned by an earlier request in the same burst validates the later ones (44 after 43 lv3; 47 after 46 lv5) |

Total: 44 `0x0112` requests in these four bursts; 20 accepted (9 + 11), 24 silently dropped (17 + 7). Refused requests produce zero bytes.

**Why `0x010f` becomes 115 bytes at NV_BASIC 9** (line 2039): frame = `4 + 37n`. Lists for NV_BASIC 1..8 are 41 bytes (n=1: effective job Novice). When the 9th point is spent, spent points `P = 9` reaches the Novice threshold, the effective job becomes Archer-line and the Archer tree appears. The 115-byte list (n=3) holds NV_BASIC lv 9 (`upFlag` 0, at max), **AC_OWL (43) lv 0 and AC_DOUBLE (46) lv 0** (both `upFlag` 1; the two Archer skills with no prerequisite). Vulture's Eye needs Owl's Eye 3, Concentration needs Vulture 1, Arrow Shower needs Double Strafe 5, so those are not yet visible. The same 115-byte list is sent twice (2039 and 2042), around `0x010e`.

Order observed for NV_BASIC 9 (passive, tree grows): `0x010f`(115, from `status_calc_pc` tree diff) -> `0x010e skill 1 lv 9 upFlag 0` -> `0x00b0 0x0c = 0` -> `0x010f`(115). For NV_BASIC 1..8 (tree unchanged): `0x010e`, `0x00b0 0x0c`, `0x010f`(41).

Dancer skill-ups (lines 2166-2222), showing the passive vs active difference:
* **Passive (inf 0) AC_OWL**: 2177 `0x0141` DEX bonus and `0x00b0` HIT/ATK/etc. diffs *first*, then `0x010e 43 lv1`, `SP_SKILLPOINT`, `0x010f`(115: 1, 43, 46). At OWL lv3 (2191): an extra `0x010f`(152: adds AC_VULTURE 44 lv 0) *before* `0x010e 43 lv3` (tree grew), then the usual trailing list.
* **Passive AC_VULTURE lv1** (2198): extra `0x010f`(189) adds AC_CONCENTRATION (45) *before* its `0x010e`.
* **Active AC_CONCENTRATION / AC_DOUBLE**: only `0x010e` -> `0x00b0 0x0c` -> `0x010f`; `0x010e` carries `sp` (Concentration 25, Double 12) and `range2` (Double 10). At DOUBLE lv4 (2214) the trailing `0x010f` (259 bytes, n=7) adds **BD_ADAPTATION (304) and DC_DANCINGLESSON (323)**, at lv5 (2217) adds AC_SHOWER (47). This is the `normalize_job` rule: spent points = 9 + OWL 3 + VULTURE 1 + CONCENTRATION 1 + DOUBLE 4 = 18 = 9 + (`change_level_2nd` - 1), so `change_level_2nd` was 10 for this character (inference from the arithmetic; the Dancer tree is revealed exactly when spent reaches 18).
* `0x010f` frame sizes seen (4 + 37n): 41 (n=1), 115 (3), 152 (4), 189 (5), 263 (7), 300 (8). `upFlag` is 1 until lv == max.

Job change in this session (`@job 12`, Assassin, line 2591): after the usual stat/look packets and `0x099a`: `0x010f`(374 bytes: 1,48,49,50,52,132,134; Dancer line gone from the list), `0x00b0 0x19` (SP_MAXWEIGHT), **`0x0441` 132, `0x0441` 134**, `0x010f`(189: 1,48,49,50,52), then the `0x0141` stat diffs. No `SP_SKILLPOINT` anywhere.

## 5. `0x0441 ZC_SKILLINFO_DELETE`: what triggers it

`clif_deleteskill` (`clif.cpp:5810`) sends `0x0441 skillId:u16` and, only if `skip_infoblock` is false, a full `0x010f`. Callers: `status_calc_pc` tree diff (`status.cpp:4975-4983`, `skip_infoblock = true`, then one `clif_skillinfoblock`), item-granted skill removal (`pc.cpp:5459/5494/5576`, [equipment.md](./equipment.md); skill 1013 in equip captures), `atcommand.cpp:3901` (`@lostskill`), `chrif.cpp` (WE_CALLBABY). At 20260219 the first form applies: **one `0x0441` per skill whose `id` went from non-zero to 0 inside a `status_calc_pc`, followed by one `0x010f`**. Captured (section 10): this fires after `@skreset`, `@reset` and `@jlvl -n` resets, but **not** on any job change in `rathena_skills_sequence.txt` (Novice->Swordman->Knight->Mage->Knight: one `0x010f` only, no `0x0441`).

Why a job change emits them: `pc_jobchange` calls `pc_calc_skilltree` + `clif_skillinfoblock` at its step 15 ([job_change.md](./job_change.md)), then step 19 `status_calc_pc(SCO_FORCE)` recomputes the tree. The first pass can still list high-tier skills because `pc_calc_skillpoint` counted the old (still-visible) skills; the second pass trims them. This only happens for some changes (second/third-class lines, normalization overshoot); 1st/2nd-class swaps in `rathena_skills_sequence.txt` produce a single `0x010f` and no deletes (section 10). Observed in two captures (inference of the cause, the packet pattern is observed):
* `equip_sequence_4.txt` 2608-2612: Dancer -> Assassin: `0x010f` {1,48,49,50,52,132,134}, `0x00b0 0x19`, `0x0441` x2 (132, 134), `0x010f` {1,48,49,50,52}.
* `attribute_sequence.txt` Rune Knight -> Shadow Cross (3716-3723): `0x010f` n=10 {1,48,49,50,52,132,134,2021,2024,5014}, ..., `0x0441` x5 (132,134,2021,2024,5014), `0x010f` {1,48,49,50,52}. Also 4370-4387 (`0x0441` x17 then `0x010f` n=14 {1..7,55,56,58..62}, preceded by the new tree's previous list).
A minimal implementation can send just the final consistent list; to match the wire exactly, mimic "list, [other status diffs], deletes, list".

## 6. Skill reset: `pc_resetskill(sd, flag)` (`pc.cpp:9428`)

Flags: `1` = send packets + recalc; `2` = dry run, return spent total, change nothing; `4` = only reset Bard/Dancer change-sex skills (returns 0 unless class is Bard/Dancer line); `0` = silent reset (caller sends).

Rules, in order:
1. Flag 4 with a non-Bard/Dancer class: return 0.
2. Unless flag 2: **Taekwon Ranker returns 0 immediately** (no reset). Then side-effects: remove SG_DEVIL blind status icon; strip options that need skills the player is about to lose (riding, falcon, dragon, warg, warg rider, mado, cart) via `pc_setoption`; vaporize an active homunculus when AM_CALLHOMUN known; end Sprite Mable / Soul Attack status.
3. For every skill in `skill_db` with `lv > 0`:
   * skip wedding/spirit (linker) skills;
   * `NV_TRICKDEAD` on a non-Novice: set `lv = 0`, flag PERMANENT, **not refunded** (platinum skill);
   * `NV_BASIC` on a non-Novice: **kept** untouched (never reset); on a Novice it is reset and refunded;
   * `flag == PERM_GRANTED`: skipped (kept);
   * flag-4 mode: skip anything that is not a change-sex skill;
   * quest skills (when `quest_skill_learn` is off): never refunded; wiped to 0 only if `quest_skill_reset` is on (and not a dry run), otherwise kept;
   * otherwise refund: flag PERMANENT adds `lv`; flag >= 10 adds `flag - 10` (the real learned level under a temporary grant); other flags (temporary) add nothing;
   * unless dry run: `lv = 0`, `flag = PERMANENT` (the `id` is left alone; recalculation fixes visibility).
4. `flag & 2` or total refund 0: return the total, no state change to `skill_point`.
5. `skill_point += total`. Only if `flag & 1`: `SP_SKILLPOINT` push, then full `0x010f`, then `status_calc_pc(SCO_FORCE)`. **No `0x0441`, no `0x010e` from the reset itself.** The recalc after the list emits its own deletes + second `0x010f` when the visible tree shrinks (captured for `@skreset`/`@reset`, section 10). With flag 0 (`@jlvl -n`) there is no first list: only the recalc's deletes + list.
Returns the refund. Total refund = sum of levels of all reset skills, so after a reset `skill_point` = previous unspent + all spent (not recomputed from job level).

Callers and flags: `@skreset`/`@resetskill` -> 1; `@reset` -> 1 (after stats reset); `@jlvl -n` -> 0 (only when `skill_point < n`, see below); `pc_resetlvl(type != 3)` (script `resetlvl`) -> 0 followed by its own full packet burst and `clif_skillinfoblock`; script `resetskill` -> 1; `@changesex` / `@changecharsex` and the script sex-change commands -> 4; script `skillpointcount` -> 2 (dry run).

## 7. Job change and skills

(Order of the whole `pc_jobchange` is in [job_change.md](./job_change.md); only skill points here.)
* **Skill levels are never cleared and skill points are never refunded** by `pc_jobchange`; `SP_SKILLPOINT` is not sent (confirmed in both captures).
* Step 1 records `change_level_2nd/3rd/4th = job_level` at the change (the thresholds in section 3), stored in char vars.
* Step 15 `pc_calc_skilltree` + `0x010f`; step 19 `status_calc_pc(SCO_FORCE)` may add `0x0441` deletes + a second `0x010f` (section 5).
* Skills outside the new job's (Inherit-expanded) tree get `id = 0` but keep `lv` in memory; changing back before the next save would show them again. See section 8 for what is persisted.
* Options that require skills not in the new tree are stripped (job_change step 17); active statuses of the old class's skills are ended (`changeSkillTree`, step 3).
* Job level resets to 1; the +1 point per job level is only given on level-up, so the points spent in the old class stay spent.

## 8. Job level changes, atcommands, persistence, login

**Job level up** (`pc.cpp:8325-8352`): `job_level++`, `skill_point++`; packets `SP_JOBLEVEL`, `SP_JOBEXP`, `SP_NEXTJOBEXP`, `SP_SKILLPOINT`; `status_calc_pc(SCO_FORCE)` (re-runs the tree: level-gated entries can unlock; a changed tree sends `0x010f`); then the level-up effect. No unconditional list re-send.

**`@jlvl +n`** (`atcommand.cpp:1745`): `job_level += n`, `skill_point += n`, effect; then `SP_JOBLEVEL`, `SP_JOBEXP`, `SP_NEXTJOBEXP`, `SP_SKILLPOINT`, `status_calc_pc(SCO_FORCE)`. List re-sent only if the visible tree changed (capture: none, line 2073-2082). **`@jlvl -n`**: `job_level -= n` (clamped to 1); if `skill_point < n`, `pc_resetskill(sd, 0)` (silent full reset, refund) then if still `< n` points become 0, else `skill_point -= n`; same four status pushes + recalc. Because `pc_resetskill` changed the levels *before* the recalc's snapshot, the client list is not re-sent unless the visible tree changes, so the client can show stale levels until the next list (known quirk). Captured (section 10): in the one observed case the tree did shrink, so deletes + a fresh zeroed list were sent (not stale); the unchanged-tree case is still unobserved.

**`@reset`** (`7666`, alias none): `pc_resetstate` (its stat/point pushes, [stats.md](./stats.md)), then `pc_resetskill(sd, 1)` (`SP_SKILLPOINT`, `0x010f`, recalc), then msg 208 via `0x008e`. Command listed in `atcommands.yml` as `reset`.
**`@skreset`** (`9658`, canonical `resetskill`, alias `skreset`): `pc_resetskill(sd, 1)`, then msg 206 (`%s` = char name). `@streset` / `resetstat` is the stat-only twin (msg 207).
**`@allskill`** (`3805`, canonical `allskill`, aliases `allskills`, `skillall`, `skillsall`): `pc_allskillup`, then `skill_point = 0`, `SP_SKILLPOINT` (0), then msg 76. `pc_allskillup` (`9183`): restore overshadowed levels; with `all_skill` permission (`pc_grant_allskills`, **false** for the stock Admin group, `groups.yml:231`) every non-NPC/guild skill gets max level; otherwise, every skill of `skill_tree_db[status.class_]` (not normalized, so the full Inherit-expanded tree) is set to the tree's `MaxLevel`, skipping quest skills (unless `quest_skill_learn`), wedding/spirit skills and `SG_DEVIL`. Then `status_calc_pc(SCO_NONE)` and an explicit full `0x010f`; so wire order: [recalc diffs], `0x010f`, `SP_SKILLPOINT 0`, `0x008e`. Because the spent total is now huge, the later normalization exposes the full tree. Captured as predicted: `0x010f`, `SP_SKILLPOINT 0`, `0x008e`, no recalc diffs, no deletes (section 10).
Permissions: none of `allskill`, `resetskill`, `reset` are in a default `groups.yml` `Commands:` list; only `all_commands` (Admin) can use them (same as `@job`, see [atcommands.md](./atcommands.md)). `@skpoint`/`skillpoint` (`2873`): adds to `skill_point` (saturating), msg 1011 on empty arg.

**Persistence** (`char.cpp:418-455`, `main.sql:1045`): table `skill(char_id, id, lv, flag)`, PK (`char_id`, `id`). On save the whole table is rewritten for the char (delete + insert) only if the in-memory array differs from the last-saved copy. Rows written: `id != 0` and `flag != TEMPORARY`; rows with `lv == 0` and flag PERMANENT/PERM_GRANTED are skipped; overshadowed slots (flag >= 10) are written with their real level (`flag - 10`) and flag 0; PERM_GRANTED keeps flag 3. **Skills with `id == 0` (hidden by a class change) are not written**, so their levels are lost at the next save (captured: the Knight levels were gone after Knight->Mage->relog->Knight, section 10). Load reads `id, lv, flag` (flag kept only if 3, else 0). `skill_point` is a column of `char`.
**When saved**: `pc_skillup` and the resets never call `chrif_save`. Triggers: periodic autosave (`autosave_time` 300 s default, `conf/map_athena.conf:91`), logout / map-server change, and the explicit `chrif_save(CSAVE_NORMAL)` at the end of `pc_jobchange` (`pc.cpp:11076`). `pc_clean_skilltree` (called before saving) folds overshadowed levels back and clears temporary/plagiarized slots.
**Login**: char server sends the `skill` rows inside the char data; on map entry `status_calc_pc(SCO_FIRST|SCO_FORCE)` (`intif.cpp:3505`) runs `pc_calc_skilltree` (so `id` visibility is rebuilt before the client sees anything), and on `LoadEndAck` of a new connection `clif_skillinfoblock` is sent, then `clif_hotkeys_send` (`0x0b20`, hotkeys may name skill ids; note only), `clif.cpp:10905-10910`. Not re-sent on later map changes of the same session. See [packets/settings.md](../packets/settings.md) for the login burst.

## 9. Messages used (ids in `map_msg.conf`; text not reproduced)

| Id | Used by |
| :--- | :--- |
| 76 | `@allskill` success |
| 206 | `@skreset` success (`%s` name) |
| 207 | `@streset` success |
| 208 | `@reset` success (`%s` name) |
| 1011 | `@skpoint` usage |
| 23 / 24 / 25 / 159 / 987 | `@jlvl` (see [atcommands.md](./atcommands.md)) |

## 10. Capture: skills (rathena_skills_sequence.txt)

Source: [captures/rathena_skills_sequence.txt](../captures/rathena_skills_sequence.txt), real rAthena `d4b8e7b`, 20260219, recorded 2026-10-29 with `PROXY_FULL_PAYLOAD=1`. File line = proxy `#` + 10 (verified on all 1651 framed rows); every line number below is the FILE line. Payload column omits the 4-byte (`0x010f`) / 2-byte (fixed) header, `Len` is the full frame. Sessions: observer = account 2000001 (char `skillsandtraits`, gid 150010, map session `#5`), actor = account 2000000 (char `skillsandtraits2`, gid 150011, map `#6`, relog `#8`). The `0x00f3` chat packets show an empty payload (proxy does not decode them), so each command below is identified by its effects; commands marked (inferred) are not visible as text. Only these skill ids occur: 1 NV_BASIC, 2 SM_SWORD, 3 SM_TWOHAND, 4 SM_RECOVERY, 5 SM_BASH, 6 SM_PROVOKE, 7 SM_MAGNUM, 8 SM_ENDURE, 9/10/11/14/16/19/20 mage roots, 55 KN_SPEARMASTERY, 56 KN_PIERCE, 60 KN_TWOHANDQUICKEN, 61 KN_AUTOCOUNTER, 63 KN_RIDING.

Proxy quirks: no misframes of `0x010f`/`0x0b20`/`0x0acb` (unlike the older settings.md login capture). Per-session `0x8480`/`0x8481` before `0x082d` is the 4-byte account id echo shown as an unknown 4-byte opcode; `0x0b72` appears as 4-byte empty frames after each page when an account has 3 characters (#9, 1630-1638); `0x00f3`, `0x0b1c`, `0x0360` etc. are "KNOWN / UNIMPLEMENTED" with empty payload.

### 10.1 Step map (file lines) and deviations

| Step | Lines | What the wire shows |
| :--- | :--- | :--- |
| Actor login (fresh Novice) | 265-320 (#6; list at 312) | `0x010f` 41 bytes = NV_BASIC lv 0 `upFlag 1` (inf 0, sp 0, range2 0), then `0x0b20` x2, `0x0acb` x4, `SP_SKILLPOINT 0`, `0x00bd` |
| 1 `@jlvl 9` | 443-465 | msg `0x008e`, `SP_JOBLEVEL 10`, `0x0acb` x2, `SP_SKILLPOINT 9`, stat diffs; **no `0x010f`**. Observer gets `0x019b` (job-level-up effect, type 1) at 444 |
| 1 NV_BASIC x9 | req 472-480 (same ms) -> 481-507 | 9 x [`0x010e`, `SP_SKILLPOINT` 8..0, `0x010f` 41]. Lv 9: `0x010e` upFlag 0, list `NV_BASIC 9/upFlag 0`. Real Novice: tree unchanged, so **no extra pre-list at lv 9** |
| 1 "once more at max" | none | **No `0x0112` was sent**: the client blocks the request (also for the "0 points" click in step 2). Refusal at max / 0 points is therefore unobserved |
| 2 `@job 1` | 512-569 | `0x00be` x6, `SP_JOBLEVEL 1`, `0x0acb` x2, `0x01d7` x4, **one** `0x010f` (189: ids 1(lv 9),2,4,5,6) at 536, `SP_MAXWEIGHT`, stat/look diffs, `0x0a24` x8, `0x008e`. No `0x0441`, no `SP_SKILLPOINT`. Observer: `0x01d7` look changes 513-522 |
| 2 `@jlvl 9` | 572-589 | `SP_JOBLEVEL 10`, `SP_SKILLPOINT 9`; no list |
| 2 spend | SM_SWORD 594; BASH x5 607-611; PROVOKE x3 633-635 | see 10.3. Total 9 points -> `SP_SKILLPOINT 0` at 643 |
| 3 `@jlvl -5` | 650-663 | see 10.5 |
| 4 `@jlvl 40` | 671-678 | `SP_JOBLEVEL 45`, `SP_SKILLPOINT 44` |
| 4 spend 14 | req 707-720 -> 721-763 | all 14 accepted: Sword 1, Twohand 1, Bash 5, Magnum 1, Provoke 5, Endure 1 (points 44 -> 30) |
| 4 `@skreset` | 766-773 | see 10.5 |
| 5 spend 19 | req 782-800 -> 801-858 | all 19 accepted (Provoke 5, Endure 1, Bash 5, Magnum 6, Sword 1, Twohand 1; 44 -> 25) |
| 5 `@reset` | 859-892 | see 10.5 |
| 6 `@allskill` | 895-898 | see 10.5 |
| 7 `@job 7` | 906-935 | see 10.6 |
| 7 `@jlvl 10`, learn 55 | 971-1012 | `SP_JOBLEVEL 11`, `SP_SKILLPOINT 10`; learn KN_SPEARMASTERY (**id 55**): see 10.3 |
| 8 extra command (deviation) | 1020-1035 | job level 11 -> 13 and points 9 -> 11: `+2` job levels (a `@jlvl 2`, inferred); not in the script |
| 8 `@job 2` | 1039-1099 | see 10.6 |
| 8 relog | `0x00b2`/`0x00b3` 1106-1107 (back to char select, not a full logout), char list #7 1111-1129, map #8 1131-1250 | see 10.6 |
| 8 `@job 7` after relog | 1250-1287 | see 10.6 |
| 9 observer | 1299-1615 | see 10.7. Deviations: the observer also did a second `@jlvl 9` (1443), a `@job 7` (1479, inferred from char select job 7) and `@jlvl 20` (1530), and tried Knight skills |

### 10.2 `0x010f` entry facts (all 90 lists decoded: `id:u16 inf:i32 lv:u16 sp:u16 range2:u16 name[24] upFlag:u8`)

* **Entry order is ascending skill id** in every list (no exceptions); frame = `4 + 37n` (41, 189, 226, 263, 300, 448, 485 seen).
* Level-0 entries: `sp = 0`, `range2 = 0`, **`inf` is still the real TargetType** (Bash 1, Magnum 4, Endure 4, Sight 4, Cold Bolt 1, passives 0), `upFlag = 1`. Fresh NV_BASIC lv 0: `upFlag 1`.
* `upFlag` is 0 only at the tree max (NV_BASIC 9 for a Novice-class char, 9 after job change too; 10 for SM skills after `@allskill`). It stays 1 with 0 free points (list after the last point spent, 644, still all 1).
* Learned active skills carry `sp`/`range2` in both `0x010f` and `0x010e`: SM_BASH lv 1..5 `sp 8, range2 1` (lv 10: 15, 1); SM_PROVOKE lv 1..5 `sp 4,5,6,7,8, range2 9` (lv 10: 13, 9); SM_MAGNUM lv 1..6 `sp 30, range2 0` (lv 10: 30); SM_ENDURE lv 1 `sp 10, range2 0` (lv 10: 10). Passives (SWORD, TWOHAND, RECOVERY, KN_SPEARMASTERY) `sp 0, range2 0`. `0x010e` = `id:u16 lv:u16 sp:u16 range2:u16 upFlag:u8` (11-byte frame), same values as the list entry.

### 10.3 Skill-up reply order (confirmed)

* Active skill (Bash, Provoke, Magnum, Endure): `0x010e`, `SP_SKILLPOINT (0x0c)`, `0x010f`. Newly unlocked skills show up only in that trailing list: Bash 5 adds 7 (626), Provoke 5 adds 8 (760), Magnum is not preceded by a pre-list.
* Passive that unlocks something (SM_SWORD lv 1 -> adds 3 at 595 and 721; KN_SPEARMASTERY 1 -> adds 56 at 1009; NV_BASIC 9 on a Swordman -> adds 2,4,5,6 at 1436): `0x010f` (new tree, already containing the incremented level) **before** `0x010e`, then `SP_SKILLPOINT`, then the usual trailing `0x010f`.
* Passive that unlocks nothing (SM_TWOHAND 1 at 725, NV_BASIC 1..8, NV_BASIC 9 on a real Novice): `0x010e`, `SP_SKILLPOINT`, `0x010f`. No stat-diff packets for SM_SWORD/KN_SPEARMASTERY (no weapon).
* Burst of N requests in one millisecond: N reply groups in request order; later requests see unlocks from earlier ones (SM_TWOHAND after SM_SWORD, SM_MAGNUM after Bash 5 within burst 707-720).
* Refusals are silent (zero bytes): observer clicked hidden/unlearnable ids with free points: 1395-1397 (SM_SWORD while still effectively Novice, 9 points), 1559-1575 (ids 55, 56 x5, 58 x3, 63, 57) and 1597-1607 (11 more), only the visible ones in the same burst were answered.

### 10.4 Novice -> first class unlock (observer, skillup_limit)

After `@jlvl 9` (1299) and `@job 1` (1327) with NV_BASIC unspent, the Swordman char got `0x010f` 41 bytes = **only NV_BASIC lv 0** (1341). SM_SWORD x3 at 1395-1397 with 9 free points: no reply. NV_BASIC x9 (1403-1411): 8 normal groups (list 41), the 9th produced `0x010f` 189 (1,2,4,5,6) at 1436, `0x010e lv 9 upFlag 0`, `SP_SKILLPOINT 0`, `0x010f` 189. This confirms the Novice-effective-job rule (section 3): the class tree appears the moment 9 points are spent. Other observer packets: none for skill-ups (see 10.8).

Second-class gate (observer, `@job 7` at 1479, 1st-class change at job level 10): after Provoke 5 + Endure 1 the char had spent 9 (NV_BASIC) + 1 + 1 + 5 + 1 = 17 points and 21 free points; Knight skills (55...) were refused (1559, 1597-1607) and absent from lists (1593). Consistent with `P >= 9 + (change_level_2nd - 1) = 18` needed; the threshold itself was not reached, so 18 is inferred. Char select later shows job 7, joblv 21, sppoint 21 (1620).

### 10.5 Resets and `@allskill` (actor)

| Command | Lines | Exact order |
| :--- | :--- | :--- |
| `@jlvl -5` (job lv 10, 0 free, 9 spent) | 650-663 | `0x008e`, `SP_JOBLEVEL 5`, `0x0acb` x2, `SP_SKILLPOINT 4` (= 9 refund - 5), **`0x0441` 3, `0x0441` 7, `0x010f` (189: 1 lv 9, 2,4,5,6 all lv 0)**, stat diffs. NV_BASIC 9 kept. **No first list**; the one list is fresh (levels 0), not stale, because the visible tree shrank (Twohand/Magnum removed). Observer: nothing |
| `@skreset` (30 free, 14 spent) | 766-773 | `SP_SKILLPOINT 44`, `0x010f` (296: ids 1-8, all levels 0, nothing deleted yet), `0x0441` 3, 7, 8, `0x010f` (185: 1,2,4,5,6), `0x008e` |
| `@reset` (25 free, 19 spent) | 859-892 | `0x0141` x12 (STR..LUK types 13-18 value 1 with bonus; trait types 219-224 value 0), `0x00be` x12 (cost 32-37 = 2; 247-252 = 0), `SP_STATUSPOINT 48`, `SP_TRAITPOINT (0xe7) 0`, `SP_SKILLPOINT 44`, `0x010f` (all 0), `0x0441` 3, 7, 8, `0x010f` (5), `0x008e`. Stat side first, skills last; no derived-stat diffs after |
| `@allskill` (44 free, Swordman) | 895-898 | `0x010f` (296: ids 1-8; NV_BASIC stays 9, 2-8 at lv 10 `upFlag 0`, Bash 15/1, Provoke 13/9, Magnum 30/0, Endure 10/0), `SP_SKILLPOINT 0`, `0x008e`. No recalc diffs, no deletes. Visible set = Swordman tree only (no quest skills, no Knight) |

Refund arithmetic: new free points = old free + all spent except NV_BASIC (44 = 30 + 14; 44 = 25 + 19; 4 = (0 + 9) - 5).

### 10.6 Job changes and persistence (actor)

* Novice -> Swordman (512): one `0x010f` with the final tree; Swordman -> Knight (906): `0x00be` x6, `SP_JOBLEVEL 1`, `0x0acb` x2, `0x01d7` x4, **one** `0x010f` (932: 12 entries = 8 Swordman at lv 10/`upFlag 0` + 55, 60, 61, 63 at lv 0), then HP/stat diffs. Swordman levels survive an inherited-tree change. No `SP_SKILLPOINT`, no `0x0441`.
* Knight -> Mage (1039): same shape, **one** `0x010f` (1065: ids 1 lv 9, 9, 10, 11, 14, 16, 19, 20 at lv 0, 296 bytes), **no `0x0441` for the 8 vanished Knight/Swordman ids** (the replaced list is enough), no `SP_SKILLPOINT` (points stay 11). NV_BASIC 9 survives.
* After `@job 2` the char went through `0x00b2` restart. Char select sppoint (offset 94 of the 175-byte entry): 11 (job 2, joblv 1) at 1114/1123, 11 (job 7, joblv 1) at 1649/1658; the observer's char shows sppoint 21 (1620). Skill points persist and equal the last `SP_SKILLPOINT`.
* Map login after relog (1194): `0x010f` (same 8 Mage entries) then `0x0b20` x2, `0x0acb` x4, `SP_SKILLPOINT 11`, `0x00bd` (same order as the first login, 312-320).
* `@job 7` after the relog (1250): `0x010f` 1274 = ids 1 (lv 9), 2, 4, 5, 6, 55, all other at lv 0, `upFlag 1`. **The Knight levels (SM skills at 10 from `@allskill`, KN_SPEARMASTERY 1) did not survive Mage + relog**; only NV_BASIC 9 did. (Cannot tell whether the in-memory levels survive a Mage detour without relog, since the relog intervened; the source says they would until the next save.) The Knight tree showed only 55 as new because the Knight change happened at Mage job level 1 (`change_level_2nd = 1`, so the extra points needed is 0).
* Observers see for the actor's job changes: `0x01d7` look packets (types 0 job, 1 hair, 7, 0xd, 2, 9, 3, 4, 5, 0xc) and `0x080e`; the actor's logout appears as `0x0080` type 2 (1109).

### 10.7 Observer-visible packets for the actor's skill actions

Between 419 and 1300 session #5 receives only: `0x019b` type 1 for every positive `@jlvl` (444, 586, 672, 991, 1022), `0x01d7` look bursts on job changes, `0x080e` HP updates (700, 996, 1034, 1057, 1058, 1140), `0x007f` ticks, `0x0080` type 2 at logout. **Nothing for skill-ups, resets, `@allskill`, `@jlvl -5`** (650-663 produce no observer packet). The actor symmetrically gets `0x019b` for the observer's `@jlvl` (1300, 1444, 1554).

### 10.8 Checks against earlier source-only claims

Confirmed: reply order (section 2), silent refusals, pre-list for tree-growing passives, trailing-list unlock for actives, `@jlvl +n` sends no list, `@allskill` order, NV_BASIC kept on reset for a Swordman, hidden skills lost across relog, sp/range2 only for learned levels. Corrected: job-change `0x0441` is not universal; `@jlvl -n` and resets send a fresh list when the tree shrinks; list order is ascending id; the client never sends `0x0112` at 0 points or at max level.

## 11. Open questions (remaining after `rathena_skills_sequence.txt`)

* Refusal of `0x0112` at 0 points / at max level / on a hidden id with 0 points (the client never sent one here; send a raw packet or patch the client). Hidden-id refusal with free points is captured (10.3).
* `@jlvl -n` reset where the visible tree does NOT shrink (stale-list case), e.g. only NV_BASIC-dependent skills.
* Whether hidden levels survive a job detour without a relog (e.g. `@job 2` then straight `@job 7`). Relog loss is captured (10.6).
* Exact `change_level_2nd` threshold (18 spent expected for the observer's case; only 17 tested). Reaching 18 should reveal 55 etc.
* `@skreset` as a Novice (NV_BASIC reset and refunded) vs Swordman (kept, captured).
* Skill-up while dead / sitting / in a vending shop (expected accepted, no checks in `pc_skillup`).
* First-class unlock with `BaseLevel`/`JobLevel` gates in `skill_tree.yml`: confirm `0x010f` right after `SP_SKILLPOINT` on job level-up.
