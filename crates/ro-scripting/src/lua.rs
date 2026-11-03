//! The Lua hook backend spike (spec §2.11, DESIGN §5), behind the `lua` feature. The engine
//! doesn't use it: `verify.sh` doesn't build the feature, and `Lua` isn't `Send`, so a
//! `LuaHooks` stays out of the shared registries.
//!
//! A script returns a table with two optional parts:
//! - `skills`: a list of `{ id, effects = { spec, ... }, hook = "name", reject_undead = bool }`;
//! - `hooks`: `name = function(ctx) ... end`, each returning a list of effects (or nil).
//!
//! An effect spec is `{ kind = "weapon_damage", ratio, hit_pct, div }` or
//! `{ kind = "heal", to, hp, sp, show }`. A number is a fixed value; `{ base, per_level }` is a
//! per-level one. A hook returns effects of the same shapes with plain numbers, or
//! `{ kind = "spend_sp" | "spend_hp" | "spend_zeny", amount }`. `to` is `"caster"` or
//! `"target"`; `show` is `"skill"` or `"silent"`.
//!
//! A hook's `ctx` has `skill`, `level`, `source` (`"learned"`, `"item_skill"`, `"mob"`,
//! `"item_use"`), `item` (the source's item id, or 0), `args` (a list of 4), `caster` and
//! `target` (the `UnitView` fields by name), and the methods `ctx:roll(n)` (the engine's roll)
//! and `ctx:skill_level(id)`. How `ctx` is built is the [`CtxMode`].

use std::fmt;
use std::path::Path;

use mlua::{
    Function, Lua, MetaMethod, Scope, Table, UserData, UserDataFields, UserDataMethods, Value,
};

use crate::{
    CastSource, Effect, EffectList, EffectSpec, HealShow, HookBackend, HookCtx, HookError, HookId,
    LevelValue, Roll, SkillDef, TargetRule, Who,
};

/// How a hook call passes its context to Lua (the spike measures both).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CtxMode {
    /// A fresh table per call, with plain values and the two methods as scoped functions.
    #[default]
    Table,
    /// One scoped userdata per call that reads the context on access.
    Userdata,
}

/// A script failed to load: a Lua error, or a table of the wrong shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuaLoadError(pub String);

impl fmt::Display for LuaLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LuaLoadError {}

/// Hooks written in Lua, in one VM.
///
/// Not for production as is: the VM has the full standard library (`io`, `os`), with no memory
/// or instruction limits (`DEFERRED.md` "effects", Lua).
pub struct LuaHooks {
    lua: Lua,
    hooks: Vec<(String, Function)>,
    mode: CtxMode,
}

impl Default for LuaHooks {
    fn default() -> Self {
        Self::new()
    }
}

impl LuaHooks {
    /// An empty backend with the Lua standard library, passing contexts as tables.
    pub fn new() -> Self {
        Self {
            lua: Lua::new(),
            hooks: Vec::new(),
            mode: CtxMode::default(),
        }
    }

    pub fn set_ctx_mode(&mut self, mode: CtxMode) {
        self.mode = mode;
    }

