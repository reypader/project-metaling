# Mob Lifecycle: spawn, damage log, death, respawn

Target `PACKETVER = 20260219`, renewal (`RENEWAL`, `RENEWAL_EXP`, `RENEWAL_DROP` all on; `src/config/renewal.hpp:45,52`). Citations are `$RATHENA_PATH/src/map/...` at commit `d4b8e7b8f` unless stated; battle conf defaults from `conf/battle/*.conf`. Wire layouts: [map_units.md](../packets/map_units.md) and [ground_items.md](../packets/ground_items.md). Static data fields: [mob_db.md](../db/mob_db.md).

**Verification status:** written from source. [`captures/item_drop_pickup_sequence.txt`](../captures/item_drop_pickup_sequence.txt) (2026-10-23) has since confirmed the §3.1 kill order. Killer: `0x0977` hp 0 → exp packets → `0x0add` in the same tick, then `0x0080` type 1 about 250 ms later. The first drop lands on the mob's own cell (`DIR_CENTER`). See [PF-15](../../../PACKET_FINDINGS.md#pf-15). The rest (rates, damage log, respawn) is still source-only. rust-ro (`$RUSTRO_PATH`, commit `14193fcd`) has no usable mob-death/drop/exp logic (only a `MobDeathClientNotification` event and DB models); not used.

## 1. Spawn

### 1.1 Spawn line (permanent spawn) `npc_parse_mob` (`npc.cpp:5218-5420`)

```
<map>,<x>,<y>{,<xs>,<ys>}	monster|boss_monster	<name>{,<level>}	<mob id|AegisName>,<amount>{,<delay1>{,<delay2>{,<event>{,<size>{,<ai>}}}}}
```

* Defaults: `delay1 = 5000` ms, `delay2 = 0`. `amount` must be 1..1000. `<level>` -1/absent = use db level (0 or > MAX_LEVEL is an error). `size` -1..2 (Small/Medium/Big), `ai` -1..max. The mob id field may be numeric or an `AegisName`.
* Real data: `prt_fild08,0,0	monster	Poring	1002,20,5000` (map-wide, 20 mobs, 5 s); `prt_fild00,285,138,10,10	monster	Green Plant	1080,5,360000,180000`; `in_sphinx5,0,0,0,0	boss_monster	Pharaoh	1157,1,3600000,600000,1` (`npc/re/mobs/**`).
* `boss_monster` only sets `spawn.state.boss`. It changes respawn variance and the spawn-search shortcut (below); it is NOT the same as the `Class: Boss` / `Mvp` db flag.
* Spawn rectangle: `xs == 0 && x > 0` becomes `xs = 1` (same for `ys`); negative span = 0 with a warning. Forced **map-wide random** (`x = y = -1`, `xs = ys = 0`) when `x == 0 && y == 0`, or `xs == ys == 1` on a non-walkable cell, or `force_random_spawn: yes` (default no).
* `mob_count_rate: 100` scales `amount` (min 1 when `amount > 1`). `mob_spawn_variance: 1` (default): `delay2` is kept only for `boss_monster` lines; for normal lines `delay2` is forced to 0 (`npc.cpp` ~5345).
* `delay1 != 5000 && delay1 < mob_respawn_time (1000)` is replaced by 5000 with a warning (only when the field is present).
* `override_mob_names: 0` (default): the name on the spawn line is used; 1 = db English name; 2 = db Japanese name. The name goes into `md->name` (<= 23 chars).
* Unknown map / unhosted map / x,y outside map: the line is skipped (error logged).
* `npc_parse_mob2` (`npc.cpp:5198`) creates `num` mob objects (`mob_spawn_dataset`) and calls `mob_spawn` for each. `randomize_center_cell: yes` (default): when `xs > 1`, each mob's own center X is `rnd[x-xs+1, x+xs-1]` (same for Y); otherwise center = line x,y.

### 1.2 Once-spawn (script `monster`, GM commands) `mob_once_spawn` (`mob.cpp:636-730`)

* No `spawn_data` is retained (`md->spawn == nullptr`). Position: explicit x,y if inside the map (`0 < x < xs`, `0 < y < ys`); else `map_search_freecell` anywhere on the map. With a player and `x<0||y<0`: free cell next to the player.
* `mob_spawn(md)` places it at `md->x,y` with no further search. `mob_once_spawn_area` picks random cells inside a rectangle (up to `min(1000, w*h*3)` tries).
* On death such mobs are freed (`mob_dead` returns 5, `status_damage` -> `unit_free`), i.e. never respawn and the object id is released. Everything else in section 3 is identical.

### 1.3 Object creation `mob_spawn_dataset` (`mob.cpp:465-505`)

