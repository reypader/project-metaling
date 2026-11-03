//! Times hook calls: Rust against Lua, and the two ways of passing the context to Lua (spec
//! §2.11). Run it in release:
//!
//! ```text
//! cargo run -p ro-scripting --features lua --release --example hook_bench
//! ```
//!
//! Every case goes through `HookBackend::call` (or `SkillDef::resolve` for the declarative
//! Bash) into a reused `EffectList`, like the engine's cast end. The results are written up in
//! catalog `scripting/lua_notes.md` §3.

use std::collections::HashMap;
use std::error::Error;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ro_core::ItemId;
use ro_scripting::{
    CastSource, CtxMode, Definitions, Effect, EffectList, HookBackend, HookCtx, HookError, HookId,
    LuaHooks, Roll, RustHooks, SkillLevels, UnitView,
};

const CALLS: u32 = 200_000;
const RUNS: usize = 5;

struct Xorshift(u64);

impl Roll for Xorshift {
    fn roll(&mut self, n: u32) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        if n == 0 {
            0
        } else {
            u32::try_from(self.0 % u64::from(n)).unwrap_or(0)
        }
    }
}

struct Levels(HashMap<u16, u8>);

impl SkillLevels for Levels {
    fn level(&self, skill: u16) -> u8 {
        self.0.get(&skill).copied().unwrap_or(0)
    }
}

/// Bash as a Rust hook, the same work as the Lua `sm_bash` hook.
struct RustBash;

impl HookBackend for RustBash {
    fn hook_id(&self, name: &str) -> Option<HookId> {
        (name == "sm_bash").then_some(HookId(0))
    }

    fn call(
        &self,
        _hook: HookId,
        ctx: &HookCtx<'_>,
        _roll: &mut dyn Roll,
        out: &mut EffectList,
    ) -> Result<(), HookError> {
        let level = i32::from(ctx.level);
        out.push(Effect::WeaponDamage {
            ratio: 100 + 30 * level,
            hit_pct: 5 * level,
            div: 1,
        });
        Ok(())
    }
}

/// The median of `RUNS` runs of `CALLS` calls, in nanoseconds per call.
fn time(mut call: impl FnMut(&mut EffectList) -> Result<(), HookError>) -> Result<f64, HookError> {
    let mut out = EffectList::new();
    for _ in 0..CALLS / 10 {
        out.clear();
        call(&mut out)?;
    }
    let mut runs = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        for _ in 0..CALLS {
            out.clear();
            call(black_box(&mut out))?;
        }
        runs.push(start.elapsed());
    }
    runs.sort();
    let median = runs.get(RUNS / 2).copied().unwrap_or(Duration::ZERO);
    Ok(median.as_secs_f64() * 1e9 / f64::from(CALLS))
}

fn main() -> Result<(), Box<dyn Error>> {
    let lua_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lua");
    let mut lua = LuaHooks::new();
    let lua_defs = lua.load_file(&lua_dir.join("sm_bash.lua"))?;
    lua.load_file(&lua_dir.join("item_heal.lua"))?;
    let lua_bash = lua.hook_id("sm_bash").ok_or("no sm_bash hook")?;
    let lua_item_heal = lua.hook_id("item_heal").ok_or("no item_heal hook")?;

    let rust = RustHooks::builtin();
    let rust_defs = Definitions::builtin(&rust)?;
    let rust_bash_def = rust_defs.skill(5).ok_or("no Bash")?;
    let lua_bash_def = lua_defs.first().ok_or("sm_bash.lua has no skill")?;
    let rust_item_heal = rust.hook_id("item_heal").ok_or("no item_heal hook")?;

    let caster = UnitView {
        base_level: 50,
        vit: 33,
        int: 17,
        max_hp: 1200,
        max_sp: 150,
        ..UnitView::default()
    };
    let levels = Levels(HashMap::from([(4, 5), (227, 4)]));
    let bash = HookCtx {
        caster,
        target: caster,
        skill: 5,
        level: 10,
        source: CastSource::Learned,
        args: [0; 4],
        skills: &levels,
    };
    let potion = HookCtx {
        skill: 0,
        level: 0,
        source: CastSource::ItemUse(ItemId(501)),
        args: [45, 65, 40, 60],
        ..bash
    };
    let mut roll = Xorshift(0x5eed);

    println!("ns per call, median of {RUNS} runs of {CALLS} calls");
    let ns = time(|out| rust_bash_def.resolve(&bash, &rust, &mut roll, out))?;
    println!("bash       rust definition (specs)      {ns:8.1}");
    let ns = time(|out| lua_bash_def.resolve(&bash, &lua, &mut roll, out))?;
    println!("bash       lua definition (specs)       {ns:8.1}");
    let ns = time(|out| RustBash.call(HookId(0), &bash, &mut roll, out))?;
    println!("bash       rust hook                    {ns:8.1}");
    let ns = time(|out| rust.call(rust_item_heal, &potion, &mut roll, out))?;
    println!("item_heal  rust hook                    {ns:8.1}");
    for mode in [CtxMode::Table, CtxMode::Userdata] {
        lua.set_ctx_mode(mode);
        let ns = time(|out| lua.call(lua_bash, &bash, &mut roll, out))?;
        println!("bash       lua hook, {mode:<8?} ctx         {ns:8.1}");
        let ns = time(|out| lua.call(lua_item_heal, &potion, &mut roll, out))?;
        println!("item_heal  lua hook, {mode:<8?} ctx         {ns:8.1}");
    }
    breakdown()?;
    Ok(())
}

