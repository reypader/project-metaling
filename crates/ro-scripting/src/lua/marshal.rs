//! Builds the Lua side of a hook call: the `ctx` table or userdata and the unit views.

use mlua::{Lua, MetaMethod, Scope, Table, UserData, UserDataFields, UserDataMethods, Value};

use crate::{CastSource, HookCtx, Roll};

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
pub(super) fn ctx_table<'scope, 'env>(
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
pub(super) struct ScopedCtx<'c, 'r> {
    pub(super) ctx: HookCtx<'c>,
    pub(super) roll: &'r mut dyn Roll,
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
