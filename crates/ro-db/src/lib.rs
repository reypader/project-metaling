//! Static game data: map walkability grids (`.gat`), per-map NPC and mob spawn lists (YAML), and
//! later YAML registries. Read-only at runtime.

mod cell;
mod map;
mod mob_spawn;
mod npc;
mod yaml;

pub use cell::{CellType, DbLoadError, MapDataError};
pub use map::MapGrid;
pub use mob_spawn::{MapMobSpawns, MobId, MobSpawn, MOB_SPAWN_DB_TYPE, MOB_SPAWN_DB_VERSION};
pub use npc::{
    MapNpcs, NpcKind, NpcSpawn, NpcState, WarpDest, NPC_SPAWN_DB_TYPE, NPC_SPAWN_DB_VERSION,
};