/// Where a Lua hook call's time goes: the same Bash body called through raw `mlua`, adding one
/// part of `LuaHooks::call` at a time.
fn breakdown() -> Result<(), Box<dyn Error>> {
    let lua = mlua::Lua::new();
    let numbers: mlua::Function = lua
        .load("return function(lv) return 100 + 30 * lv, 5 * lv, 1 end")
        .eval()?;
    let list: mlua::Function = lua
        .load(
            "return function(ctx) local lv = type(ctx) == 'number' and ctx or ctx.level
             return { { kind = 'weapon_damage', ratio = 100 + 30 * lv, hit_pct = 5 * lv, div = 1 } } end",
        )
        .eval()?;
    let level = 10u8;
    let mut roll = Xorshift(1);
    let fail = |e: mlua::Error| HookError::Failed(e.to_string());
    let effect = |ratio, hit_pct, div| Effect::WeaponDamage {
        ratio,
        hit_pct,
        div,
    };
    // The list → `Effect` parse that `LuaHooks` does, minus the kind checks.
    let parse = |value: mlua::Table, out: &mut EffectList| -> mlua::Result<()> {
        for entry in value.sequence_values::<mlua::Table>() {
            let entry = entry?;
            let _kind: mlua::LuaString = entry.get("kind")?;
            out.push(effect(
                entry.get("ratio")?,
                entry.get("hit_pct")?,
                entry.get("div")?,
            ));
        }
        Ok(())
    };

    println!("\nbreakdown (raw mlua, bash body)");
    let ns = time(|out| {
        let (ratio, hit_pct, div): (i32, i32, i16) = numbers.call(level).map_err(fail)?;
        out.push(effect(ratio, hit_pct, div));
        Ok(())
    })?;
    println!("number in, 3 numbers out                {ns:8.1}");
    let ns = time(|out| parse(list.call(level).map_err(fail)?, out).map_err(fail))?;
    println!("number in, effect list out              {ns:8.1}");
    let ns = time(|out| {
        let value: mlua::Table = lua.scope(|_| list.call(level)).map_err(fail)?;
        parse(value, out).map_err(fail)
    })?;
    println!("  + empty scope                         {ns:8.1}");
    let ns = time(|out| {
        let value: mlua::Table = lua
            .scope(|scope| {
                let roll_fn = scope.create_function_mut(|_, n: u32| Ok(roll.roll(n)))?;
                let level_fn = scope.create_function(|_, skill: u16| Ok(u32::from(skill)))?;
                list.call((level, roll_fn, level_fn))
            })
            .map_err(fail)?;
        parse(value, out).map_err(fail)
    })?;
    println!("  + 2 scoped functions                  {ns:8.1}");
    let ns = time(|out| {
        let ctx = lua.create_table_with_capacity(0, 2).map_err(fail)?;
        ctx.raw_set("level", level).map_err(fail)?;
        parse(list.call(ctx).map_err(fail)?, out).map_err(fail)
    })?;
    println!("small ctx table (level only), no scope  {ns:8.1}");
    let ns = time(|out| {
        let ctx = lua.create_table_with_capacity(0, 3).map_err(fail)?;
        ctx.raw_set("level", level).map_err(fail)?;
        for side in ["caster", "target"] {
            let unit = lua.create_table_with_capacity(0, 15).map_err(fail)?;
            for field in 0..15 {
                unit.raw_set(field + 1, field).map_err(fail)?;
            }
            ctx.raw_set(side, unit).map_err(fail)?;
        }
        parse(list.call(ctx).map_err(fail)?, out).map_err(fail)
    })?;
    println!("ctx table with 2 x 15-field units       {ns:8.1}");
    let ns = time(|out| {
        let ctx = lua.create_table_with_capacity(0, 3).map_err(fail)?;
        ctx.raw_set("level", level).map_err(fail)?;
        ctx.raw_set("caster", StaticUnit(UnitView::default()))
            .map_err(fail)?;
        ctx.raw_set("target", StaticUnit(UnitView::default()))
            .map_err(fail)?;
        parse(list.call(ctx).map_err(fail)?, out).map_err(fail)
    })?;
    println!("ctx table with 2 'static unit userdata  {ns:8.1}");
    Ok(())
}

/// A `'static` unit userdata: its metatable is built once per VM, unlike the scoped userdata
/// that `CtxMode::Userdata` creates per call.
struct StaticUnit(UnitView);

impl mlua::UserData for StaticUnit {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("vit", |_, this| Ok(this.0.vit));
        fields.add_field_method_get("int", |_, this| Ok(this.0.int));
    }
}
