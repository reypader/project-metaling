//! Player auto-attack: the request, the swing loop, ammo and delayed hits (spec
//! `specs/combat/spec.md` §2.2-§2.7; catalog `mechanics/auto_attack.md`).
//!
//! One per-map min-heap holds every combat timer (swings, step actions, delayed hits, death
//! vanishes). It is
//! popped in `step_tick` after the walkers and before the intents. Swing times come from the
//! timer's due time, never the tick's `now`, so a 20 ms tick never stretches the interval.
//! Stale timers are skipped: a swing or step action carries the generation it was armed with,
//! and a hit re-checks both units when it fires.

mod damage_log;
mod rules;
mod state_dto;
mod systems;
mod timers;

pub(crate) use damage_log::DamageLog;
pub(crate) use rules::attacker_view;
pub(crate) use state_dto::{AttackState, CombatTimer, PendingCast, PendingHit, StepSkill};
pub(crate) use timers::CombatTimers;
