//! Static game data: map walkability grids (`.gat`), and later YAML registries and spawns.
//! Read-only at runtime.

pub mod map;

pub use map::{GatMapGrid, MemoryMapGrid};
pub use ro_core;
