//! Map simulation runtime: a single-threaded ECS map instance (see `DESIGN.md` §1a).

mod engine;
pub(crate) mod entity;
mod error;
mod mob;
pub(crate) mod movement;
pub(crate) mod path;
mod rng;
pub(crate) mod sparse_set;
pub(crate) mod spatial;
mod warp;
pub(crate) mod world;

pub use engine::{MapInstance, DEFAULT_ENTITY_CAPACITY, START_NPC_ID};
pub use error::EngineError;
pub use mob::MobView;
pub use warp::MapTransfer;
