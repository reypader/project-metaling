use crate::{EffectList, HookCtx, HookError, HookId, Roll};

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
