# Specification: Effects foundation (skill and item effects through one scripting interface)

Status: **final for M0** (2026-11-01). Capture: [`reference/catalog/captures/scripting_1.txt`](../../reference/catalog/captures/scripting_1.txt) (rAthena `d4b8e7b`, two clients: A = `skilltest2` map session `#7` casts, B = `skilltest1` `#4` watches; file line = proxy `#` + 10). Write-ups:
- skills: catalog `scripting/skills.md` §14 (§3-§10 hold the source facts, tagged [V]/[I]/[C]);
- potions: `mechanics/inventory.md` §7.5 (§7.1-§7.4 source);
- zeny and GM commands: `mechanics/atcommands.md` "Capture: scripting_1.txt";
- natural regen: `mechanics/stats.md` "Capture: scripting_1.txt (regen and `0x0a27`)".
- supplement: [`scripting_2.txt`](../../reference/catalog/captures/scripting_2.txt) (2026-11-01), catalog `scripting/skills.md` §15: zeny at entry, wrong weapon, wrong target, Heal spam, `0x0b1a` element with an elemental weapon, `0x0088` before a cast while walking, Heal on Undead.

Where this spec says "captured", the capture is the ground truth. **Source-only, not captured:** a cast below `canact` for Heal (the client paces Heal at ~600 ms itself), a partial-zeny cast, a potion faster than `item_use_interval` (the client paces itself at about 120 ms).

Design: `crates/ro-scripting/DESIGN.md` §1-§5 (the interface), `crates/ro-engine/DESIGN.md` §5.1 "Casting", §9.7, §11. Background research: catalog `scripting/items.md`, `scripting/units.md`, `scripting/lua_notes.md`, `scripting/rust-ro-script-architecture.md`.

## Scope

**In:**
- `ro-scripting`: the effect vocabulary, definitions, hooks, the `HookBackend` trait and the Rust hooks.
- Casting four target skills with no cast time: `NV_FIRSTAID` (142), `SM_BASH` (5) without its stun, `MC_MAMMONITE` (42), `AL_HEAL` (28) as a heal only.
- Potion use (`0x0439` → `0x01c8`) for items whose whole script is one `itemheal` or `percentheal`.
- Zeny: load, save, spend, `@zeny`. Plus `@heal`, which the manual check needs.
- A feature-gated Lua spike behind the same trait.

**Out** (§5 and `DEFERRED.md` "effects"): status effects; magic damage; cast times in the client; cooldowns; ground skills; passives; `itemskill`; mob casts; NPC scripts; equip scripts; item boxes; the `0x080e` HP meter; player death.

## 0. Data pipeline

### 0.1 `tools/skill-import` (extend)
- Keep these `skill_db.yml` fields: `CastTime`, `FixedCastTime`, `AfterCastActDelay`, `AfterCastWalkDelay`, `Cooldown`, `HitCount`, `Element`, `DamageFlags`, and from `Requires`: `HpCost`, `ZenyCost`, `Weapon`, `ItemCost`, `State`.
- Per-level lists expand like `SpCost` and `Range` (catalog `db/skills.md` "list expansion"). An unset value is 0, `FixedCastTime` included: rAthena loads an absent `FixedCastTime` as 0, and only an explicit −1 (one NPC skill) triggers renewal's 20 % default split (catalog `db/skills.md` "skill_db.yml cast/requirement fields", `scripting/skills.md` §4.1; corrected 2026-11-01).
- Field shapes: `Element` keeps rAthena's codes, including the specials `Weapon` 12, `Endowed` 13 and `Random` 14 (Bash and Mammonite are `Weapon`). `Requires.Weapon` is a weapon-type set (empty = any). `ItemCost` keeps the aegis name, amount and optional level. `State` is a name. `DamageFlags` and `CastTimeFlags` are flag sets.
- Bump the skill data version. Regenerate `crates/ro-db/data/skill/skill_db.yml` and update `NOTICE.md`. The header test keeps passing.

