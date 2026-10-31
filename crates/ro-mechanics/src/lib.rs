//! Pure stat formulas (renewal): the player and mob status calculations, normal-attack damage,
//! the client attack range, the kill exp share, stat-point costs and the battle config knobs
//! they read. No I/O, no ECS state, no `ro-db` types: `ro-engine` maps its
//! data into the input views here. Formulas follow `reference/catalog/mechanics/stats.md`
//! (clean-room), checked against `reference/catalog/captures/attribute_sequence.txt`.

mod bonus;
mod class;
mod combat;
mod config;
mod exp;
mod mob;
mod points;
mod status;

pub use bonus::{BonusSlot, BonusTotals, BONUS_SLOT_COUNT};
pub use class::ClassFlags;
pub use combat::{
    calc_weapon_attack, check_distance_client, client_distance, AttackerView, AttrRates,
    DamageResult, DefenderView, HitKind, WeaponAttack,
};
pub use config::BattleConfig;
pub use exp::{exp_share, shared_exp};
pub use mob::{calc_mob_status, MobStatusInputs};
pub use points::{next_stat_cost, next_trait_cost, raise_stat, raise_trait, stat_cost, StatRaise};
pub use status::{
    calc_status, GearInput, HandInput, JobRow, Primary, RefineInput, Status, StatusInputs, Traits,
    WeaponClass,
};
