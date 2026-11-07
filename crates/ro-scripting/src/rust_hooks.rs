use crate::builtin_hooks::{al_heal, item_heal, percent_heal};
use crate::{EffectList, HookBackend, HookCtx, HookError, HookId, Roll};

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
