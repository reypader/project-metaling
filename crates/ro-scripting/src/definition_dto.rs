//! Skill and item-use definitions: effect specs plus an optional hook (DESIGN §2). Costs, cast
//! and delay times, range and target type are not here: they come from the imported data, and
//! the engine puts the `Spend*` effects in front of what a definition resolves to.

use crate::{CastSource, EffectList, EffectSpec, HookBackend, HookCtx, HookError, HookId, Roll};

/// Which targets a skill refuses, beyond what its data target type allows. The engine checks it
/// with the other target checks when the cast is requested: a refused target is a silent drop.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TargetRule {
    /// Undead-element mobs (Heal: rAthena turns it into holy damage, which isn't built yet).
    pub reject_undead: bool,
}

/// One skill's definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDef {
    pub id: u16,
    pub effects: Vec<EffectSpec>,
    pub hook: Option<HookId>,
    pub target: TargetRule,
}

impl SkillDef {
    /// Appends the effects of a cast at `ctx.level`: the specs in order, then the hook's. Only
    /// `CastSource::Learned` is supported. On an error `out` is cleared, so a failed cast
    /// applies nothing (the engine's costs included).
    pub fn resolve(
        &self,
        ctx: &HookCtx<'_>,
        hooks: &dyn HookBackend,
        roll: &mut dyn Roll,
        out: &mut EffectList,
    ) -> Result<(), HookError> {
        if ctx.source != CastSource::Learned {
            out.clear();
            return Err(HookError::Unsupported(ctx.source));
        }
        resolve(&self.effects, self.hook, ctx, hooks, roll, out)
    }
}

/// The use effects the item data can name (`ro_db::ItemUse` without its numbers, which reach
/// the hook as `HookCtx::args`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemUse {
    /// `itemheal`; args `[hp_min, hp_max, sp_min, sp_max]`.
    ItemHeal,
    /// `percentheal`; args `[hp_rate, sp_rate, 0, 0]`.
    PercentHeal,
}

/// One item-use kind's definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemUseDef {
    pub kind: ItemUse,
    pub effects: Vec<EffectSpec>,
    pub hook: Option<HookId>,
}

impl ItemUseDef {
    /// Appends the effects of one use. Only `CastSource::ItemUse` is supported. On an error
    /// `out` is cleared.
    pub fn resolve(
        &self,
        ctx: &HookCtx<'_>,
        hooks: &dyn HookBackend,
        roll: &mut dyn Roll,
        out: &mut EffectList,
    ) -> Result<(), HookError> {
        if !matches!(ctx.source, CastSource::ItemUse(_)) {
            out.clear();
            return Err(HookError::Unsupported(ctx.source));
        }
        resolve(&self.effects, self.hook, ctx, hooks, roll, out)
    }
}

fn resolve(
    effects: &[EffectSpec],
    hook: Option<HookId>,
    ctx: &HookCtx<'_>,
    hooks: &dyn HookBackend,
    roll: &mut dyn Roll,
    out: &mut EffectList,
) -> Result<(), HookError> {
    for spec in effects {
        out.push(spec.resolve(ctx.level));
    }
    if let Some(hook) = hook {
        if let Err(e) = hooks.call(hook, ctx, roll, out) {
            out.clear();
            return Err(e);
        }
    }
    Ok(())
}
