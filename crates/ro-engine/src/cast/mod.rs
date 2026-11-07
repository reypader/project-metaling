//! Casting a skill on a unit and applying its effects (spec `specs/effects/spec.md` §2.2-§2.6;
//! catalog `scripting/skills.md` §3-§10, §14-§15).
//!
//! A request (`0x0438`) runs the silent checks, the target and range checks (or stores the step
//! action), then the cast-begin requirements. Cast begin stops the attack loop (and a walk, with
//! `0x0088`), sends `0x0b1a` and sets the gates. Castend re-checks the target, resolves the
//! definition through `ro-scripting` and applies the effects in order: the costs, `0x0983` for a
//! skill with an after-cast delay, then heals and weapon hits. With no cast time (every skill
//! in the slice) castend runs in the same tick.

mod adapters;
mod cast_consts;
mod skill_scripts;
mod systems;

pub(crate) use adapters::MapRoll;
pub use skill_scripts::SkillScripts;
