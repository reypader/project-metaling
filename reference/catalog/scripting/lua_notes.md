# Lua for game-server scripting in Rust: mlua facts, alternatives, API-shape lessons

Source: web research 2026-10-31 (URLs below) plus the Effects plan R4 findings; no rAthena involvement. Not PACKETVER dependent. §3.1 is our own benchmark (Effects M9 spike, 2026-11-03); the rest is not measured by us. Related: [units.md](./units.md), [rust-ro-script-architecture.md](./rust-ro-script-architecture.md), [skills.md](./skills.md), [items.md](./items.md).

## 1. mlua

* Current version: **0.12.2** (crates.io, released 2026-10-03; 0.12.0 on 2026-07-05; 0.11.6 on 2026-01-27). License MIT. https://crates.io/crates/mlua , https://github.com/mlua-rs/mlua , https://docs.rs/mlua
* Backends (exactly one feature): `lua55 lua54 lua53 lua52 lua51`, `luajit`/`luajit52`, `luau` (auto-vendored), `luau-jit`, `luau-vector4`. `vendored` builds Lua/LuaJIT from source (needs a C compiler; acceptable for us as MIT-compatible). No default features.
* Other features: `send` (`Lua: Send + Sync`, adds bounds on `Function`/`UserData`, uses a lock), `async`, `serde`, `error-send`, `macros`, `anyhow`, `userdata-wrappers`, `module`.
* Sandbox: `Lua::sandbox(true)` is **Luau-only** (read-only globals, isolated per-script env). On PUC Lua/LuaJIT, build the environment table yourself (load chunks with a custom `_ENV` containing only whitelisted functions; omit `os`, `io`, `debug`, `package`, `load*`, `dofile`).
* Limits: `Lua::set_memory_limit` (allocator-level cap), `Lua::set_hook` with instruction-count triggers (abort runaway scripts by returning an error), `set_interrupt` (Luau). Check the docs.rs page for exact signatures when implementing.
* Coroutines: Lua threads map to `mlua::Thread`; `Thread::resume` lets a script yield to the host (fits "yield to the tick": a script yields a wait request, the engine resumes it on a later tick). `async` feature builds async functions on top of the same mechanism.
* GC: incremental/generational GC runs inside the VM; `gc_collect`/`gc_step` can be driven at tick boundaries to bound pauses. Memory limit plus per-tick `gc_step` keeps latency predictable.

## 2. Alternatives (one line each)

* `piccolo`: pure-Rust Lua VM, fuel-bounded and resumable by design, but experimental/incomplete stdlib.
* `rhai`: embedded Rust-like scripting, easy sandbox, not Lua (no reuse of Lua tooling or knowledge).
* `rune`: Rust-flavoured dynamic language with async-ish generators, small ecosystem.
* `koto`: small embeddable language, not Lua, young.
* WASM (wasmtime/wasmi): strongest isolation and fuel metering, best for untrusted mods; heavier boundary and tooling cost.

## 3. Performance facts

* LuaJIT is substantially faster than PUC Lua on compute loops; Luau sits between, with its own typed fast paths. Pure-Lua logic is rarely the bottleneck for skill/item effect hooks that run a few times per event.
* Crossing the Rust<->Lua boundary is the cost that matters (userdata creation, metatable lookups, string conversion, table construction). **No published Rust->Lua call cost exists** that we could find (mlua links script-bench-rs but gives no per-call numbers). We measured it ourselves in §3.1. Measure again before committing to per-tick or per-hit calls.
* Guidance until measured: avoid fine-grained userdata in inner loops; make coarse calls with plain values or small tables; pre-resolve functions once (`Function` handle) instead of looking up by name per call.

### 3.1 Measured: the M9 spike (2026-11-03)

`crates/ro-scripting/examples/hook_bench.rs`, run with `cargo run -p ro-scripting --features lua --release --example hook_bench`. mlua 0.12.2, vendored Lua 5.4, Apple M3 Pro, rustc 1.95, release. The figures are the median of 5 runs of 200 000 calls, in ns per call, into a reused `EffectList`. Two runs agreed within 3 %.

