//! Map simulation runtime: a single-threaded ECS map instance (see `DESIGN.md` §1a).

mod cast;
mod combat;
mod command;
mod drops;
mod engine;
pub(crate) mod entity;
mod equipment;
mod error;
mod experience;
mod floor_item;
mod inventory;
mod item_use;
mod job;
mod level_command;
mod mob;
pub(crate) mod movement;
pub(crate) mod path;
mod rng;
mod settings;
mod skills;
pub(crate) mod sparse_set;
pub(crate) mod spatial;
mod stat_up;
mod stats;
mod status;
mod vitals;
mod warp;
mod wear;
pub(crate) mod world;

pub use cast::SkillScripts;
pub use drops::{roll_mob_drops, MobDropRequest};
pub use engine::{MapInstance, Registries, DEFAULT_ENTITY_CAPACITY, START_NPC_ID};
pub use error::EngineError;
pub use mob::MobView;
pub use warp::{MapTransfer, PlaySnapshot};