    /// Runs the script at `path`, keeps its hooks and returns its skill definitions, with hook
    /// names bound to this backend.
    pub fn load_file(&mut self, path: &Path) -> Result<Vec<SkillDef>, LuaLoadError> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| LuaLoadError(format!("{}: {e}", path.display())))?;
        self.load_source(&path.display().to_string(), &source)
    }

    /// [`Self::load_file`] for a script in memory; `name` shows in error messages. Nothing is
    /// kept when loading fails.
    pub fn load_source(&mut self, name: &str, source: &str) -> Result<Vec<SkillDef>, LuaLoadError> {
        let lua_error = |e: mlua::Error| LuaLoadError(format!("{name}: {e}"));
        let shape = |message: String| LuaLoadError(format!("{name}: {message}"));
        let root: Table = self
            .lua
            .load(source)
            .set_name(format!("@{name}"))
            .eval()
            .map_err(lua_error)?;

        let mut added = Vec::new();
        if let Some(hooks) = root.get::<Option<Table>>("hooks").map_err(lua_error)? {
            for pair in hooks.pairs::<String, Function>() {
                added.push(pair.map_err(lua_error)?);
            }
        }
        // `pairs` order isn't stable; sort so the ids are.
        added.sort_by(|a, b| a.0.cmp(&b.0));
        for (hook, _) in &added {
            if self.hook_id(hook).is_some() {
                return Err(shape(format!("hook {hook} is already loaded")));
            }
        }
        let first = self.hooks.len();
        let id_of = |hook: &str| -> Option<HookId> {
            let index = self
                .hooks
                .iter()
                .chain(&added)
                .position(|(n, _)| n == hook)?;
            u16::try_from(index).ok().map(HookId)
        };

        let mut skills = Vec::new();
        if let Some(list) = root.get::<Option<Table>>("skills").map_err(lua_error)? {
            for entry in list.sequence_values::<Table>() {
                let entry = entry.map_err(lua_error)?;
                let id: u16 = entry.get("id").map_err(lua_error)?;
                let mut effects = Vec::new();
                if let Some(specs) = entry.get::<Option<Table>>("effects").map_err(lua_error)? {
                    for spec in specs.sequence_values::<Table>() {
                        let spec = spec.map_err(lua_error)?;
                        effects.push(
                            effect_spec(&spec).map_err(|e| shape(format!("skill {id}: {e}")))?,
                        );
                    }
                }
                let hook = match entry.get::<Option<String>>("hook").map_err(lua_error)? {
                    Some(hook) => Some(
                        id_of(&hook)
                            .ok_or_else(|| shape(format!("skill {id}: no hook named {hook}")))?,
                    ),
                    None => None,
                };
                let reject_undead = entry
                    .get::<Option<bool>>("reject_undead")
                    .map_err(lua_error)?
                    .unwrap_or(false);
                skills.push(SkillDef {
                    id,
                    effects,
                    hook,
                    target: TargetRule { reject_undead },
                });
            }
        }
        if u16::try_from(first + added.len()).is_err() {
            return Err(shape("too many hooks".to_owned()));
        }
        self.hooks.extend(added);
        Ok(skills)
    }
}

impl HookBackend for LuaHooks {
    fn hook_id(&self, name: &str) -> Option<HookId> {
        let index = self.hooks.iter().position(|(n, _)| n == name)?;
        u16::try_from(index).ok().map(HookId)
    }

    fn call(
        &self,
        hook: HookId,
        ctx: &HookCtx<'_>,
        roll: &mut dyn Roll,
        out: &mut EffectList,
    ) -> Result<(), HookError> {
        let (name, function) = self
            .hooks
            .get(usize::from(hook.0))
            .ok_or(HookError::UnknownHook(hook))?;
        let failed = |e: mlua::Error| HookError::Failed(format!("{name}: {e}"));
        let result: Value = self
            .lua
            .scope(|scope| {
                let arg = match self.mode {
                    CtxMode::Table => Value::Table(ctx_table(&self.lua, scope, ctx, roll)?),
                    CtxMode::Userdata => {
                        Value::UserData(scope.create_userdata(ScopedCtx { ctx: *ctx, roll })?)
                    }
                };
                function.call(arg)
            })
            .map_err(failed)?;
        // Like a Rust hook, a failed call adds nothing.
        let len = out.len();
        push_effects(result, out).map_err(|e| {
            out.truncate(len);
            failed(e)
        })
    }
}

/// The `UnitView` fields a script can read, by name.
const UNIT_FIELDS: [&str; 15] = [
    "base_level",
    "job",
    "str",
    "agi",
    "vit",
    "int",
    "dex",
    "luk",
    "hp",
    "max_hp",
    "sp",
    "max_sp",
    "matk_base",
    "weapon_matk",
    "weapon_level",
];

