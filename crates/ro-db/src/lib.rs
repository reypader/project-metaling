//! Static game data: map walkability grids (`.gat`), per-map NPC spawn lists (YAML), and later
//! YAML registries. Read-only at runtime.

mod cell;
mod map;
mod npc;

pub use cell::{CellType, DbLoadError, MapDataError};
pub use map::MapGrid;
pub use npc::{
    MapNpcs, NpcKind, NpcSpawn, NpcState, WarpDest, NPC_SPAWN_DB_TYPE, NPC_SPAWN_DB_VERSION,
};