### 0.2 `tools/item-import` (extend)
- Lift a `Script` that is exactly one statement into a `Use:` field (catalog `scripting/items.md` §5 "Importer notes"):
  - `itemheal <n|rand(a,b)>,<n|rand(c,d)>;` becomes `{ ItemHeal: { Hp: [a, b], Sp: [c, d] } }` (a constant n is `[n, n]`).
  - `percentheal a,b;` becomes `{ PercentHeal: { Hp: a, Sp: b } }`.
  - Trailing `/* … */` comments are allowed. Anything else is not lifted.
- The importer prints how many scripts were lifted and how many were not. Expected: 288 lifted (212 `itemheal`, 76 `percentheal`).
- Bump the item data version, regenerate, update `NOTICE.md`.

## 1. Data API (`ro-db`, root re-exports)
- `SkillData` gains the §0.1 fields, with accessors by level like `sp(level)`: `cast_time`, `fixed_cast_time`, `after_cast_act_delay`, `after_cast_walk_delay`, `cooldown`, `hp_cost`, `zeny_cost`, `hit_count`, `element`, and the requirement sets.
- `ItemData.on_use: Option<ItemUse>` (an `ro-db` type), with `ItemUse::ItemHeal { hp: (u32, u32), sp: (u32, u32) }` and `ItemUse::PercentHeal { hp: u8, sp: u8 }`.

## 2. Behaviour

### 2.1 The interface (`ro-scripting`; DESIGN §2)
- **Effect specs and effects.** `EffectSpec` holds per-level values (`base + per_level·lv`). Resolved `Effect`s are `WeaponDamage { ratio, hit_pct, div }`, `Heal { to: Who, hp, sp, show }`, `SpendSp`, `SpendHp`, `SpendZeny` and `ConsumeItem`. `Who` is `Caster` or `Target`.
- **Definitions.** `Definitions::builtin()` holds the four skills and the two item-use kinds. `skill(id)` returns a definition. `resolve(def, ctx, hooks, roll, out)` writes the effects into a caller-owned `EffectList`. **Costs are not in definitions**: the engine reads them from `SkillData` and puts the `Spend*` effects first.
- **Hooks.** A hook is named, and the registry maps names to `HookId` indices once. `RustHooks` implements:
  - `al_heal` (§2.5);
  - `item_heal` (`itemheal` scaling, §2.7);
  - `percent_heal`.
- **Hook calls.** `HookBackend::call(hook, &HookCtx, &mut dyn Roll, &mut EffectList) -> Result<(), HookError>`.
- **Context.** `HookCtx` carries `Copy` `UnitView`s of the caster and target (base level, job, battle STR..LUK, HP/SP and max, `matk_base`, the weapon MATK and level). It also carries skill id and level, `CastSource`, `args: [i32; 4]`, and `&dyn SkillLevels`.
- **Determinism.** Rolls happen in a fixed order, so the same seed gives the same effects. A hook error is logged (`warn!`), and the cast or use fails with no effects. A cast sends nothing; an item use sends only the failure `0x01c8` (rAthena always answers a refused use, or the client gets stuck; session C ruling). It never panics.
- **Sources** (settled 2026-11-01): `CastSource` is `Learned`, `ItemSkill(ItemId)` (a skill cast started by an item, like Fly Wing's `itemskill`), `Mob`, and `ItemUse(ItemId)` (an item's own use script, like a potion). This slice implements `Learned` and `ItemUse`. `ItemSkill` and `Mob` exist in the type but are rejected with `HookError::Unsupported`. A Fly Wing would later make two calls: `ItemUse(601)` for its script, then `ItemSkill(601)` for the teleport cast.
- **Target rule** (settled 2026-11-01): a `SkillDef` carries a declarative target filter that the engine checks with the other target checks at the request (§2.2 step 5). Heal's filter rejects Undead mobs.

