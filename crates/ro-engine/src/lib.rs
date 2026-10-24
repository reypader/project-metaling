//! Map simulation runtime: a single-threaded ECS map instance (see `DESIGN.md` §1a).

mod command;
mod drops;
mod engine;
pub(crate) mod entity;
mod error;
mod floor_item;
mod inventory;
mod mob;
pub(crate) mod movement;
pub(crate) mod path;
mod rng;
pub(crate) mod sparse_set;
pub(crate) mod spatial;
mod warp;
pub(crate) mod world;

pub use drops::{roll_mob_drops, MobDropRequest};
pub use engine::{MapInstance, DEFAULT_ENTITY_CAPACITY, START_NPC_ID};
pub use error::EngineError;
pub use mob::MobView;
pub use warp::MapTransfer;
