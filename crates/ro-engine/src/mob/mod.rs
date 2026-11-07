//! Permanent mob spawns: spawn groups, placement and creation (spec
//! `specs/mob-spawning/spec.md` ENG-MOB-01..03; catalog `mechanics/mobs.md` §1.1, §1.3, §1.4).
//!
//! Placement follows rAthena's defaults: `randomize_center_cell: yes`, `force_random_spawn: no`,
//! `no_spawn_on_player: 0`. HP, level, speed and the boss type come from mob_db (spec
//! `specs/combat/spec.md` §2.1); a mob id missing from it spawns with rAthena's struct defaults.

mod mob_stats;
mod placement;
mod spawn_group;
mod systems;
mod view;

pub(crate) use mob_stats::MobStats;
pub(crate) use spawn_group::SpawnGroup;
pub use view::MobView;

#[cfg(test)]
mod tests;