### 2.2 Cast request (`0x0438`) and its silent drops
`0x0438` carries `lv u16@2, skill u16@4, target u32@6`, 10 bytes (catalog `scripting/skills.md` §5). The checks run in Stage 1 in this order. Each is a **silent drop** with no packet unless stated (§3, §6, §14):
1. The player is in a state that can't act: sitting, or a cast or chat in progress.
2. The skill is unknown, passive, ground-targeted, or not in the definitions.
3. The learned level is 0, counting only `visible` slots (`SkillSet.level()` also counts hidden slots). The cast level is `min(learned, requested)`, and a request of 0 is raised to 1.
4. `now < canact_at`: too early. rAthena's cause 4 is suppressed by `display_skill_fail` 2 (captured: no `0x0110` anywhere).
5. Target:
   - A self skill (`inf` 4, First Aid) always targets the caster, whatever the client sent.
   - Bash and Mammonite need a live mob. A **player** target is not refused here: the cast begins and fails at castend (§2.4 step 1; captured with Pierce on a player).
   - Heal takes the caster, another player or a live mob, except an Undead mob (its `SkillDef` target filter): silent, nothing sent or spent.
   - A target that doesn't exist or is in another map is dropped silently.
6. Requirements at cast begin, each with a `0x0110` (skill, btype 0, itemId 0, flag 0, cause), SELF, in rAthena's source order (catalog §3, §6; only SP-before-zeny is captured). They run **before the range check** (`unit.cpp:2324` vs `:2362`): out of range with too little SP still gets cause 1, and nothing is stored as a step action.
   - Weight at 90 % of the max or more (`major_overweight_rate`): cause 9.
   - HP at or below `hp_cost(lv)` (no slice skill has one): cause 2.
   - Weapon not allowed: cause 6 (captured: no `0x0b1a`, nothing spent).
   - SP below `sp(lv)`: cause **1**.
   - Zeny below `zeny_cost(lv)`: cause **5**. SP is checked first (captured: SP 4 with 4000 zeny gave cause 1).
   - A skill with an `ItemCost` is dropped silently (no slice skill has one; item costs are deferred).
   - A failure changes nothing (a pending step action survives it).