fn unit_field(unit: &crate::UnitView, name: &str) -> Option<i64> {
    Some(match name {
        "base_level" => i64::from(unit.base_level),
        "job" => i64::from(unit.job),
        "str" => i64::from(unit.str),
        "agi" => i64::from(unit.agi),
        "vit" => i64::from(unit.vit),
        "int" => i64::from(unit.int),
        "dex" => i64::from(unit.dex),
        "luk" => i64::from(unit.luk),
        "hp" => i64::from(unit.hp),
        "max_hp" => i64::from(unit.max_hp),
        "sp" => i64::from(unit.sp),
        "max_sp" => i64::from(unit.max_sp),
        "matk_base" => i64::from(unit.matk_base),
        "weapon_matk" => i64::from(unit.weapon_matk),
        "weapon_level" => i64::from(unit.weapon_level),
        _ => return None,
    })
}

fn source_fields(source: CastSource) -> (&'static str, u32) {
    match source {
        CastSource::Learned => ("learned", 0),
        CastSource::ItemSkill(item) => ("item_skill", item.0),
        CastSource::Mob => ("mob", 0),
        CastSource::ItemUse(item) => ("item_use", item.0),
    }
}

fn args_table(lua: &Lua, args: [i32; 4]) -> mlua::Result<Table> {
    let table = lua.create_table_with_capacity(4, 0)?;
    for (index, arg) in args.into_iter().enumerate() {
        table.raw_seti(index + 1, arg)?;
    }
    Ok(table)
}

fn unit_table(lua: &Lua, unit: &crate::UnitView) -> mlua::Result<Table> {
    let table = lua.create_table_with_capacity(0, UNIT_FIELDS.len())?;
    for name in UNIT_FIELDS {
        table.raw_set(name, unit_field(unit, name))?;
    }
    Ok(table)
}

/// [`CtxMode::Table`]: the context as a fresh table, with `roll` and `skill_level` as functions
/// that live until the call returns.
fn ctx_table<'scope, 'env>(
    lua: &Lua,
    scope: &'scope Scope<'scope, 'env>,
    ctx: &'env HookCtx<'env>,
    roll: &'env mut dyn Roll,
) -> mlua::Result<Table> {
    let table = lua.create_table_with_capacity(0, 9)?;
    let (source, item) = source_fields(ctx.source);
    table.raw_set("skill", ctx.skill)?;
    table.raw_set("level", ctx.level)?;
    table.raw_set("source", source)?;
    table.raw_set("item", item)?;
    table.raw_set("args", args_table(lua, ctx.args)?)?;
    table.raw_set("caster", unit_table(lua, &ctx.caster)?)?;
    table.raw_set("target", unit_table(lua, &ctx.target)?)?;
    table.raw_set(
        "roll",
        scope.create_function_mut(move |_, (_ctx, n): (Value, u32)| Ok(roll.roll(n)))?,
    )?;
    let skills = ctx.skills;
    table.raw_set(
        "skill_level",
        scope.create_function(move |_, (_ctx, skill): (Value, u16)| Ok(skills.level(skill)))?,
    )?;
    Ok(table)
}

/// [`CtxMode::Userdata`]: the context behind one userdata that lives until the call returns.
struct ScopedCtx<'c, 'r> {
    ctx: HookCtx<'c>,
    roll: &'r mut dyn Roll,
}

impl UserData for ScopedCtx<'_, '_> {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("skill", |_, this| Ok(this.ctx.skill));
        fields.add_field_method_get("level", |_, this| Ok(this.ctx.level));
        fields.add_field_method_get("source", |_, this| Ok(source_fields(this.ctx.source).0));
        fields.add_field_method_get("item", |_, this| Ok(source_fields(this.ctx.source).1));
        fields.add_field_method_get("args", |lua, this| args_table(lua, this.ctx.args));
        fields.add_field_method_get("caster", |_, this| Ok(LuaUnit(this.ctx.caster)));
        fields.add_field_method_get("target", |_, this| Ok(LuaUnit(this.ctx.target)));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("roll", |_, this, n: u32| Ok(this.roll.roll(n)));
        methods.add_method("skill_level", |_, this, skill: u16| {
            Ok(this.ctx.skills.level(skill))
        });
    }
}

