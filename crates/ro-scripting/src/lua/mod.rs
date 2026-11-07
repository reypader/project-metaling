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

mod ctx_mode;
mod error;
mod marshal;
mod parse;

use std::path::Path;

use mlua::{Function, Lua, Table, Value};

use crate::{EffectList, HookBackend, HookCtx, HookError, HookId, Roll, SkillDef, TargetRule};

pub use ctx_mode::CtxMode;
pub use error::LuaLoadError;
use marshal::{ctx_table, ScopedCtx};
use parse::{effect_spec, push_effects};

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
