//! Named hooks behind one backend trait, and the Rust backend (spec §2.1, §2.5, §2.7).
//!
//! Rolls, in order (the same seed gives the same effects from any backend):
//! - `al_heal`: one MATK roll, only when the weapon gives a MATK range;
//! - `item_heal`: the HP roll, then the SP roll, each only when its range has more than one
//!   value (rAthena's script `rand` isn't called for a constant);
//! - `percent_heal`: none.

use std::fmt;

use crate::{CastSource, Effect, EffectList, HealShow, HookCtx, Roll, Who};

/// A hook's index in its backend, looked up once by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HookId(pub u16);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookError {
    /// The backend has no hook with this id.
    UnknownHook(HookId),
    /// The call's source isn't implemented yet.
    Unsupported(CastSource),
    /// The hook rejected its input.
    Failed(String),
}

impl fmt::Display for HookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownHook(id) => write!(f, "no hook with id {}", id.0),
            Self::Unsupported(source) => write!(f, "cast source {source:?} is not supported"),
            Self::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for HookError {}

/// A set of hooks (Rust now, Lua later). A hook reads the context and appends effects; it never
/// changes the world.
pub trait HookBackend {
    /// The id of the hook called `name`.
    fn hook_id(&self, name: &str) -> Option<HookId>;

    fn call(
        &self,
        hook: HookId,
        ctx: &HookCtx<'_>,
        roll: &mut dyn Roll,
        out: &mut EffectList,
    ) -> Result<(), HookError>;
}

