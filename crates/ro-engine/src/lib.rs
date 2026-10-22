//! Map simulation runtime: a single-threaded ECS map instance (see `DESIGN.md` §1a).

mod engine;
pub(crate) mod entity;
pub(crate) mod movement;
pub(crate) mod path;
pub(crate) mod sparse_set;
pub(crate) mod spatial;
pub(crate) mod world;

pub use engine::{MapInstance, DEFAULT_ENTITY_CAPACITY};
pub use ro_contracts;
