//! Game rules for skills and items (see `DESIGN.md` §1-§5): the effect vocabulary, the skill and
//! item-use definitions, and the hook backends behind one trait. Definitions and hooks return
//! effects; `ro-engine` checks, consumes costs, applies the effects and sends the packets.
//! NPC scripts come later (DESIGN §6-§7).

mod context;
mod definition;
mod effect;
mod hook;
#[cfg(feature = "lua")]
mod lua;

pub use context::{CastSource, HookCtx, Roll, SkillLevels, UnitView};
pub use definition::{Definitions, ItemUse, ItemUseDef, MissingHook, SkillDef, TargetRule};
pub use effect::{Effect, EffectList, EffectSpec, HealShow, LevelValue, Who};
pub use hook::{HookBackend, HookError, HookId, RustHooks};
#[cfg(feature = "lua")]
pub use lua::{CtxMode, LuaHooks, LuaLoadError};