type HookFn = fn(&HookCtx<'_>, &mut dyn Roll, &mut EffectList) -> Result<(), HookError>;

/// The built-in hooks as plain Rust functions.
pub struct RustHooks {
    hooks: Vec<(&'static str, HookFn)>,
}

impl RustHooks {
    /// `al_heal`, `item_heal` and `percent_heal`.
    pub fn builtin() -> Self {
        Self {
            hooks: vec![
                ("al_heal", al_heal),
                ("item_heal", item_heal),
                ("percent_heal", percent_heal),
            ],
        }
    }
}

impl HookBackend for RustHooks {
    fn hook_id(&self, name: &str) -> Option<HookId> {
        let index = self.hooks.iter().position(|(n, _)| *n == name)?;
        u16::try_from(index).ok().map(HookId)
    }

    fn call(
        &self,
        hook: HookId,
        ctx: &HookCtx<'_>,
        roll: &mut dyn Roll,
        out: &mut EffectList,
    ) -> Result<(), HookError> {
        let (_, f) = self
            .hooks
            .get(usize::from(hook.0))
            .ok_or(HookError::UnknownHook(hook))?;
        f(ctx, roll, out)
    }
}

// Skill ids the potion bonus reads.
const SM_RECOVERY: u16 = 4;
const MG_SRECOVERY: u16 = 9;
const AM_LEARNINGPOTION: u16 = 227;

/// Renewal Heal (catalog `scripting/skills.md` §7): `(BaseLv + INT) / 5 × 30 × lv / 10`, then
/// the MATK term (status MATK plus a roll in the weapon's MATK range), at least 1. Meditatio
/// and the other heal bonuses are not applied yet.
fn al_heal(ctx: &HookCtx<'_>, roll: &mut dyn Roll, out: &mut EffectList) -> Result<(), HookError> {
    let caster = &ctx.caster;
    let mut hp =
        (i64::from(caster.base_level) + i64::from(caster.int)) / 5 * 30 * i64::from(ctx.level) / 10;
    let weapon = i64::from(caster.weapon_matk);
    let variance = weapon * i64::from(caster.weapon_level) / 10;
    let min = i64::from(caster.matk_base) + weapon - variance;
    let max = i64::from(caster.matk_base) + weapon + variance;
    hp += min;
    if max > min {
        hp += i64::from(roll.roll(u32::try_from(max - min).unwrap_or(u32::MAX)));
    }
    out.push(Effect::Heal {
        to: Who::Target,
        hp: u32::try_from(hp.max(1)).unwrap_or(u32::MAX),
        sp: 0,
        show: HealShow::Skill,
    });
    Ok(())
}

/// `itemheal` (catalog `mechanics/inventory.md` §7.4). `args` = `[hp_min, hp_max, sp_min,
/// sp_max]`, inclusive. Each side is scaled by `100 + 2·VIT (INT) + 10·HP (SP) Recovery +
/// 5·Learning Potion` percent when that is not 100 and raises the value.
fn item_heal(
    ctx: &HookCtx<'_>,
    roll: &mut dyn Roll,
    out: &mut EffectList,
) -> Result<(), HookError> {
    let [hp_min, hp_max, sp_min, sp_max] = ctx.args;
    let hp = rand(hp_min, hp_max, roll)?;
    let sp = rand(sp_min, sp_max, roll)?;
    let learning = 5 * i64::from(ctx.skills.level(AM_LEARNINGPOTION));
    let hp_bonus =
        100 + 2 * i64::from(ctx.caster.vit) + 10 * i64::from(ctx.skills.level(SM_RECOVERY));
    let sp_bonus =
        100 + 2 * i64::from(ctx.caster.int) + 10 * i64::from(ctx.skills.level(MG_SRECOVERY));
    out.push(Effect::Heal {
        to: Who::Caster,
        hp: scaled(hp, hp_bonus + learning),
        sp: scaled(sp, sp_bonus + learning),
        show: HealShow::Silent,
    });
    Ok(())
}

/// `percentheal` (catalog `mechanics/inventory.md` §7.4). `args` = `[hp_rate, sp_rate, 0, 0]`,
/// in percent of max.
fn percent_heal(
    ctx: &HookCtx<'_>,
    _roll: &mut dyn Roll,
    out: &mut EffectList,
) -> Result<(), HookError> {
    let [hp_rate, sp_rate, ..] = ctx.args;
    if hp_rate < 0 || sp_rate < 0 {
        return Err(HookError::Failed(format!(
            "percent_heal: negative rate {hp_rate},{sp_rate}"
        )));
    }
    out.push(Effect::Heal {
        to: Who::Caster,
        hp: percent_of(ctx.caster.max_hp, hp_rate),
        sp: percent_of(ctx.caster.max_sp, sp_rate),
        show: HealShow::Silent,
    });
    Ok(())
}

/// Script `rand(min, max)`, inclusive; a constant draws nothing.
fn rand(min: i32, max: i32, roll: &mut dyn Roll) -> Result<i64, HookError> {
    if min < 0 || max < min {
        return Err(HookError::Failed(format!(
            "item_heal: bad range {min}..={max}"
        )));
    }
    let span = u32::try_from(max - min).unwrap_or(u32::MAX);
    Ok(if span == 0 {
        i64::from(min)
    } else {
        i64::from(min) + i64::from(roll.roll(span.saturating_add(1)))
    })
}

/// `value × bonus / 100`, kept only when the bonus isn't 100 and the result is higher.
fn scaled(value: i64, bonus: i64) -> u32 {
    let boosted = value * bonus / 100;
    let value = if bonus != 100 && boosted > value {
        boosted
    } else {
        value
    };
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// rAthena `apply_rate` for a heal: `max × rate / 100` (`max / 100 × rate` above 100000), the
/// full max at 100 % or more, and at least 1 for a non-zero rate.
fn percent_of(max: u32, rate: i32) -> u32 {
    if rate <= 0 {
        return 0;
    }
    if rate >= 100 {
        return max;
    }
    let (max, rate) = (u64::from(max), u64::from(rate.unsigned_abs()));
    let value = if max > 100_000 {
        max / 100 * rate
    } else {
        max * rate / 100
    };
    u32::try_from(value.max(1)).unwrap_or(u32::MAX)
}
