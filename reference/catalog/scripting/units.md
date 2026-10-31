# NPC scripting vs monster behaviour: unit builtins, shared unit model, mob AI split (renewal, PACKETVER 20260219)

Source: rAthena `$RATHENA_PATH` at `d4b8e7b8f` (read-only research 2026-10-31; line numbers spot-checked). Files: `src/map/script.cpp` (BUILDIN_DEF table), `src/map/unit.cpp`/`unit.hpp`, `src/map/mob.cpp`/`mob.hpp`, `db/re/mob_skill_db.txt`, `npc/re/instances/*.txt`. Usage counts are approximate `grep` counts over shipped `npc/`. Rules here are server-side only, not PACKETVER dependent. Purpose: decide what a future scripting API must expose (shared surface) versus what stays NPC-only or engine-AI-only. Related: [skills.md](./skills.md), [items.md](./items.md), [../mechanics/mobs.md](../mechanics/mobs.md) (section 17), [lua_notes.md](./lua_notes.md).

## 1. Unit builtins (`script.cpp`)

| Builtin | BUILDIN_DEF line | Shipped use (approx) | Notes |
| :--- | :--- | :--- | :--- |
| `monster` | 28078 | ~3400 | spawn at cell, returns unit id; optional event label (death callback) |
| `areamonster` | 28080 | ~1000 | spawn in rectangle |
| `killmonster` | 28081 | ~450 | by name/map, optional event |
| `summon` | 28234 | ~120 | slave of the invoking player |
| `unitexists` | 28332 | few | |
| `getunitdata` / `setunitdata` | 28338 / 28339 | ~130 / ~140 | property get/set by `UMOB_*` etc. (HP, mode, ...) |
| `unitwalk` / `unitwalkto` | 28340 / 28341 | ~120 | walk to cell with optional arrival-event label |
| `unitwarp` | 28343 | few | |
| `unitattack` / `unitstopattack` | 28344 / 28345 | 0 / 0 | unused in shipped scripts |
| `unittalk` | 28347 | ~590 | overhead text |
| `unitskilluseid` | 28348 (impl `script.cpp:20250`) | ~53 | cast on unit |
| `unitskillusepos` | 28349 (impl `:20305`) | ~106 | cast on cell |
| `npcskill` | 28455 (impl `:23286`) | ~97 | NPC casts on a player |
| `bg_monster` | 28404 | 0 | |

Event labels: `OnMyMobDead` appears in 80 files (dominant, death callback per spawned mob). `OnPCKillEvent`, `OnNPCKillEvent`, `OnPCDieEvent` are rare.

## 2. Shared unit model and cast path

* `struct unit_data` (`unit.hpp:22`) is embedded by PC/MOB/PET/NPC/HOM/MER/ELEM alike, so walking, targeting, skill timers etc. are one implementation.
* `unitskilluseid`/`unitskillusepos` and `npcskill` call the same `unit_skilluse_id2` (`unit.cpp:2077`) / `unit_skilluse_pos2` (`unit.cpp:2605`) that mob skills use (`mob.cpp:4436`, `:4485`, both with the row's `casttime`/`cancel`). Players use the unit_skilluse_id/pos wrappers (`unit.cpp:1767`, `:2587`). So "who casts" is a source `block_list`, not a different code path.

## 3. Mob AI is C++; mob skills are data

* Brain: `mob_ai_sub_hard` (`mob.cpp:1841`, state machine: idle/walk/loot/berserk/angry/rush/follow, aggro, target pick, chase, attack), called from the timer path (`:2245`, `:2301`, `:2338`) and the lazy-AI sweep `mob_ai_sub_lazy` (`:2359`, `map_foreachmob` at `:2461`). Skill hook inside it: `mobskill_use(md, tick, -1)` at `:1887`; event-driven calls such as `MSC_SPAWN` (`:1222`) and `MSC_RUDEATTACKED` (`:1951`).
* States (`mob.hpp:94-103`): `MSS_ANY=-1, IDLE, WALK, LOOT, DEAD, BERSERK` (aggressive attacking), `ANGRY` (retaliating, not yet attacked), `RUSH` (chasing after being attacked), `FOLLOW` (chasing unprovoked), `ANYTARGET`.
* Targets (`mob.hpp:438-451`): target, random, self, friend, master, around1..8 (ground skills; 1-4 around self 3x3..9x9, 5-8 around target), `around`=around4.
* Conditions (`mob.hpp:455-481`): always, myhpltmaxrate, myhpinrate, friendhpltmaxrate/inrate, mystatuson/off, friendstatuson/off, attackpcgt/ge, slavelt/le, closedattacked, longrangeattacked, afterskill, skillused, casttargeted, rudeattacked, masterhpltmaxrate, masterattacked, alchemist, spawn, mobnearbygt, groundattacked, damagedgt, trickcasting.
* `db/re/mob_skill_db.txt` (~14.6k non-comment lines; reader `mob_readskilldb` `mob.cpp:6628`, row parser `:6370+`, 19 columns): `MobID,Dummy,State,SkillID,SkillLv,Rate,CastTime,Delay,Cancelable,Target,CondType,CondValue,val1..val5,Emotion,Chat`. Rate is /10000, Delay is ms before the same skill can recast. Negative MobID = global row: -1 all boss types, -2 all normal types, -3 all mobs (header comment lines 1-60). The Dummy column is info only (`Scorpion@NPC_FIREATTACK`). `myhpinrate` uses CondValue as lower bound and val1 as upper bound. File state names are text (`any, idle, walk, dead, loot, attack, angry, chase, follow, anytarget`).

## 4. What instance scripts do with units

Representative: TempleOfDemonGod, WaterGarden, HiddenGarden, DeepForest, MazeofOz, plus others under `npc/re/instances/` (about 9 files use `unitskill*`/`npcskill`, e.g. TempleOfDemonGod, WaterGarden, HiddenGarden, NightmarishJitterbug, RitualOfBlessing, EclageInterior). They only **command** units:
* timed casts of fixed skills on the boss (`unitskilluseid 'boss_id,"NPC_WIDEFREEZE",5;` in TempleOfDemonGod ~685/764);
* HP resets / phase HP via `setunitdata ... UMOB_HP` (TempleOfDemonGod 344-776), mode flips;
* spawn and kill phases of waves (`monster`/`killmonster`, `OnMyMobDead` to advance);
* escort walk routes with arrival labels (`unitwalk 'guide_id,63,290, instance_npcname(...)+"::OnEvent01"`, HiddenGarden 293-412).
They never replace target selection, aggro or chasing.

## 5. Conclusion for our API

* **Shared surface** (usable by NPC scripts, skills, mob skills): spawn/despawn with a death callback; unit actions (walk with arrival callback, warp, stop, attack, cast on unit/cell with cast time/cancel flags); unit property get/set (hp, mode, ...); queries (units in range, count by owner); events (death, spawn, damaged, skill used); timers.
* **NPC-only**: dialog (`mes`/`select`/`input`), shops, player-attached operations.
* **AI-only (engine)**: target selection, state machine, aggro, chase, and picking a skill from the `mob_skill_db` table (state/condition/target/rate/delay evaluation).
