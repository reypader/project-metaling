//! Skill and item-use definitions: effect specs plus an optional hook (DESIGN §2). Costs, cast
//! and delay times, range and target type are not here: they come from the imported data, and
//! the engine puts the `Spend*` effects in front of what a definition resolves to.

use std::fmt;

use crate::{
    CastSource, EffectList, EffectSpec, HealShow, HookBackend, HookCtx, HookError, HookId,
    LevelValue, Roll, Who,
};

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

/// A definition names a hook its backend doesn't have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingHook(pub &'static str);

impl fmt::Display for MissingHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the hook backend has no hook named {}", self.0)
    }
}

impl std::error::Error for MissingHook {}

/// Every skill and item-use definition, with hook names bound to one backend's ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definitions {
    /// Sorted by id.
    skills: Vec<SkillDef>,
    item_heal: ItemUseDef,
    percent_heal: ItemUseDef,
}

// Skill ids of the built-in definitions.
const SM_BASH: u16 = 5;
const AL_HEAL: u16 = 28;
const MC_MAMMONITE: u16 = 42;
const NV_FIRSTAID: u16 = 142;

impl Definitions {
    /// The built-in definitions (spec §2.5, §2.7), bound to `hooks`:
    /// - First Aid: heal the caster 5 HP;
    /// - Bash: weapon damage at ratio `100 + 30·lv`, hit `+5·lv` %;
    /// - Mammonite: weapon damage at ratio `100 + 50·lv` (its zeny cost is data);
    /// - Heal: the `al_heal` hook; Undead mobs refused;
    /// - potions: the `item_heal` and `percent_heal` hooks.
    pub fn builtin(hooks: &dyn HookBackend) -> Result<Self, MissingHook> {
        let hook = |name: &'static str| hooks.hook_id(name).ok_or(MissingHook(name));
        let weapon = |ratio, hit_pct| EffectSpec::WeaponDamage {
            ratio,
            hit_pct,
            div: LevelValue::fixed(1),
        };
        let mut skills = vec![
            SkillDef {
                id: NV_FIRSTAID,
                effects: vec![EffectSpec::Heal {
                    to: Who::Caster,
                    hp: LevelValue::fixed(5),
                    sp: LevelValue::fixed(0),
                    show: HealShow::Skill,
                }],
                hook: None,
                target: TargetRule::default(),
            },
            SkillDef {
                id: SM_BASH,
                effects: vec![weapon(LevelValue::new(100, 30), LevelValue::new(0, 5))],
                hook: None,
                target: TargetRule::default(),
            },
            SkillDef {
                id: MC_MAMMONITE,
                effects: vec![weapon(LevelValue::new(100, 50), LevelValue::fixed(0))],
                hook: None,
                target: TargetRule::default(),
            },
            SkillDef {
                id: AL_HEAL,
                effects: Vec::new(),
                hook: Some(hook("al_heal")?),
                target: TargetRule {
                    reject_undead: true,
                },
            },
        ];
        skills.sort_by_key(|def| def.id);
        let item = |kind, name| -> Result<ItemUseDef, MissingHook> {
            Ok(ItemUseDef {
                kind,
                effects: Vec::new(),
                hook: Some(hook(name)?),
            })
        };
        Ok(Self {
            skills,
            item_heal: item(ItemUse::ItemHeal, "item_heal")?,
            percent_heal: item(ItemUse::PercentHeal, "percent_heal")?,
        })
    }

    /// The definition of skill `id`; `None` for a skill that can't be cast yet.
    pub fn skill(&self, id: u16) -> Option<&SkillDef> {
        self.skills
            .binary_search_by_key(&id, |def| def.id)
            .ok()
            .map(|index| &self.skills[index])
    }

    pub fn item_use(&self, kind: ItemUse) -> &ItemUseDef {
        match kind {
            ItemUse::ItemHeal => &self.item_heal,
            ItemUse::PercentHeal => &self.percent_heal,
        }
    }

    /// Skill definitions in id order.
    pub fn skills(&self) -> impl Iterator<Item = &SkillDef> {
        self.skills.iter()
    }
}
