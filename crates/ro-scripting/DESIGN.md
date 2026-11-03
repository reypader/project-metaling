# Project Metaling — Scripting Design (`crates/ro-scripting`)

> **Status (2026-11-03).** §1-§4 are implemented (`specs/effects/`: skills and item effects through Rust
> hooks). §5 is a feature-gated spike (`LuaHooks`), not used by the engine. §6 (shared unit API) and §7
> (NPC dialogs) are later phases. Deferred work: `DEFERRED.md` "effects". Research: catalog `scripting/skills.md`, `scripting/items.md`, `scripting/units.md`,
> `scripting/lua_notes.md`, `scripting/rust-ro-script-architecture.md`.

## 1. Purpose

`ro-scripting` is where game **rules** live: what a skill or an item does. `ro-engine` owns the
**plumbing**: when a cast starts and ends, the checks, consuming costs, applying results, and the packets.
The long-term goal is to move the rules into **Lua** and then change them, so the game can grow away from
RO's mechanics (toward horizontal progression) by editing scripts rather than engine code. rAthena's script
language is not a target: we never parse or run it. Its data comes in only through the `tools/*-import`
importers.

Three rules follow from that goal:

1. **Composable effects.** Most skills and items are a short list of engine-owned building blocks with
   numbers attached. Code is only for what the blocks can't express.
2. **Scripts return effects; they never mutate the world.** A hook reads a context and appends to an
   effect list. The engine applies the list at a defined point. That keeps borrows simple, keeps the tick
   deterministic, and makes the Rust and Lua backends interchangeable.
3. **Backend-agnostic.** The engine sees one trait. The first backend is plain Rust functions. A Lua backend
   implements the same trait.

## 2. Effects, definitions and hooks

~~~text
 imported skill_db / item_db (GPL data)        ro-scripting                        ro-engine
 costs, timing, range, target type,      ┌──────────────────────────┐     ┌──────────────────────────┐
 lifted item `Use:` values ─────────────►│ Definitions               │     │ cast pipeline (cast.rs)  │
                                         │  SkillDef: EffectSpec[]   │◄────│  checks, 0x0b1a, timers, │
                                         │            + hook?        │     │  castend re-check        │
                                         │ HookBackend (Rust | Lua)  │     │                          │
                                         │  hook(ctx) -> Effect[]    │────►│ effect applier (effect.rs│
                                         └──────────────────────────┘     │  damage, heal, spend,    │
                                                                           │  consume, packets)       │
                                                                           └──────────────────────────┘
~~~

* **`EffectSpec`** is data inside a definition, with per-level values (`base + per_level × lv`), e.g.
  `WeaponDamage { ratio: 100+30·lv, hit_pct: 5·lv }` for Bash, `Heal { to: Caster, hp: 5 }` for First Aid.
* **`Effect`** is the resolved form the engine applies: `WeaponDamage`, `Heal`, `SpendSp`, `SpendHp`,
  `SpendZeny`, `ConsumeItem`. `ApplyStatus`, `Teleport` and `Spawn` are reserved names, added by the plans
  that need them. Effects live in a reusable `SmallVec` buffer; the hot path doesn't allocate.