/// A unit as userdata, for [`CtxMode::Userdata`].
struct LuaUnit(crate::UnitView);

impl UserData for LuaUnit {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |_, this, name: mlua::LuaString| {
            Ok(unit_field(&this.0, &name.to_str()?))
        });
    }
}

fn level_value(value: Value) -> mlua::Result<LevelValue> {
    match value {
        Value::Table(table) => Ok(LevelValue::new(
            table.get::<Option<i32>>("base")?.unwrap_or(0),
            table.get::<Option<i32>>("per_level")?.unwrap_or(0),
        )),
        Value::Nil => Ok(LevelValue::fixed(0)),
        Value::Integer(n) => i32::try_from(n)
            .map(LevelValue::fixed)
            .map_err(|_| mlua::Error::runtime(format!("{n} is out of range"))),
        other => Err(mlua::Error::runtime(format!(
            "expected a number or {{ base, per_level }}, got {}",
            other.type_name()
        ))),
    }
}

fn who(table: &Table) -> mlua::Result<Who> {
    match table.get::<String>("to")?.as_str() {
        "caster" => Ok(Who::Caster),
        "target" => Ok(Who::Target),
        other => Err(mlua::Error::runtime(format!("unknown target {other:?}"))),
    }
}

fn heal_show(table: &Table) -> mlua::Result<HealShow> {
    match table.get::<String>("show")?.as_str() {
        "skill" => Ok(HealShow::Skill),
        "silent" => Ok(HealShow::Silent),
        other => Err(mlua::Error::runtime(format!("unknown show {other:?}"))),
    }
}

fn effect_spec(table: &Table) -> mlua::Result<EffectSpec> {
    match table.get::<String>("kind")?.as_str() {
        "weapon_damage" => Ok(EffectSpec::WeaponDamage {
            ratio: level_value(table.get("ratio")?)?,
            hit_pct: level_value(table.get("hit_pct")?)?,
            div: level_value(table.get("div")?)?,
        }),
        "heal" => Ok(EffectSpec::Heal {
            to: who(table)?,
            hp: level_value(table.get("hp")?)?,
            sp: level_value(table.get("sp")?)?,
            show: heal_show(table)?,
        }),
        other => Err(mlua::Error::runtime(format!(
            "unknown effect spec kind {other:?}"
        ))),
    }
}

/// Appends a hook's returned list (nil = no effects) to `out`.
fn push_effects(result: Value, out: &mut EffectList) -> mlua::Result<()> {
    let list = match result {
        Value::Nil => return Ok(()),
        Value::Table(list) => list,
        other => {
            return Err(mlua::Error::runtime(format!(
                "a hook returns a list of effects, got {}",
                other.type_name()
            )))
        }
    };
    for effect in list.sequence_values::<Table>() {
        let effect = effect?;
        out.push(match effect.get::<String>("kind")?.as_str() {
            "weapon_damage" => Effect::WeaponDamage {
                ratio: effect.get("ratio")?,
                hit_pct: effect.get::<Option<i32>>("hit_pct")?.unwrap_or(0),
                div: effect.get::<Option<i16>>("div")?.unwrap_or(1),
            },
            "heal" => Effect::Heal {
                to: who(&effect)?,
                hp: effect.get::<Option<u32>>("hp")?.unwrap_or(0),
                sp: effect.get::<Option<u32>>("sp")?.unwrap_or(0),
                show: heal_show(&effect)?,
            },
            "spend_sp" => Effect::SpendSp(effect.get("amount")?),
            "spend_hp" => Effect::SpendHp(effect.get("amount")?),
            "spend_zeny" => Effect::SpendZeny(effect.get("amount")?),
            other => {
                return Err(mlua::Error::runtime(format!(
                    "unknown effect kind {other:?}"
                )))
            }
        });
    }
    Ok(())
}
