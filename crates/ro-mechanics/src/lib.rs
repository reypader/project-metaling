//! Pure stat formulas (renewal): the player status calculation, stat-point costs and the
//! battle config knobs they read. No I/O, no ECS state, no `ro-db` types: `ro-engine` maps its
//! data into the input views here. Formulas follow `reference/catalog/mechanics/stats.md`
//! (clean-room), checked against `reference/catalog/captures/attribute_sequence.txt`.

mod bonus;
mod class;
mod config;
mod points;
mod status;

pub use bonus::{BonusSlot, BonusTotals, BONUS_SLOT_COUNT};
pub use class::ClassFlags;
pub use config::BattleConfig;
pub use points::{next_stat_cost, raise_stat, stat_cost, StatRaise};
pub use status::{
    calc_status, GearInput, HandInput, JobRow, Primary, RefineInput, Status, StatusInputs,
    WeaponClass,
};