* **`SkillDef`** = skill id + effect specs + an optional hook. **Definitions hold effects and hooks only.**
  Costs, cast and delay times, range and target type come from the imported `skill_db`, and potion values
  from the item importer's `Use:` field. That keeps rAthena-derived numbers in GPL data, read from disk,
  and out of code and out of our Lua files (CLAUDE.md licensing). Formula *facts* (Bash's ratio, First
  Aid's 5 HP, the renewal Heal formula) are allowed in definitions and hooks.
* **Hooks** are named. Definitions refer to them by name; the registry turns names into a `HookId` index
  once, so a cast never does a string lookup. A hook gets a **`HookCtx`**: small `Copy` views of the caster
  and target (level, job, battle STR..LUK, HP/SP and max, MATK range), skill id and level, the
  `CastSource`, up to four integer args (item heal ranges), and the caster's skill levels through a
  `SkillLevels` trait. It also gets a `Roll` (the engine's seeded RNG) and the output list.
* **Costs first.** The engine checks the data costs, then puts the `Spend*` effects at the front of the
  list. A hook may append more costs.
* **Determinism.** Rolls are drawn in a documented order (the weapon hit roll, then variance; Heal's MATK
  roll is its own draw). The same seed and inputs give the same effects from either backend.
* **Errors.** A hook error is logged and the cast fails with no effects applied. Nothing panics.

### 2.1 Crate boundaries

`ro-scripting` depends only on `ro-core` (plus `smallvec`, and `mlua` behind the `lua` feature). It never
sees `ro-db` or `ro-mechanics` types: the engine builds the views. `ro-engine` depends on `ro-scripting`
and implements `SkillLevels` and `Roll` for it. Damage and stat formulas stay in `ro-mechanics`:
`Effect::WeaponDamage` carries a ratio and a hit modifier that the engine feeds into
`ro_mechanics::calc_weapon_attack` through an `AttackContext`.

## 3. Casters and sources

Players, mobs, NPC scripts and items all cast through the same rAthena path (`unit_skilluse_id2` →
`skill_castend_id`). Ours is one entry point too:
`cast(caster, skill, level, target, CastSource)`.

| `CastSource` | Requirements | Cast time and delay | Status |
| :--- | :--- | :--- | :--- |
| `Learned` | SP, zeny, weapon… from `skill_db`, checked at cast start and again at castend, consumed at castend | `skill_db` + renewal cast formula + `delayfix` | first slice |
| `ItemSkill(ItemId)` | skipped (`itemskill`, Fly Wing, scrolls); the delayed-consume item is deleted at cast begin | `skill_db` | later |
| `Mob` | none | the `mob_skill_db` row, unchanged | later (needs mob AI) |
| `ItemUse(ItemId)` | not a cast: an item's own use script (a potion), run from `0x0439` | none | first slice |

A Fly Wing makes two calls: `ItemUse(601)` for its script, which will return a cast effect, then
`ItemSkill(601)` for the teleport cast the client sends back (settled 2026-11-01).

## 4. Unit handles (settled 2026-10-31)

The old open note asked how `ro-scripting` names an engine entity without a reverse dependency. Settled:

* **Now:** effects address units relatively, `Who::{Caster, Target}`. The first slice needs nothing more.
* **When queries or spawns arrive** (area effects, the shared unit API in §6): add a wire-gid newtype
  `UnitId(u32)` to `ro-core`. That is the `bl.id` the client already sees (account id for players, the
  110000000+ pool for mobs and NPCs; the engine has `gid(e)`). The engine maps it to and from `Entity`. Move
  `MobId` and the future `ScriptId` into `ro-core` at the same time.

## 5. Lua backend

Planned shape. The `specs/effects` spike (M9, 2026-11-03) built `LuaHooks` behind the `lua` feature (`src/lua.rs`,
`lua/sm_bash.lua`, `lua/item_heal.lua`). It loads definitions and hooks from a returned table and matches the Rust
backend effect for effect and roll for roll (SCR-LUA-01). The benchmark (`examples/hook_bench.rs`, catalog
`scripting/lua_notes.md` §3.1) puts a bare call at ~35 ns, but a hook call at ~2.5 µs with a fresh ctx table and
~6.5 µs with scoped userdata, against 4-20 ns for Rust. Most of that is building and reading tables, so the production
backend should pass less (see the context bullet). The spike is not wired into the engine.

* **Engine:** `mlua` (MIT), vendored Lua 5.4 to start. Luau is the alternative if sandboxing matters more
  (its `sandbox` mode is Luau-only).
* **One VM per map worker**, not global. No `send` feature, no locks, and GC is paced per map. The VM stays
  out of the shared registries.
* **Definitions move into Lua.** A file returns a table: the effect specs and hook functions for its skills
  or items. The loader converts the table into the same `SkillDef`/`HookId` registry the Rust definitions
  fill, so the engine can't tell the backends apart.
* **The context is passed lean.** Measured (catalog `scripting/lua_notes.md` §3.1): the full ctx table is ~1 µs of
  the 2.5 µs, each scoped function ~230 ns, a returned effect table ~330 ns, and scoped non-`'static` userdata is the
  slowest option (a new metatable per call). Production: plain arguments or lazily read `'static` userdata, effects
  returned through a `'static` emitter or multiple returns, and `roll`/`skill_level` as `'static` functions reaching
  per-call state through `Lua::set_app_data`. Measure again before any per-hit hook.
* **Game state stays in Rust.** Scripts keep no state in globals. That makes reload lossless.
* **Later:** sandboxed globals (no `os`/`io`/`debug`/`package`), memory and instruction limits
  (`set_memory_limit`, `set_hook`), error reporting with file and line, and hot reload. Hot reload builds a
  new state, validates it, and swaps it in at a tick boundary, or keeps the old one if loading fails.
* Our Lua files are MIT and are read from disk. They never carry rAthena-derived values.

## 6. Shared unit API for NPC and monster scripts (later)

rAthena scripts **command** units but never take over AI decisions (catalog `scripting/units.md`): spawn
with a death label, timed casts, mode flips, HP resets, walk routes. Mob skill choice is data
(`mob_skill_db`: state, condition, rate, delay, target). So the later API is shared by NPC scripts, boss
scripts and mob behaviour:

* spawn and despawn (count, master/slave, a death callback); kill or count by owner
* unit actions: walk to (with a completion callback), warp, stop, attack, cast on a unit or cell (the §3
  entry point)
* unit property get and set (HP, mode flags, speed); queries (units in range, by owner, exists)
* events: death (with the killer), spawn, damaged, skill used on or by; timers per owner with a cancel
  handle

These become more `Effect` variants (commands) and more `HookCtx` reads, keyed by `UnitId` (§4).
Target selection, the AI state machine, aggro and skill picking stay in the engine or in data. A Lua
`on_think` hook could replace parts of them later.

## 7. NPC dialogs (later)

The dialog design below predates the effects plan. It still holds, but is now **Lua-only**: an
`mlua` coroutine per conversation instead of an Athena VM.

### 7.1 Non-blocking dialog lifecycle

**Strict rule:** a suspended script never blocks an ECS worker thread or holds a borrow on the world.

1. **Initiation** (`CZ_CONTACTNPC` or an `OnTouch` area): if the player already has a `DialogSession`, the
   click is ignored unless the session can be cancelled. Otherwise a Lua thread starts, bound to
   `(char_id, player, npc)`.
2. **Step execution:** non-yielding calls (text lines, math, variable reads) accumulate into the session's
   outbound buffer.
3. **Suspension at yield points:**
   * `npc:next()` → `ZC_SAY_DIALOG` + `ZC_WAIT_DIALOG` (`0x00B5`), state `AwaitingNext`.
     At 20260219 `mes` uses `0x0972` (catalog PF-21 notes).
   * `npc:select({...})` → `ZC_MENU_LIST` (`0x00B7`), state `AwaitingMenu { option_count }`.
   * `npc:input()` → `ZC_OPEN_EDITDLG` (`0x0142` / `0x01D4`), state `AwaitingInput`.
   * `npc:sleep_ms(n)` → a wake-up tick, state `Sleeping { wake_tick }`.
   * `close`/`end` → `ZC_CLOSE_DIALOG` (`0x00B6`) for close, then the session is dropped.
4. **Resumption or cancellation:** `CZ_REQ_NEXT_SCRIPT` (`0x00B9`), `CZ_CHOOSE_MENU` (`0x00B8`) or
   `CZ_INPUT_EDITDLG` resume the session only if they match its state, so a client can't skip a menu.
   Cancel (`0xFF`), logout, death or a warp drops the session at once.

### 7.2 Variable scopes

| rAthena prefix | Lua (`ctx`) | Lifetime | Storage |
| :--- | :--- | :--- | :--- |
| `.@var` | `local` | the script run | Lua stack |
| `@var` | `ctx.temp.var` | the player's session | in-memory map on the session |
| `var` | `ctx.char.var` | permanent per character | `ScriptVars` component, flushed to PostgreSQL (`JSONB`) |
| `#var` / `##var` | `ctx.account.var` | permanent per account | account-level `JSONB` |
| `$var` | `ctx.world.var` | permanent server-wide | shared map, flushed to a `mapreg`-style table |
| `.var` | `ctx.npc.var` | per NPC, until reload | in `ro-scripting`, keyed by NPC |

Values are `ScriptValue { Int(i64), Str(String) }`.

### 7.3 World commands and read snapshots

Dialog scripts change the world the same way hooks do: by returning commands (warp, give/take items and
zeny, set variables, spawn mobs, enable/disable NPCs). These extend the `Effect` vocabulary from §2 and
use `UnitId` (§4). They are not a separate enum. For checks like "has 500 zeny", the dialog start
attaches a small snapshot (zeny, levels, job, weight, item counts). The script updates that snapshot as it
goes, and the engine commits the authoritative change when it applies the command.
