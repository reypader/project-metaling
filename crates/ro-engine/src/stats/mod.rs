//! The stat model in the engine (spec `specs/stats/spec.md` §2.1, §2.3; DESIGN.md §5, §9):
//! mapping a player into `ro_mechanics` inputs, the dirty set, the Stage 3 recalc pass and the
//! Stage 4 follow-ups.
//!
//! Flows never recalculate. They mark the player dirty (`mark_dirty`) and queue what rAthena
//! sends after its recalc (`queue_followup`). At the end of the tick each listed player gets, in
//! order: `SP_MAXWEIGHT` if it changed, the HP/SP/AP clamp pushes, the diff, then its
//! follow-ups.

use ro_core::ServerEvent;

use crate::progression::LivePush;

mod login_push;
mod raise;
mod recalc;
mod status_push;
mod systems;

pub(crate) use recalc::weapon_class;
pub(crate) use status_push::{
    allocated_stats, battle_stats, battle_traits, couple, exp, param, status_diff, trait_costs,
    Persisted,
};

/// `PlayerInfo::stat_flags`: a recalc is due.
pub(crate) const DIRTY: u8 = 0x1;

/// The cached `GearStatus` must be rebuilt first (an equip change).
pub(crate) const STALE_GEAR: u8 = 0x2;

/// The player is in `MapInstance::stat_queue`.
const LISTED: u8 = 0x4;

/// The visible skill tree must be re-checked (skills spec §2.6): a changed set sends its
/// deletes and list after `SP_MAXWEIGHT`, before the clamps and the diff.
pub(crate) const SKILLS: u8 = 0x8;

/// What a flow queues for after the tick's recalc (spec §2.1 "Follow-up queue").
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FollowUp {
    /// A ready packet for the player.
    ToSelf(ServerEvent),
    /// A packet for the player's area, the player included, resolved when it is sent.
    ToArea(ServerEvent),
    /// `0x0141` for allocated stat `index` (0..6 STR..LUK, 6..12 POW..CRT) from the snapshot
    /// this tick's recalc committed: a stat above 255 after its stat-up or trait-up ack (which
    /// carries only 255).
    CoupleStat(u16),
    /// Every equip look to the area (`@job`, rAthena `pc_equiplookall`), each slot's view read
    /// when it is sent, so an equip later in the same tick isn't undone on screen.
    EquipLooks,
    /// Full HP and SP to the new maxima after a base level up: `SP_HP`/`SP_SP` for the parts
    /// it raised; nothing for a dead player.
    HealToMax,
    /// The `elvmax` re-check after a base level up (rAthena `pc_baselevelchanged`, the last
    /// step of `@blvl`): worn items above their max level come off.
    LevelGearCheck,
    /// Save the character now (`@job`; rAthena `chrif_save`).
    Save,
    /// Pushes whose values are read when sent: queued tails and exp (combat spec §2.9).
    Live(LivePush),
}

#[cfg(test)]
mod tests;
