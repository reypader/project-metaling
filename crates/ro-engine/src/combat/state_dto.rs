//! Combat state carried on entities and timers: attack state, step skills, pending casts and hits, timer kinds.

use ro_core::CharId;

use crate::ecs::Entity;

/// One attacker's total damage on a mob, overkill included (rAthena `s_dmglog`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DamageEntry {
    pub(crate) char_id: CharId,
    pub(crate) damage: u64,
}

/// rAthena's per-unit attack state (`ud.target`, `attack_continue`, `step_attack`,
/// `attackabletime`, `canact_tick`, `target_to`). Kept after an attack stops.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AttackState {
    pub(crate) target: Option<Entity>,
    /// Action 7: re-arm after each swing.
    pub(crate) continuous: bool,
    /// The attack came from a step action: range + 1.
    pub(crate) step_attack: bool,
    /// The next swing may start here (ms).
    pub(crate) attackable_at: u64,
    /// A fresh request (attack or skill) is accepted from here (ms).
    pub(crate) canact_at: u64,
    /// The pending step action's target (rAthena `target_to`).
    pub(crate) step_target: Option<Entity>,
    /// The pending step action is this skill cast instead of an attack (rAthena
    /// `stepskill_id`/`stepskill_lv`).
    pub(crate) step_skill: Option<StepSkill>,
    /// Bumped when the loop stops or restarts; an armed swing with an older value is stale.
    pub(crate) swing_gen: u32,
    /// Bumped when a step action is armed or replaced.
    pub(crate) step_gen: u32,
    /// The cast between cast begin and castend (rAthena `ud.skill_id`/`skilltarget`).
    pub(crate) cast: Option<PendingCast>,
    /// Bumped by every cast begin; a `CastEnd` timer with an older value is stale.
    pub(crate) cast_gen: u32,
}

/// A skill request stored as the step action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StepSkill {
    pub(crate) skill: u16,
    pub(crate) level: u16,
}

/// A begun cast waiting for its castend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PendingCast {
    pub(crate) skill: u16,
    pub(crate) level: u8,
    pub(crate) target: Entity,
}

/// A hit swung at `target`, applied at swing + amotion (spec §2.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PendingHit {
    pub(crate) attacker: Entity,
    pub(crate) target: Entity,
    pub(crate) damage: i32,
    /// The Chebyshev distance at swing time plus `AREA_SIZE`: farther at apply time drops it.
    pub(crate) max_distance: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CombatTimer {
    Swing {
        attacker: Entity,
        generation: u32,
    },
    StepAction {
        attacker: Entity,
        generation: u32,
    },
    Hit(PendingHit),
    /// The end of a cast begun with this generation (spec `specs/effects/spec.md` §2.3).
    CastEnd {
        caster: Entity,
        generation: u32,
    },
    /// A dead mob's `0x0080` type 1 around its death cell, then its despawn.
    Vanish {
        mob: Entity,
        cell: (u16, u16),
    },
}