* `md->id = npc_get_new_npc_id()`: same global pool as NPCs (starts at `110000000`, see [npcs.md Ids](./npcs.md#ids)). `map_addiddb` registers it. The id is allocated once; **a respawn reuses the same id** (no new id, no new object).
* `md->db = mob_db[mob_id]`; `md->level = spawn level` if given; `special_state.size` and `.ai` come from the spawn line (NOT from the db `Size`). View data: `status_set_viewdata(md, mob_id)` -> `look[LOOK_BASE] = mob id`, all other looks 0 (`mob.cpp` `s_mob_db` ctor; `MobAvailDatabase` can override with a player sprite, ignore).
* `centerX/centerY` = spawn x,y (or the randomized center).

### 1.4 Placement and (re)initialisation `mob_spawn` (`mob.cpp:1117-1224`)

Order:
1. If it has `spawn` data: `x,y = centerX,centerY`. Skip the search entirely only if: normal mob, `xs+ys >= 1`, `rnd_chance(1, xs*ys)` hits and the center is reachable (`CELL_CHKREACH` = walkable). For `boss_monster` with a fixed 1x1 spawn on a nonzero cell the search is always skipped.
2. Otherwise `map_search_freecell(md, rx = xs-1, ry = ys-1, flag, tries = 8)` (`map.cpp:1798-1867`): picks `rnd[bx-rx, bx+rx]` x `rnd[by-ry, by+ry]` (inclusive; for `rx<0`: whole map inside `[map_edge_size(15), xs-15-1]`), skips the base cell itself, skips cells within 5 of the map edge (`x < 5 || x > xs-5 ...`), accepts the first walkable cell. `no_spawn_on_player: 0` (default) = no player-avoidance.
3. If that fails and the center is not walkable: map-wide search (50 tries). If that fails too: retry in `mob_respawn_time` (1000 ms) via `mob_delayspawn`; `mob_spawn` returns 1.
4. State reset: `status_calc_mob(md, SCO_FIRST)` copies the db status block into `md->status` (so `hp = max_hp`, `sp = max_sp`, `speed = WalkSpeed`) and sets `md->level` (spawn level if valid else db level); `dmglog.clear()`, target/attacked ids 0, skill delays 0, `MSS_IDLE`, `next_walktime = tick + rnd()%1000 + MIN_RANDOMWALKTIME`.
5. `map_addblock(md)`; **if the map has players** (`map_getmapdata(m)->users`) `clif_spawn(md)`; then `mobskill_use(MSC_SPAWN)`.

`max_hp` in `md->status` already includes the load-time rates: `monster_hp_rate` / `mvp_hp_rate` (both 100) (`mob.cpp:5578-5590`). Initial HP == Hp from `mob_db.yml` at default rates.

### 1.5 What the client receives for a mob

* **Appear (new/respawn, observers already in range):** `clif_spawn` (`clif.cpp:1680`) -> `clif_spawn_unit` -> `0x09fe` to `AREA_WOS` (everyone within 14 except the mob), then `clif_name_area` (sends the name packet to `AREA`; see `CZ_REQNAME` note below), and a special-effect packet for big/medium spawn-line sizes (`EF_GIANTBODY2` / `EF_BABYBODY2`, `clif.cpp:1731-1741`).
* **Already there when a player arrives / walks into range:** `0x09ff` (standing, `state` 0) via `clif_getareachar_unit` (`clif.cpp:5029`); `0x09fd` if `ud.walktimer` is running. Plus a `0x0977` HP bar if that player is in the mob's `dmglog` (`clif.cpp:5118-5124`). The big/medium size effect is sent on this path too, to that player only (`clif_specialeffect_single`, `clif.cpp:5100-5106`), and again on the walking-mob refresh path (`clif.cpp:2102-2108`). `clif_name_area` runs on both paths.
* **Field values vs players/NPCs** (layouts: [0x09ff](../packets/map_units.md#0x09ff-zc_notify_standentry11-idle-unit-comes-into-view), [0x09fe](../packets/map_units.md#0x09fe-zc_notify_newentry11-unit-spawns-in-view); `clif.cpp:1041-1210, 1213-1330`):

| Field | Mob value |
| :--- | :--- |
| `object_type` | `0x05` (`clif_bl_type`, `clif.cpp:352-364`): `0x0` if the mob view id is a PC job id, `0xD` for `AI_ABR`, `0xE` for `AI_BIONIC` (summons only). NOT `0x0C` (that is an NPC using a mob sprite). |
| `aid` / `gid` | `md->id` / `0` |
| `speed` | `status.speed` = db `WalkSpeed` (default 150; Poring 400, Scorpion 200); NPCs are fixed 200; status changes alter it |
| `body_state`, `health_state`, `effect_state`, `virtue` | `sc.opt1/opt2/option/opt3` (0 normally) |
| `job` | mob id (`LOOK_BASE`) |
| `head`, `weapon`, `shield`, accessories, palettes, `robe`, `head_dir`, `body` | 0 |
| `guild_id`, `emblem_ver` | 0 (non-zero for guardians / alchemist summons) |
| `honor`, `is_pk_mode_on`, `sex` | 0 |
| `pos_dir` | `WBUFPOS(x, y, dir)`; spawn dir 0 |
| `x_size`, `y_size` | 0 (PC only: 5) |
| `state` (0x09ff only) | 0 (not dead/sit) |
| `clevel` | `md->level` (`clif_setlevel`: default `client_limit_unit_lv: 0`, only NPC/pet forced to 0) |
| `font` | 0 |
| `max_hp`, `hp` | `-1,-1` while `hp == max_hp`; real values `status.max_hp`, `status.hp` once damaged, if `monster_hp_bars_info: yes` (default) and no `hidemobhpbar` mapflag (`clif.cpp:1166,1313,1421`) |
| `is_boss` | `get_bosstype()`: 0 normal, 1 `Class: Boss` (miniboss), 2 `Modes: Mvp` (`mob.cpp:377-395`) |
| `name` | `md->name` (spawn-line name; <= 23 + NUL) |

* `CZ_REQNAME` for a mob: `0x0adf` with `md->name` and empty title at default `show_mob_info: 0`; with non-zero `show_mob_info` the old `ZC_ACK_REQNAMEALL` carrying "HP: x/y" text is sent instead (`clif.cpp:10030-10075`). Guardians always use the long form.
* `clevel` and `is_boss` in the table above hold for both paths: `0x09ff` sets them at `clif.cpp:1161,1175`, `0x09fe` at `clif.cpp:1308,1322`.

### 1.6 Spawn data facts for an importer

Source: `$RATHENA_PATH` at `d4b8e7b8f`, renewal manifest `npc/re/scripts_main.conf`. Counts come from a throwaway scanner that follows `import:`/`npc:` recursively (835 `npc:` files via 14 confs), skips `//` lines and script bodies by brace depth.

**Manifest scope.** The renewal manifest also pulls in `npc/scripts_monsters.conf` (non-RE `npc/mobs/{jail,pvp,towns}.txt`, 25 lines) and quest/job files with top-level spawn lines. 3334 lines are under `npc/re/mobs/**`; 555 are elsewhere (largest: `npc/quests/okolnir.txt` 450, `npc/re/jobs/novice/academy.txt` 21, `npc/re/jobs/3-2/genetic.txt` 18; the Doram `npc/re/custom/lasagna/lasa_fild.txt` 11 lines / 545 mobs and `lasa_dun.txt` 9 / 451 are listed in `npc/re/scripts_athena.conf:276-277` and their maps are enabled in `conf/maps_athena.conf:1225-1229,1232`, so they are active by default).

**Line recognition** (`npc.cpp:5690-5810`):
* Fields are tab-split into w1..w4; w4 runs from the third tab to end of line, so a trailing `\t//comment` lands inside w4. Fewer than 4 fields: "Unable to parse".
* w2 is matched with `strcmpi` against `monster` / `boss_monster` (`npc.cpp:5809`).
* Before dispatch, w1 is checked with `%15[^,],%6hd,%6hd`: unknown map skips the line, a map not loaded on this server skips it silently, x/y outside the map is an error and skips it (`npc.cpp:5730-5765`).

**Parse formats** (`npc.cpp:5235-5240`):
* w1: `%15[^,],%6hd,%6hd,%6hd,%6hd` (needs at least 1 field). Missing values stay 0, so a bare map name means `x = y = 0` = map-wide.
* w3: `%23[^,],%11d` (needs 1). A missing level stays -1 (= db level).
* w4: `%23[^,],%11d,%11u,%11u,%77[^,],%11d,%11d[^\t\r\n]` into `id, amount, delay1, delay2, event, size, ai`; needs at least 2 (id and amount). The trailing `[^\t\r\n]` is a literal match, not a char class, and has no effect. Defaults: `delay1 = 5000`, `delay2 = 0`, event empty, size -1, ai 0. Delays are bounded `<= 0xfffffff` (`npc.cpp:5361`).
* A trailing `\t//comment` ends the sscanf cleanly (the next expected `,` doesn't match a tab). Any parse failure skips only that line.

**Name** (`npc.cpp:5218-5232, 5362-5370`; `mob.cpp:439-490`):
* `%23[^,]` cuts the name silently to 23 characters (`NAME_LENGTH` 24, `mmo.hpp:154`); no trimming. 35 renewal lines are longer (longest 31, e.g. "Elusive Wraith Dead (Nightmare)").
* `mob_parse_dataset` replaces exactly `--en--` with the db `Name` and `--ja--` with the db `JapaneseName`; the loader sets `JapaneseName = Name` when absent (`mob.cpp:5040-5042`). `override_mob_names` 1/2 writes these same tokens.
* Renewal data: no `--en--`; 32 `--ja--` lines, all in `npc/re/mobs/dungeons/for_dun.txt` (`SPIRIT_*` mobs without a `JapaneseName`, so they get the db `Name`, e.g. "L Blue Earth Spirit").
* No spawn name contains `#`; the spawn path does no `#` handling (only mob chat cuts at `#`, `mob.cpp:4211`). All names are ASCII.

**Id field** (`npc.cpp:5260-5275`): `strtol(id, &end, 0)`; if anything is left after the number it is looked up as an AegisName (`mobdb_search_aegisname`, case-insensitive); else the number must be a mob_db id (clones rejected). `strtol` accepts leading spaces: 8 lines in `ein_dun.txt` (57-64) have ` 20255`, so an importer must trim.

**Validation quirks** (`npc.cpp:5277-5305`):
* `amount` must be 1..1000 (`npc.cpp:5277`).
* Spawn level (w3 `name{,level}`, `%23[^,],%11d`, `npc.cpp:5234`; `mob_lv` starts at -1, `:5220`): absent or non-numeric level leaves -1. Reject test is `(mob_lv == 0 || mob_lv > MAX_LEVEL) && mob_lv != -1` (`:5301-5305`): 0 and >`MAX_LEVEL` skip the whole line with `ShowError`. -1 and any other negative (e.g. -5) pass without error; `mob.level = mob_lv` only runs for `0 < mob_lv <= MAX_LEVEL` (`:5314-5315`), so negatives are silently ignored (level stays unset = 0). No clamping.
* `MAX_LEVEL` = 275, unconditional (`src/map/map.hpp:78`, no `RENEWAL` branch, not config-overridable; `conf/battle` has no knob).
* At spawn: `mob_spawn_dataset` copies `data->level` only if > 0 (`mob.cpp:477-478`). `status_calc_mob_` with `SCO_FIRST` keeps `md->level` only if `0 < level <= MAX_LEVEL && level != md->db->lv`, else uses `md->db->lv` (`status.cpp:2781-2785`). Level above db level only raises stats/exp when `mobs_level_up` is set (`status.cpp:2792`, `mob.cpp:3102`).
* The size/ai validity checks test fields that are still 0 and can never fail; values are applied only when size is 1..2 and ai is 1..`AI_MAX`, otherwise silently ignored.
* The `delay1 < mob_respawn_time` replacement (by 5000) only runs when delay1 was given and is not 5000.

**Event field.** `mob_parse_dataset` strips one leading and one trailing `"` (`mob.cpp:446-452`); `mob_spawn_dataset` keeps it as `md->npc_event` only if at least 4 characters remain (`mob.cpp:~480`). So the values `0`/`1` (95 lines) are ignored; 44 lines carry real labels (`"ant_d02_i_boss::OnMobDead"`, `"lhz_dun_n::OnRegularDead3208"`, `"illusion_mob#moonlight::OnKill"`).

**Counts** (3889 spawn lines):

| Metric | Value |
| :--- | :--- |
| `monster` / `boss_monster` | 3814 / 75 |
| Sum of `amount` | 66040 (under `npc/re/mobs/**`: 3334 lines, 64077 mobs; the other 555 lines add 1963) |
| `amount` range | 1..216; 1628 lines have 1 |
| Distinct maps | 416 (379 under `npc/re/mobs/**`, 37 only elsewhere); all in `db/map_index.txt` and `conf/maps_athena.conf` |
| `level` / `size` / `ai` given | 0 / 0 / 0 lines |
| `delay1` / `delay2` / `event` given | 3453 / 1127 / 139 lines |
| `delay2` non-zero | 974 `monster`, 69 `boss_monster` (non-boss values are zeroed at default `mob_spawn_variance: 1`, `npc.cpp:5350-5354`) |
| Map-wide (`x==0 && y==0`) | 2284 (1995 `map,0,0`, 73 `map,0,0,0,0`, 216 bare map) |
| Explicit rectangle (not 1x1) / explicit 1x1 / single cell without span | 357 / 6 / 1242 |
| Mob refs | 3614 numeric lines, 23 AegisName lines (15 distinct); all resolve |
| Odd formatting | 252 lines with a trailing `\t//comment` (216 `tra_fild_123.txt`, 32 `for_dun.txt`, 4 `championmobs.txt`); 8 leading-space ids; 8 exact duplicate lines (spawn twice); no CRLF, no negative coordinates, no `delay1 < 1000` |

**Dynamic mobs** (`conf/battle/monster.conf:254`, `npc.cpp:5392-5406`, `map.cpp:3004-3098`): `dynamic_mobs: yes` (default) stores a spawn line in the map's `moblist` (cap `MAX_MOB_LIST_PER_MAP` = 128 lines, `map.hpp:72`) and creates the mobs when the first player enters (`map_spawnmobs`). When the last player leaves, `map_removemobs` removes them after `mob_remove_delay` (300000 ms), sparing MVPs (damaged mobs are removed too with `mob_remove_damaged: yes`).

### 1.7 Not used yet: movement and AI notes

* Spawn arms the walk timer: `next_walktime = tick + rnd()%1000 + MIN_RANDOMWALKTIME` (`mob.cpp:1190`); `MIN_RANDOMWALKTIME = 4000` ms (`mob.hpp:39`), `MIN_MOBTHINKTIME = 100`.
* Two timers: `mob_ai_hard` every 100 ms and `mob_ai_lazy` every 1000 ms (`mob.cpp:7363-7364`). Hard AI runs only for mobs within `AREA_SIZE + ACTIVE_AI_RANGE` = 14 + 4 cells of a player (`mob.cpp:46, 2351`) when `monster_ai & 0x20` is off (default).
* `mob_ai_sub_lazy` (`mob.cpp:2359-2445`) handles mobs with no player near. A mob a player activated keeps hard AI for `mob_active_time` (5000 ms) / `boss_active_time` after the last contact (`battle.cpp:8622`). In lazy mode a mob random-walks only if `mob_is_spotted`; a never-spotted mob never walks. The lazy walk rolls `mob_nopc_move_rate` / `boss_nopc_move_rate` (both 100).
* `mob_randomwalk` (`mob.cpp:1673-1770`): needs `next_walktime` reached, `MD_CANMOVE`, no `MD_NORANDOMWALK`, and `unit_can_move`. Picks a random passable, pathable cell within ±7 (15x15 window, random stride direction). Success: `MSS_WALK`, `next_walktime = tick + rnd()%1000 + 4000 + walkpath_time`. Failure retries next interval; after 1000 failures it respawns (only with `mob_stuck_warning`).
* In hard AI, slaves don't random-walk; a failed walk delays the next try by `rnd()%1000` or until `canmove_tick`. After dropping a target, `next_walktime = rnd()%1000 + 4000` (or `rnd()%1000` with `monster_ai & 0x8`).
* Walk speed is `WalkSpeed` (§1.5).

## 2. HP reduction

Chain for a hit: attack calc -> `clif_damage` -> `0x08c8` to `AREA` immediately (`battle.cpp:7399`, layout [0x08c8](../packets/map_units.md#0x08c8-zc_notify_act-aka-zc_notify_act3)) -> `battle_delay_damage` timer (`delay_battle_damage: yes`, delay about amotion) -> `battle_damage` (`battle.cpp:319`) -> `status_damage(src, target, dhp, dsp, dap, walkdelay, flag, skill_id)` (`status.cpp:1492`).

`status_damage` for a mob target, in order (`status.cpp:1492-1802`):
1. `hp` clamped to current `status->hp` (if `flag&2` and damage >= hp, return 0 instead). Negative damage = heal.
2. Non-passive damage (`flag&1 == 0`): breaks damage-removable status changes, cancels cast (`unit_skillcastcancel(target,2)`), stamps `ud.dmg_tick`.
3. `dhp` (value for the log) = the requested damage, **not clamped** (overkill counts) unless `exp_calc_type == 1`, which logs the real HP lost (`status.cpp:1559-1565`).
4. `status->hp -= hp` (can reach 0 here).
5. `mob_damage(md, src, dhp)` (`mob.cpp:2774`): `mob_log_damage` (section 2.1); then, if `monster_hp_bars_info: yes` and not `hidemobhpbar`: for every `dmglog` entry whose player is online and within 14 cells (`AREA_SIZE`) of the mob, send them `0x0977` HP bar (`clif_monster_hp_bar`, `clif.cpp:19957`: `u16 0x0977, u32 mob id, u32 hp, u32 max_hp`, 14 bytes, `packet_len` `clif_packetdb.hpp:1639`). On the killing blow `hp` is already 0, so the killer receives `0x0977` with `hp = 0` right before death handling. `show_mob_info&3` additionally re-sends the name packet to `AREA`.
6. A mob attacking something logs its own normal-attack damage onto the victim mob's log as `damage_tanked` (see 2.1).
7. If `status->hp != 0`: return (walk delay applied). Else `status->hp = 0` and `mob_dead(md, src, type)` (`flag&4 ? 3 : 0`; type bit 1 = no drops, bit 2 = no exp; used by `killmonster`-style kills). Revival hooks (Kaizel, Ultimate Sacrifice, `SC_REBIRTH` once) short-circuit before death handling and set `state.rebirth`.
8. After `mob_dead` returns nonzero: `status_change_clear`; then `flag&4` -> `unit_free`, `flag&2` -> `unit_remove_map(CLR_DEAD)` (for a dead mob this only `map_delblock`s, **no packet**: `unit.cpp` ~3830-3840 "BL_MOB is handled by mob_dead").

### 2.1 Damage log `mob_log_damage` (`mob.cpp:2672-2772`, `mob.hpp` `s_dmglog`, `map.hpp:67` `DAMAGELOG_SIZE 20`)

* `md->dmglog` is a deque of `{ int32 id /*char_id*/, int64 dmg, int64 dmg_tanked, flag }` with `flag` 0 normal, 1 homunculus (`MDLF_HOMUN`), 2 pet, 3 self (`MDLF_SELF`, `id = mob's own object id`).
* Attribution of `src`: PC -> own char_id; homunculus / mercenary / pet / elemental -> **master's** char_id (flag HOMUN for homunculus, PET for pet, NORMAL for merc/elemental); a mob with `special_state.ai` and a master -> master's char_id; any other source gets no entry (unless the mob damages itself: `MDLF_SELF`). Negative damage is not logged; zero-damage entries are logged only if the source is a valid enemy type.
* Accumulation: an existing entry with equal `(id, flag)` just gets `dmg += damage`, `dmg_tanked += tanked` (saturating at INT64_MAX). Otherwise a new entry is appended at the back. **Capacity 20**: when full, the oldest (front) entry is dropped first.
* `dmg_tanked` = damage the mob dealt to that player with normal attacks (logged from `status_damage` of the victim, `skill_id == 0`, with `damage = 0`); it only matters for MVP selection.
* Cleared on every `mob_spawn`. `spotted_log[20]` is an unrelated AI array.
* `mob_ksprotected` (`ksprotection: 0` default) is off.

## 3. Mob death sequence

Source: `mob_dead(md, src, type)`, `mob.cpp:2934-3660`.

1. If `src`: temporarily set `hp = 1`, `MSS_DEAD`, run "on dead" mob skills (`mobskill_use(-1)`), `hp = 0`.
2. **Filter the damage log.** For each entry (oldest first): add `dmg` to `total_damage`; `MDLF_SELF` only counts toward `count`; otherwise resolve `map_charid2sd(id)`: offline or on another map -> skipped entirely (not counted); on this map -> `count++`; if dead -> skipped from rewards; homunculus entries need an active homunculus, pet entries an active pet; valid entries are remembered (`tmpsd[i]`) and also merged per player into `lootdmg` (all flags summed per player).
3. **Loot priority.** `lootdmg[0].damage += total_damage * first_attack_loot_bonus / 100` (default **30**, `drops.conf:22`; `lootdmg[0]` = the *oldest surviving* log entry, i.e. the first attacker). Sort descending by damage; `first_sd`/`second_sd`/`third_sd` = top 3 players. If `lootdmg` is empty `first_sd` is the killing PC (or null).
4. **MVP player** (`mob_data::get_mvp_player`, `mob.cpp:2898`): null unless `Modes: Mvp`. With `mvp_to_loot_priority: no` (default) it is the online, alive, same-map player with the highest `dmg + dmg_tanked`; first entry wins ties (strict greater-than).
5. `exp_calc_type == 2` only: doubles the first log entry (default 0, not used).
6. **EXP** (skipped if `type&2`, PvP map without `pvp_exp` (default yes), summoned-with-master mobs, or both `nobaseexp`+`nojobexp` mapflags). Let `bonus = 100` (+ `SC_RICHMANKIM` val2 from the killer in renewal; + 10/20 x skill level for Feel/Hate "star" targets if the killer matches; + `(level-db level) x mobs_level_up_exp_rate` if `mobs_level_up`). For each valid entry (`tmpsd[i]`):
   * `per = entry.dmg / total_damage` (default `exp_calc_type: 0`; type 1 uses `dmg / max_hp`), clamped to 1. Offline/dead/off-map players take no share and their share is not redistributed.
   * Attacker-count bonus: `if count > 1 && exp_bonus_attacker(25): count = min(count, exp_bonus_max_attacker(12)); per += per * ((count-1) * 25) / 100`.
   * `mob_size_influence` (default no): Medium /2, Big x2 using the spawn-line size. Pet damage `per *= pet_attack_exp_rate / 100`.
   * `base_exp = cap(BaseExp * per * bonus / 100 * mapflag bexp/100, 1, MAX_EXP)`; same for `job_exp` with JobExp (0 stays 0; `nobaseexp`/`nojobexp` zero them). `BaseExp/JobExp/MvpExp` in the db are already multiplied at load by `base_exp_rate` / `job_exp_rate` / `mvp_exp_rate` (all 100) (`mob.cpp:5077-5095`).
   * Zeny: `zeny_from_mobs: no` (default) -> none.
   * Renewal homunculus gets `base_exp * homunculus_exp_gain / 100` via `hom_gainexp`.
   * **Level penalty (`RENEWAL_EXP`):** `rate = pc_level_penalty_mod(sd, PENALTY_EXP, null, md)` (`pc.cpp:13325`), applied as `cap(exp * rate / 100, 1, MAX_EXP)` to both base and job exp. `diff = mob level - player base level`. Table `db/re/level_penalty.yml` (type `Exp`): defined at diff 16:40, 15:115, 14:120, 13:125, 12:130, 11:135, 10:140, 9:135, 8:130, 7:125, 6:120, 5:115, 4:110, 3:105, -6:95, -11:90, -16:85, -21:60, -26:35, -31:10. Undefined diffs inherit the nearest defined value toward 0 (diff 0..2 = 100, -1..-5 = 100, 17+ = 40, <= -31 = 10) (`pc.cpp:593-615`). Map flag `norenewalexppenalty` disables it. No entry for `Mvp_Exp`/`Mvp_Drop` types, so those are 100.
   * Players not in a party (or in a party with `party.exp == 0`, no share): **`pc_gainexp(sd, md, base_exp, job_exp, 0)`** (`pc.hpp:1512`: `sd, src block_list*, base, job, exp_flag`), skipped for pet entries unless `pet_attack_exp_to_master`.
   * Party members with exp sharing on: their `base_exp/job_exp/zeny` are summed per party, then after the loop **`party_exp_share(party, md, base, job, zeny)`** (`party.cpp:1238`): eligible = party members online, on the mob's map, alive (and not idle if `idle_no_share`); `exp /= eligible count`; `party_even_share_bonus: 0` (default) otherwise `x(100 + bonus*(n-1))/100`; each gets the renewal level penalty again with their own level, then `pc_gainexp(member, md, base, job, 0)`. Note the penalty is NOT applied to the per-party sum before sharing, only per receiving member.
7. **Drops** (skipped if `type&1`, `nomobloot` mapflag, `state.rebirth`, or a summoned (`special_state.ai`) mob unless `alchemist_summon_reward` allows). Details in section 4. Items are collected into lists and dropped by a timer.
8. **MVP reward** (only when `mvp_sd != null`): `clif_mvp_effect(mvp_sd)` (`ZC_MVP`, `0x010c`, id), if `MvpExp > 0` and not `type&2`: `penalty = pc_level_penalty_mod(PENALTY_MVP_EXP)` (100), `+ exp_bonus_attacker` count bonus, `clif_mvp_exp` (`0x010b`), **`pc_gainexp(mvp_sd, md, mvp_exp, 0, 0)`**. MVP drops (unless `nomvploot`/`type&1`): `MvpDrops` rolled in db order (random order if `item_drop_mvp_mode: 1`); each `rnd()%10000 < rate` (rate after `item_rate_mvp` and renewal `PENALTY_MVP_DROP`, 100); default mode 0 awards **only the first success** then stops (mode 2: all). A winner gets `clif_mvp_item(0x010a)`, then `pc_additem(mvp_sd, item, 1, LOG_TYPE_PICKDROP_PLAYER)`; on failure it falls to the floor at the MVP's feet with `map_addflooritem(..., flags=1 (MVP), ...)`. MVP tomb (`mvp_tomb_enabled: yes`) for `boss_monster` spawns: an NPC is created at the death cell.
9. **Events:** unless a revival is pending: quest objective updates, achievement counters, Taekwon mission, `md->npc_event` (the `OnXxx` label from the spawn line event field) run via `npc_event` with killer/killed vars; else `NPCE_KILLNPC` script event for `first_sd`.
10. Cancel `deletetimer`, kill slaves (`mob_deleteslave`) if it can summon.
11. **Vanish packet scheduled:** `clif_clearunit_delayed(md, CLR_DEAD, tick + 250)` (`clif.cpp:989-1010`, `mob.cpp` ~3633-3647). After 250 ms a `0x0080` `ZC_NOTIFY_VANISH` with `type = 1` (died) is sent to `AREA` (everyone within 14 of the death cell, **including** the attacker; `clif_clearunit_area`, `clif.cpp:970-980`). The client plays the death animation on receiving it. "250 ms" lets skill effects (soul drain) show. (Mobs with a PC sprite get `CLR_DEAD` immediately and `CLR_OUTSIGHT` after 3000 ms; ignore.)
12. **Corpse / block removal is immediate, the packet is delayed:** `mob_dead` returns 3; `status_damage` then unlinks the block (`map_delblock`) with no packet. So for 250 ms the mob is gone server-side (cannot be targeted by AoI scans, no longer in the block grid) but still visible on clients. There is no server-side corpse.
13. No spawn data (`once-spawn`): returns 5, object freed. Else `mob_setdelayspawn(md)` (below) and return 3.

### 3.1 Packet order for one normal kill (single player, loot, no party) at default conf

> Open findings: [PF-15](../../../PACKET_FINDINGS.md#pf-15).

| t | Packet | Target |
| :--- | :--- | :--- |
| attack | `0x08c8` damage (type 0 normal / 8 multi / 10 crit) | `AREA` |
| damage lands | `0x0977` hp 0/max for the killer (every log member within 14 cells) | each log member `SELF` |
| same tick | EXP packets from `pc_gainexp` (other topic: `0x00b0` `SP_BASEEXP`/`SP_JOBEXP`, level-up if any) | killer |
| +0 ms (timer, because `delay_battle_damage: yes` makes the delay `0`) | `0x0add` ZC_ITEM_FALL_ENTRY per dropped item ([ground_items.md](../packets/ground_items.md)); autolooted/auto-given items instead come as the pickup ack | `AREA` around the drop cell |
| +250 ms | `0x0080` vanish `type 1` | `AREA` |

(+0 ms: `add_timer(tick + (!delay_battle_damage ? 500 : 0))` at `mob.cpp:3326`; with `delay_battle_damage: no` the drops would appear 500 ms later.) The timer still fires on a later server timer pass, not synchronously inside `mob_dead`.

## 4. Item drops

Source: `mob_dead` `mob.cpp:3250-3330`; rate functions `mob.cpp:2818-2897`, `4808-4840`, `6773-6935`.

* **Load-time rate (stored in `entry->rate`, 1..10000 = 0.01%..100%)** `mob_drop_ratio_adjust` per db drop: base `Rate` (+1 if `drop_rateincrease` and `rate < 5000`, default no); `rate = Rate * rate_adjust / 100` clamped to `[item_drop_<type>_min(1), item_drop_<type>_max(10000)]` where `rate_adjust = item_rate_<common|heal|use|equip|card>[_boss|_mvp]` (all 100 at default; type by item type, boss/mvp variants by `Class: Boss` / `Mvp` of the mob); treasure-chest-race mobs use `item_rate_treasure`; MVP drops use `item_rate_mvp`. `db/mob_item_ratio.yml` can override per item. `logarithmic_drops` off by default. Entries with rate 0 are removed. So at default conf the effective base rate is `Rate` verbatim.
* **Roll time** (`mob_getdroprate`, `mob.cpp:2818`): start = stored rate; `mob_size_influence` (off); `drops_by_luk`/`drops_by_luk2` (0); for a PC killer: bonus % = 100 + `dropaddclass`/`dropaddrace` bonuses (+25 in pk_mode with level gap >= 20) + `SC_ITEMBOOST` etc; `drop_rate = round(rate * bonus / 100)`; cap at `drop_rate_cap` (**9000** = 90%) unless the base rate itself was already above it; **renewal level penalty** `drop_rate = rate * pc_level_penalty_mod(first/second/third_sd, PENALTY_DROP, md) / 100` (diff = mob level - player level; table `Drop` in `level_penalty.yml`: 16:50, 13:60, 10:70, 7:80, 4:90, -4:90, -7:80, -10:70, -13:60, -16:50, undefined diffs inherit toward 0 (so -3..3 = 100, 17+ = 50); skipped (100) for `MD_FIXEDITEMDROP` mobs (`Class: Event`) and by mapflag `norenewaldroppenalty`); min 1 (0.01%) unless `drop_rate0item`; max 10000.
  * Order matters: luk -> player bonus -> cap 9000 -> penalty -> clamp [1,10000]. The penalty player is `first_sd`, else `second_sd`, else `third_sd`; with no player the modifier is 100.
* **Roll:** per `Drops` entry (db order, up to 10): `rnd() % 10000 < drop_rate` -> drops 1 item (amount always 1 in this path; each entry is independent). Pet-egg type items become a pet egg for `first_sd` instead of a floor item. Rare-drop broadcast is off (`rare_drop_announce: 0`).
* **Other sources before the regular loop:** `sd->add_drop` script bonuses and `get_zeny_num` (killing player only), then regular drops, Ore Discovery (`BS_FINDINGORE`, `first_sd`), map-wide / per-mob `map_drop_db` drops. `autoloot` handling inside `mob_item_drop` (`mob.cpp:2582`) hands items straight to the priority player if their autoloot threshold allows (`party_share_loot`), otherwise the item is queued.
* **Drop lists:** `dlist` (position = mob's death cell `md->x, md->y`; owners `first/second/third_charid`) and `lootlist` (items the mob had picked up with `MD_LOOTER`, which drop with `loot=true`).
* **Drop timer** `mob_delay_item_drop` (`mob.cpp:2556`) -> `mob_process_drop_list` (`mob.cpp:2531`): for each queued item `map_addflooritem(item, amount, m, x, y, first, second, third, flags=4, mob_id, canShowEffect = !loot, dir, type = BL_CHAR|BL_PET)`. Cell selection is by `dir` (compass: N(0,+1) NW W SW S SE(+1,-1) E NE, `unit.cpp:70-71`): first regular item -> `DIR_CENTER` (the mob's own cell, no free-cell check); then the direction cycles **SE -> W -> N -> SE ...** (offset one cell). For loot items the first is N and the cycle is the same. The offset cell is used if `map_cell_free` (walkable, at least 5 cells from the map edge, no PC/MOB/HOM/MER/ELEM/PET on it); otherwise `map_search_freecell_dist(distmin=1, distmax=1)` (`map.cpp:1757`) takes the first free cell among the 8 neighbours of the offset cell, scanned in fixed (i,j) order with random x and y mirroring (so which side is tried first is random); if none is free the item stays on the offset cell (floor items may stack because `BL_ITEM` is not in `type`). Details of the floor object, owner timers and expiry: [ground_items.md](../packets/ground_items.md).
* MVP-dropped (`Class: Boss`/`Mvp`) floor items get the extended protection timers automatically (mob bosstype != NONE).

## 5. Respawn

`mob_setdelayspawn` (`mob.cpp:1064`): called at the end of `mob_dead` (not for once-spawns, not for rebirth).
* `spawntime = delay1 + (delay2 ? rnd() % delay2 : 0)` (ms; delay2 is 0 for non-boss lines at default conf).
* Rate scaling (all 100 default): boss (`MD_STATUSIMMUNE`) `boss_spawn_delay`, plant-type (`MD_IGNORE*` modes) `plant_spawn_delay`, normal `mob_spawn_delay` (`spawntime / 100 * pct`). Floor at **1000 ms**.
* Timer starts at death (`gettick()` at `mob_setdelayspawn`), i.e. NOT after the 250 ms vanish; effective respawn = `spawntime`. Typical: Poring 5000, `delay1 = 5000` lines respawn 5 s after death (+/- the timer pass).
* Fires `mob_delayspawn` -> `mob_spawn(md)` (section 1.4): same object id, random cell per the spawn rectangle (not necessarily the death cell), full HP, `dmglog` cleared, then `0x09fe` `AREA_WOS` if players are on the map. If no cell can be found it retries after 1000 ms.
* Corpse removal: nothing remains on the server after `map_delblock`; the only "corpse" is the client-side animation ended by `0x0080 type 1`, which the client treats as the mob disappearing after the death animation.

## 6. Minimal implementation checklist (for a "something to kill" milestone)

1. Spawn: parse `monster` lines (map, x, y, xs, ys, name, id, amount, delay1, delay2), allocate ids from the shared pool, `hp = max_hp`, send `0x09fe` to others / `0x09ff` on entry, `object_type 0x05`, `speed = WalkSpeed`, `clevel = Level`, `hp/max_hp = -1` until damaged.
2. Damage: log per char_id (cap 20, oldest evicted), send `0x0977` to loggers in range, hp bar value includes the final 0.
3. Death: compute exp shares by `dmg / total_damage`, call `pc_gainexp(killer_sd, mob, base, job, 0)` per share (+ level penalty), build drops, schedule +0 ms drop timer and +250 ms `0x0080 type 1`, remove from the grid immediately, start the respawn timer.
4. Skip for now: MVP, slaves, looting mobs, party share (single player), autoloot, `show_mob_info`, KS protection, tombs.
