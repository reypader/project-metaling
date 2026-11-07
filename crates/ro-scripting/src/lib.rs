//! Game rules for skills and items (see `DESIGN.md` §1-§5): the effect vocabulary, the skill and
//! item-use definitions, and the hook backends behind one trait. Definitions and hooks return
//! effects; `ro-engine` checks, consumes costs, applies the effects and sends the packets.
//! NPC scripts come later (DESIGN §6-§7).

mod builtin_hooks;
mod context_dto;
mod definition_dto;
mod definitions;
mod effect_dto;
mod effect_list;
mod effect_spec;
mod hook_backend;
mod hook_error;
mod hook_id;
mod level_value;
#[cfg(feature = "lua")]
mod lua;
mod missing_hook;
mod roll;
mod rust_hooks;
mod skill_levels;

pub use context_dto::{CastSource, HookCtx, UnitView};
pub use definition_dto::{ItemUse, ItemUseDef, SkillDef, TargetRule};
pub use definitions::Definitions;
pub use effect_dto::{Effect, HealShow, Who};
pub use effect_list::EffectList;
pub use effect_spec::EffectSpec;
pub use hook_backend::HookBackend;
pub use hook_error::HookError;
pub use hook_id::HookId;
pub use level_value::LevelValue;
#[cfg(feature = "lua")]
pub use lua::{CtxMode, LuaHooks, LuaLoadError};
pub use missing_hook::MissingHook;
pub use roll::Roll;
pub use rust_hooks::RustHooks;
pub use skill_levels::SkillLevels;