| Case | ns/call |
| :--- | ---: |
| Bash, Rust definition (effect specs, no hook) | 4.5 |
| Bash, definition loaded from `sm_bash.lua` (specs, so no Lua at call time) | 4.5 |
| Bash, Rust hook (`dyn HookBackend`) | 4.3 |
| `item_heal`, Rust hook (2 rolls, 3 skill-level lookups) | 19.8 |
| Bash, `LuaHooks`, `CtxMode::Table` (fresh ctx table + 2 scoped functions) | 2 520 |
| `item_heal`, `LuaHooks`, `CtxMode::Table` | 2 910 |
| Bash, `LuaHooks`, `CtxMode::Userdata` (one scoped non-`'static` userdata) | 6 520 |
| `item_heal`, `LuaHooks`, `CtxMode::Userdata` | 7 460 |

Where the time goes (raw mlua, the same Bash body; each row is a separate case):

| Case | ns/call |
| :--- | ---: |
| number in, 3 numbers out (the bare Rust→Lua→Rust call) | 35 |
| number in, a list of one effect table out, parsed by string keys | 370 |
| the same inside an empty `Lua::scope` | 370 |
| the same + 2 scoped functions (`create_function`/`_mut`) as args | 830 |
| a ctx table holding only `level` | 480 |
| a ctx table with 2 × 15-field unit tables (string keys) | 1 330 |
| a ctx table with 2 `'static` unit userdata (metatable registered once) | 760 |

Findings:
* **The call itself is cheap (~35 ns); building and reading tables is not.** Returning one effect as a table costs ~330 ns, a 15-field unit table ~450 ns, each scoped function ~230 ns. A Lua hook is ~600× a Rust hook, but 2.5 µs is still fine for per-cast or per-item-use calls (a busy map does hundreds per second, not millions). It's not fine for per-tick or per-hit hooks across many units.
* **Scoped non-`'static` userdata is the slowest option** (6.5 µs): mlua builds a new metatable for each one (documented on `Scope::create_userdata`). `'static` userdata with a once-registered metatable is cheaper than a field-by-field table (760 vs 1 330 ns for two units).
* `Lua::scope` itself is free. The cost is what's created inside it.
* **For the production backend** (none of these are built yet): pass the few values a hook reads as plain arguments, or as lazily filled `'static` userdata, never a full snapshot table; let hooks return effects through a `'static` emitter function or multiple returns instead of a list of tables; give `roll`/`skill_level` as `'static` functions that reach the per-call state through `Lua::set_app_data`, instead of making scoped closures per call. Measure again before adding per-hit hooks.
* Determinism: with the same seed, the Lua `sm_bash` and `item_heal` hooks give the same effects and the same roll sequence as the Rust ones (SCR-LUA-01, `crates/ro-scripting/tests/lua_tests.rs`), including a constant `itemheal` side that draws no roll.

## 4. VM topology

One VM per map worker keeps everything lock-free (no `send` feature, no mutex around the Lua state). A global VM needs `send` plus a lock or a dedicated script thread with message passing. Per-map VMs cost memory per VM and duplicate loaded scripts, but isolate failures and hot reload to one map.

## 5. Prior art

* **Eluna** (Lua engine for MaNGOS/TrinityCore/AzerothCore): event registry keyed by (entity id or global, event enum), handlers registered from Lua (`RegisterPlayerEvent` etc.), scripts mutate game objects directly through bound userdata. Works, but couples scripts to live engine state and makes reload/ordering hazardous. https://github.com/ElunaLuaEngine/Eluna
* **Official RO client** ships Lua for homunculus/mercenary AI (`AI.lua`, `Util.lua`); this is client-side, runs per-unit commands (move, attack, skill). No evidence found that any official server uses Lua. 
* rAthena's own script language (see [units.md](./units.md)) uses ids and labels as handles, with event labels as callbacks.

## 6. API-shape lessons

* Register by **id** (skill id, item id, mob id), not by object reference; engine looks the handler up at call time.
* Pass a **ctx per call** (small Copy view of caster/target plus read-only queries); never hold references across calls.
* Prefer a **command buffer**: scripts push commands (heal, apply status, spawn, cast), the engine applies them after the call, in order. Keeps scripts pure over a snapshot, makes tests easy, and avoids borrowing world state from inside Lua. Direct mutation (Eluna style) is simpler to write but fragile.
* Bound every call (instruction hook plus memory limit); a script error disables that call, never the tick.

## 7. Hot reload patterns

Build a fresh Lua state (or load into a new environment), validate it (compile, required registrations present), then swap at a tick boundary. Keep all game state in Rust so nothing lives in the old VM; resolve handlers by id at call time so swapped tables take effect immediately. In-flight coroutines from the old VM should finish on the old state or be cancelled explicitly.