7. Any pending step action ends (rAthena `unit_stop_stepaction`; an attack's too). Then the range: Bash and Mammonite use 1 (`Range −1` with `skillrange_from_weapon` 0), Heal uses 9. The check is the same circular distance and line-of-sight test as the attack range (catalog `scripting/skills.md` §8). Out of range is silent, and the server never walks the player.
   - **Exception (captured, step 4):** if the player is walking and the target is out of `range - 1`, the request is stored as the step action and fires when the current step ends, like the attack's step action (`combat.rs` `arm_step_action`); it then runs the whole request again. The client walks into range itself with `0x035f` and sends `0x0438` mid-walk.
   - **Walking, target within `range - 1`** (captured in `scripting_2.txt`): stop the walk, which sends `0x0088` (`UnitStopped`) to AREA, then cast at once. `0x0088` comes right before `0x0b1a`.

### 2.3 Cast begin (same tick)
- **Stop the auto-attack loop**: clear the attack target and the pending swing, with no packet (captured, step 3). The server never resumes it, and the client re-sends `0x0437` only on the user's next click.
  - A `0x0438` that lands in the tick of a due swing finds the swing already run. That swing moved `canact_at`, so the cast is dropped (captured: 9 such drops; it matches our timers-before-intents order).
- Send **`0x0b1a`** to AREA. Fields:
  - src = caster, dst = target.
  - x = y = 0, delay = 0, disposable 0, attackMT 0.
  - element = the skill's `Element` when it is a real element (0-9): 0 (Neutral) for First Aid, **6 (Holy) for Heal** (captured). The specials `Weapon`/`Endowed`/`Random` (Bash and Mammonite are `Weapon`) are sent as **0**, whatever the weapon: captured with a Fire weapon and with a Fire endow (`scripting_2.txt`), and the source sends 0 for anything outside 0-9.
- **Gates:**
  - `canact_at = max(canact_at, now + amotion + 100, now + max(cast_time, 100)) + 100`, rAthena's cast-begin plus cast-delay value (catalog `scripting/skills.md` §4). With cast time 0 that is `now + amotion + 200`. Castend (§2.4 step 3) takes 100 off again, so the net gate is the captured `now + amotion + 100`.
  - `attackable_at = max(attackable_at, now + adelay)`: the next auto-attack swing waits a full `adelay`. Both fields already exist on `AttackState` (`crates/ro-engine/src/combat.rs:82`), like rAthena's `attackabletime` and `canact_tick`.
- Schedule `CastEnd { caster, generation }` in the combat timer heap at `now + cast_time`. With a cast time of 0 (all four skills) it runs at once, in the same tick.
  - The renewal cast-time formula (`ro-mechanics`) is implemented and engine-tested even though no slice skill uses it. With a cast time above 0, a cast-cancel or move would invalidate the generation; cancel `0x01b9` itself is deferred.

### 2.4 Castend
1. **Re-check** the target (alive, same map): gone drops the cast silently. Then `skill_check_condition_castend`: the HP cost (cause 2), then the weapon (cause 6), each a `0x0110` to SELF; only reachable with a cast time. **SP and zeny are not re-checked** (catalog §3 step 6).
   - **Target relation:** an attack skill (Bash, Mammonite) on a player sends `0x0110` (skill, cause **0**) to SELF after the `0x0b1a` already sent, and stops. Nothing is spent, and the gates stay at their cast-begin values (`canact_at` keeps its extra 100 ms; catalog §6). Captured with Pierce on a player: `0x0b1a` AREA, then `0x0110` cause 0, no SP push.
2. **Consume, in order:**
   - SP, pushing `0x00b0` `SP_SP` (var 7) with the new value;
   - zeny, pushing `0x00b1` `SP_ZENY` (var 20) with the new total (u32).
   Both are spent even when the hit misses.
3. **`canact_at = max(now + AfterCastActDelay(lv), canact_at − 100)`.** For Heal this is `max(now + 500, now + amotion + 100)`.
4. **`0x0983` EFST_POSTDELAY** goes to AREA only when the delay is above 0 (Heal only): efst **46**, AID = **the caster**, flag 1, total = remain = delay, val1-3 = 0 (captured).
5. Resolve the effects (§2.1) and apply them (§2.5-§2.6). Packet order on the caster (captured): `0x0b1a` → SP → zeny → `0x0983` → `0x09cb`/`0x01de` → HP push.

### 2.5 The four skills
| Skill | Definition | Packets after the costs (captured) |
| :--- | :--- | :--- |
| First Aid (142, lv 1, SP 3) | `Heal { to: Caster, hp: 5 }` | `0x09cb` (skill 142, value 5, target = src = caster, result 1) AREA, then `0x00b0` `SP_HP` (var 5) SELF if HP changed |
| Bash (5) | `WeaponDamage { ratio: 100 + 30·lv, hit_pct: 5·lv }` | `0x01de` AREA, then the HP lands at +amotion (§2.6) |
| Mammonite (42) | `WeaponDamage { ratio: 100 + 50·lv, hit_pct: 0 }` | as Bash; zeny `100·lv` comes from `ZenyCost` |
| Heal (28) | hook `al_heal` → `Heal { to: Target, hp }` | `0x09cb` (skill 28, value = **the computed, unclamped heal**, target, src) AREA; then the target's HP push |

- **Heal amount** (catalog `scripting/skills.md` §7; matches the capture's 64 at lv 10 and 10 at lv 1 with BaseLv 13 and all stats 1):
  - `hp = (BaseLv + INT) / 5 * 30 * lv / 10`, integer math left to right.
  - Then `+ MATK`. `MATK = INT + INT/2 + DEX/5 + LUK/3 + BaseLv/4 + 5·SPL`, plus the weapon MATK roll: `w` = weapon MATK, `variance = w·wlv/10`, `min += w − variance`, `max += w + variance`, and `+ min + roll(max − min)` when `max > min`.
  - Result `max(1, hp)`. Meditatio and the other heal bonuses are skipped (deferred).
- **Heal targets:**
  - **Another player:** the caster gets `0x09cb`; the target gets `0x09cb` (AREA) and its own `0x00b0` `SP_HP`.
  - **A mob:** the mob's HP rises (clamped to max), and `0x0977` (MobHpBar) goes to its damage-log members in the same tick (captured: A only).
  - **Full HP:** `0x09cb` still carries the full computed heal, and no HP push follows (captured for First Aid and Heal, self and other).
- **Undead targets:** Heal on a mob whose element is Undead is dropped silently at the request, by the definition's target filter (§2.1), before `0x0b1a` and before any cost. This is a deviation: rAthena turns it into holy damage at castend (captured in `scripting_2.txt`: `0x0b1a`, SP, `0x0983`, then `0x01de` skill 28 with the damage), which needs the magic pipeline (deferred).
- **Heals are vitals only.** HP changes inline at castend, and the player is not marked dirty (DESIGN §9.7).

### 2.6 Weapon-skill damage (Bash, Mammonite)
- **The calculation:** `ro_mechanics::calc_weapon_attack` takes an `AttackContext { ratio, hit_pct, can_crit, can_lucky_dodge }`. `AttackContext::NORMAL` (ratio 100, hit 0, both flags on) leaves the normal attack and its roll order unchanged.
- **Skill rules** (catalog `scripting/skills.md` §9):
  - No lucky dodge and no crit.
  - `hit_rate += hit_rate · hit_pct / 100` on the renewal `hit − flee`, before the `[5, 100]` clamp.
  - The ratio multiplies the whole of status ATK ×2 + weapon + mastery, after P.ATK and before DEF.
  - Always `BF_SHORT`.
  - Roll order: hit roll, then the weapon variance.
- **Shared hit path:** the hit is computed by the same code as a swing (a shared `weapon_hit`, split out of `combat.rs` `swing()`). The skill sends **`0x01de`** to AREA instead of `0x08c8`, with these fields:
  - skill, src, dst, tick;
  - sdelay = **the caster's amotion**, ddelay = **the target's dmotion** (0 on a miss);
  - damage (0 on a miss), lv, div 1, type **8**.
- **HP application:** the HP lands at `now + amotion` through the same `PendingHit` → `apply_hit` path. That covers the damage log, `0x0977`, death, exp and level-ups, and the delayed-hit skip rules. A miss queues no hit and sends no `0x0977` (captured).
- **Ammo:** none is spent (Bash and Mammonite are melee skills).
- **Deviation kept from combat:** size and element modifiers stay at 100 %. So the capture's Knife-vs-Poring values (Dagger vs Medium 75 %) are not reproduced; the unarmed rows (Bash lv 1 = 11, Mammonite lv 10 = 46 on a Poring) are.

### 2.7 Potion use (`0x0439`)
`0x0439` carries `index u16@2` (slot + 2) and `AID u32@4`, and only the index is read. Gates, in order (catalog `mechanics/inventory.md` §7.2):
1. The index is out of range or the slot is empty: silent.
2. The item is not usable, has no `on_use`, or `now < canuseitem_at`: send the failure `0x01c8` (index, item id, AID, amount 0, result 0) to SELF.
   - `canuseitem_at = now + item_use_interval`, where `item_use_interval` is a new `BattleConfig` field (default 100 ms).
   - Class, level and gender use restrictions are checked like equip restrictions where the data has them.
   - An item without a lifted `Use` is refused. This is a deviation: rAthena runs any script.

Walking, attacking and sitting are not gates: a potion works in each and stops neither the walk nor the auto-attack loop (source, confirmed from gameplay by the user 2026-11-01). No `0x0088`, no `stop_attack`.

On success (captured order):
1. `0x01c8` to **AREA**: index, item id, AID, amount **remaining**, result 1.
2. The item is deleted with no `0x07fa`, and `0x00b0` `SP_WEIGHT` (var 24) goes to SELF.
3. Resolve and apply the heal: `0x00b0` `SP_HP` then `SP_SP`, each sent only if the value changed. At full HP the potion is still used up and nothing else is sent.

**Heal amounts:**
- `item_heal`: roll `hp = rand(a, b)` then `sp = rand(c, d)`, both inclusive, HP first.
  - `bonus_hp = 100 + 2·VIT(battle) + 10·SM_RECOVERY + 5·AM_LEARNINGPOTION`.
  - `bonus_sp = 100 + 2·INT(battle) + 10·MG_SRECOVERY + 5·AM_LEARNINGPOTION`.
  - Apply `v = v · bonus / 100` only when bonus ≠ 100 and the result is higher (captured: VIT 51 gave +94 from roll 47).
- `percent_heal`: `max · rate / 100`, at least 1 for a non-zero rate, with no scaling. The rate is of the **max**, not the current value (source; confirmed from gameplay by the user 2026-11-01).
- Item heal-rate bonuses and fame potions are deferred.

### 2.8 Zeny
- **Storage:** `CharacterStatus.zeny: u32` is loaded from and saved to `characters.zeny` (the column already exists in `crates/ro-persistence/sql/01_schema.sql`), on the same play-state save as HP/SP. It is capped at `MAX_ZENY` 1,000,000,000.
- **At map entry: nothing.** Captured (`scripting_2.txt`) with 5000 zeny: no `0x00b1` at map entry or on a map change. The client takes zeny from the **char-select list**: `CHARACTER_INFO.money` (u32 @12), which the char-server fills from `characters.zeny` (today it writes 0).
- **Spending:** Mammonite's `SpendZeny` pushes `0x00b1` with the new total.

### 2.9 GM commands
- **`@zeny <n>`** adds n (which may be negative) and clamps to `[0, MAX_ZENY]`. Reply: `0x00b1`, then our own message line (no rAthena message text). A result outside the range is refused with a message and nothing else.
- **`@heal [hp [sp]]`**: with no arguments, HP and SP go to max. Otherwise each is added and clamped to `[1, max]` for HP (no player death exists) and `[0, max]` for SP. Push `SP_HP` then `SP_SP`, each only if it changed, then the message.
  - rAthena's negative-HP self-hit animation (`0x08c8` type 4) and its death at 0 HP are deviations, deferred with player death.
- Both commands use the existing GM gate (`command.rs`).

### 2.10 Persistence
Zeny (§2.8) is the only new column read. HP and SP after a heal or potion, the smaller stack after a potion, and SP and zeny after a cast are all saved by the existing play-state save. Nothing saves on cast or use.

### 2.11 Lua spike (feature `lua`, not in production)
- `crates/ro-scripting/lua/sm_bash.lua` returns a table: the Bash definition (effect specs) plus a hook function that returns the same effects as the Rust definition.
- `LuaHooks` (`mlua`, vendored Lua 5.4) implements `HookBackend`. Its loader turns the table into a `SkillDef`. For lv 1-10 with the same seed, its effects equal the Rust ones.
- An example `hook_bench` times Rust vs Lua hook calls, comparing a fresh context table per call against `Lua::scope` userdata. The numbers go into catalog `scripting/lua_notes.md`.
- `LuaHooks` is not in `Registries`. `verify.sh` doesn't build the feature, so the spike's checks run by hand.

## 3. Scenarios

| ID | Scenario | Expected |
| :--- | :--- | :--- |
| DB-EFF-01 | Load the shipped skill data | Bash: `sp` 8 at lv 1-5 and 15 at lv 6-10, `zeny_cost` 0. Mammonite: `zeny_cost(10)` = 1000. Heal: `after_cast_act_delay` 500 at every level, element Holy. Header present. |
| DB-EFF-02 | Load the shipped item data | 501 `ItemHeal { hp: (45, 65), sp: (0, 0) }`, 505 SP heal, 500 `PercentHeal`. An item with `itemheal` + `sc_end` has no `on_use`. Header present. |
| DB-EFF-03 | Importer on our own small fixtures | Each shape in §0.2 is lifted, a mixed script is not, and the counts are reported. |
| MECH-EFF-01 | `AttackContext::NORMAL` | Every MECH-CMB vector unchanged (regression guard). |
| MECH-EFF-02 | Skill context | No lucky dodge or crit; ratio after P.ATK and mastery; Bash hit modifier before the clamp. The unarmed capture rows: Bash lv 1 = 11, Mammonite lv 10 = 46 on a Poring. |
| MECH-EFF-03 | Renewal cast time and `delayfix` | Catalog `scripting/skills.md` §4.1: variable × (1 − sqrt((2·DEX + INT)/530)) truncated, plus the fixed part; the 20 % split only for an explicit −1; `IgnoreDex`; the mob passthrough; the worked examples (1000 ms at DEX 1/INT 1 = 924). `delayfix` = `max(AfterCastActDelay, 0)` at defaults (a negative delay is `−delay + amotion`). Plus the user's cast-time capture once recorded. |
| SCR-EFF-01 | Resolve First Aid | `[Heal { Caster, 5 }]`, no roll used. |
| SCR-EFF-02 | Resolve Bash lv 1..10 | `WeaponDamage` ratio 130..400, hit_pct 5..50. |
| SCR-EFF-03 | `al_heal` | BaseLv 13, all stats 1, no weapon: lv 10 = 64, lv 1 = 10. BaseLv 99, INT 99, DEX/LUK 1: lv 10 = 1342, lv 1 = 289 (`scripting_2.txt`). With a weapon MATK, the roll stays within `[min, max)`. |
| SCR-EFF-04 | `item_heal` | Roll 47 at VIT 51 gives 94. Roll 57 at INT 1 gives 58. With bonus 100, the roll is unchanged. |
| SCR-EFF-05 | A hook error | The effects are cleared, the error is returned, nothing panics. |
| PKT-EFF-01 | Decode `0x0438`, `0x0439` | Field offsets as §2.2/§2.7. |
| PKT-EFF-02 | Encode `0x0b1a`, `0x01de`, `0x09cb`, `0x0983`, `0x01c8`, `0x00b1`, `0x0110` | Bytes equal capture frames (e.g. proxy `#1669`, `#1984`, `#6710`, `#6711`, `#5824`). |
| ENG-EFF-01 | First Aid, HP below max | Order `0x0b1a`, SP, `0x09cb` 5, `SP_HP`. Observer: `0x0b1a`, `0x09cb`. |
| ENG-EFF-02 | First Aid at full HP | `0x0b1a`, SP, `0x09cb` 5, no HP push. |
| ENG-EFF-03 | Cast again before `canact_at` | Silent drop (no event). Cast at `amotion + 100` accepted. |
| ENG-EFF-04 | Not enough SP / zeny / wrong weapon / 90 % weight | `0x0110` cause 1 / 5 / 6 / 9, no `0x0b1a`, nothing consumed; SP checked before zeny; out of range with too little SP still gets cause 1. |
| ENG-EFF-05 | Bash hit on a Poring | `0x0b1a` (element 0), SP, `0x01de` (sdelay amotion, ddelay dmotion, type 8). HP, `0x0977` and a kill land at +amotion. |
| ENG-EFF-06 | Bash miss | `0x01de` damage 0, ddelay 0; SP spent; no `0x0977`. |
| ENG-EFF-07 | Bash during an auto-attack loop | The loop stops and is not resumed. A cast in the tick of a due swing is dropped. |
| ENG-EFF-08 | Bash out of range; cast while walking | Out of range: silent drop. Sent mid-walk out of `range − 1`: fires at the step end. Mid-walk within range (self or a mob): `0x0088` then the cast at once. A cast ends a pending attack step action. |
| ENG-EFF-09 | Mammonite | SP then `0x00b1` (new total) then `0x01de`; zeny spent on a miss. |
| ENG-EFF-10 | Heal self, other player, mob, full HP | §2.5: `0x0983` (efst 46, AID caster, 500) before `0x09cb`; unclamped value; the target's HP push; mob `0x0977` to damage-log members; no HP push at full HP. |
| ENG-EFF-11 | Heal on an Undead mob | Silent drop. |
| ENG-EFF-12 | Cast with a cast time (test-only definition) | `0x0b1a` delay = the cast time; the effect at `CastEnd`; a stale generation is ignored. |
| ENG-EFF-13 | Potion at reduced HP | `0x01c8` AREA (remaining amount), `SP_WEIGHT`, `SP_HP`. |
| ENG-EFF-14 | Potion at full HP / SP potion / last potion | Consumed; only `0x01c8` + `SP_WEIGHT`. SP potion pushes `SP_SP`. The last potion empties the slot. |
| ENG-EFF-15 | Potion refused (interval, not usable, no `Use`) | Failure `0x01c8` SELF, nothing consumed. |
| ENG-EFF-16 | `@zeny`, `@heal` | §2.9 pushes, clamps and refusals. |
| ENG-EFF-17 | Bash on a player | `0x0b1a` AREA, then `0x0110` cause 0 SELF; no SP, no damage; `canact_at` = cast begin's `now + amotion + 200`. |
| PER-EFF-01 | Zeny round trip | Saved and loaded; the default is 0. The char list carries it (`money` @12 of `CHARACTER_INFO`); map entry sends no `0x00b1`. |
| BIN-EFF-01 | Over TCP: First Aid, Bash on a spawned mob, a potion | The frames arrive in the §2 order on caster and observer. |
| SCR-LUA-01 | Lua Bash vs Rust Bash, lv 1-10, same seed | Equal effects; `hook_bench` runs. |

## 4. Licensing
- Costs, timing, ranges, elements and potion values are rAthena-derived. They live only in `crates/ro-db/data/` (GPL), arrive through `tools/skill-import` and `tools/item-import`, and are read from disk at runtime.
- Definitions and hooks (Rust now, our Lua files later) hold effect shapes and formula **facts** (ratios, the Heal and `itemheal` formulas, First Aid's 5 HP). That is allowed by CLAUDE.md, written from the catalog, and never copied from rAthena code.
- `mlua` and Lua 5.4 are MIT. Test fixtures are our own.

## 5. Deviations and deferred (moved into `DEFERRED.md` "effects", 2026-11-03)
- **Deviations:**
  - Heal on Undead is dropped, not turned into damage.
  - Items without a lifted `Use` are refused.
  - The size and element modifiers stay at 100 %.
  - `@heal` can't kill, and has no self-hit animation.
  - `0x080e` (the HP meter rAthena sends to `view_hpmeter` GM observers, and to party members) is not sent.
- **Deferred (found in session B):** the castend walk delay (`300 + AfterCastWalkDelay`, catalog §3 step 8); with a cast time above 0, a pending cast is not cleared by a warp, and attacks are not refused while it runs (both belong with cast cancel); skill item costs.
- **Deferred:** everything in `DEFERRED.md` "effects": statuses, magic, cast bars in the client, cast cancel, cooldowns, ground and splash skills, passives, `itemskill`, mob and NPC casts, boxes and other item scripts, equip scripts, heal bonuses, the production Lua backend, sandboxing, hot reload. Also Bash's stun, Meditatio, and the shared NPC/monster unit API.
