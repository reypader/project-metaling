//! Pure stat formulas (renewal): the player and mob status calculations, weapon-attack damage,
//! cast time and after-cast delay, the client attack range, the kill exp share, stat-point costs
//! and the battle config knobs they read. No I/O, no ECS state, no `ro-db` types: `ro-engine`
//! maps its data into the input views here. Formulas follow `reference/catalog/mechanics/stats.md`
//! (clean-room), checked against `reference/catalog/captures/attribute_sequence.txt`.

mod bonus_slot;
mod bonus_totals;
mod cast;
mod cast_dto;
mod class;
mod combat;
mod combat_dto;
mod config;
mod distance;
mod exp;
mod mob;
mod points;
mod status;
mod status_dto;
mod status_input_dto;

pub use bonus_slot::{BonusSlot, BONUS_SLOT_COUNT};
pub use bonus_totals::BonusTotals;
pub use cast::{cast_time, delayfix};
pub use cast_dto::{CastTimeData, Caster};
pub use class::ClassFlags;
pub use combat::calc_weapon_attack;
pub use combat_dto::{
    AttackContext, AttackerView, AttrRates, DamageResult, DefenderView, HitKind, WeaponAttack,
};
pub use config::BattleConfig;
pub use distance::{check_distance_client, client_distance};
pub use exp::{exp_share, shared_exp};
pub use mob::{calc_mob_status, MobStatusInputs};
pub use points::{next_stat_cost, next_trait_cost, raise_stat, raise_trait, stat_cost, StatRaise};
pub use status::calc_status;
pub use status_dto::{Primary, Status, Traits};
pub use status_input_dto::{GearInput, HandInput, JobRow, RefineInput, StatusInputs, WeaponClass};
