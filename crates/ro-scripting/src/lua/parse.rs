//! Reads effect specs and hook results back from Lua tables.

use mlua::{Table, Value};

use crate::{Effect, EffectList, EffectSpec, HealShow, LevelValue, Who};

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

pub(super) fn effect_spec(table: &Table) -> mlua::Result<EffectSpec> {
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
pub(super) fn push_effects(result: Value, out: &mut EffectList) -> mlua::Result<()> {
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
