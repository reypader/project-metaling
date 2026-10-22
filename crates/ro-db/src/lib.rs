//! Static game data: map walkability grids (`.gat`), and later YAML registries and spawns.
//! Read-only at runtime.

pub mod cell;
pub mod map;

pub use cell::{CellType, DbLoadError, MapDataError, MapWalkability};
pub use map::{GatMapGrid, MemoryMapGrid};
pub use ro_core;
