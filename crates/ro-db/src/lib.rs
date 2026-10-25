//! Static game data: map walkability grids (`.gat`), per-map NPC and mob spawn lists (YAML), and
//! the item and mob registries (YAML). Read-only at runtime.

mod cell;
mod item;
mod map;
mod mob;
mod mob_spawn;
mod npc;
mod yaml;

pub use cell::{CellType, DbLoadError, MapDataError};
pub use item::{
    AmmoType, Gender, ItemData, ItemDb, ItemType, WeaponType, ALL_CLASSES, ALL_JOBS,
    ITEM_DATA_FILES, ITEM_DATA_TYPE, ITEM_DATA_VERSION,
};
pub use map::MapGrid;
pub use mob::{
    MobClass, MobData, MobDb, MobDrop, MAX_DROP_RATE, MAX_MOB_DROPS, MAX_MVP_DROPS, MOB_DATA_FILE,
    MOB_DATA_TYPE, MOB_DATA_VERSION,
};
pub use mob_spawn::{MapMobSpawns, MobId, MobSpawn, MOB_SPAWN_DB_TYPE, MOB_SPAWN_DB_VERSION};
pub use npc::{
    MapNpcs, NpcKind, NpcSpawn, NpcState, WarpDest, NPC_SPAWN_DB_TYPE, NPC_SPAWN_DB_VERSION,
};
