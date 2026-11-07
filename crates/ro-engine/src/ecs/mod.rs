//! ECS building blocks: entity handles, component storage, the spatial grid and the RNG.

mod entity;
mod entity_allocator;
mod rng;
mod sparse_set;
mod spatial;

pub(crate) use entity::Entity;
pub(crate) use entity_allocator::EntityAllocator;
pub(crate) use rng::SplitMix64;
pub(crate) use sparse_set::SparseSet;
pub(crate) use spatial::{step_visibility, SpatialGrid8x8, AREA_SIZE};
