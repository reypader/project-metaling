//! Static game data: map walkability grids (`.gat`), per-map NPC and mob spawn lists (YAML), and
//! the item, mob, job and refine registries (YAML). Read-only at runtime.

mod cell;
mod item;
mod job;
mod map;
mod mob;
mod mob_spawn;
mod npc;
mod refine;
mod yaml;

pub use cell::{CellType, DbLoadError, MapDataError};
pub use item::{
    AmmoType, Gender, ItemData, ItemDb, ItemType, WeaponType, ALL_CLASSES, ALL_JOBS,
    ITEM_DATA_FILES, ITEM_DATA_TYPE, ITEM_DATA_VERSION,
};
pub use job::{
    Job, JobBonus, JobBonusEntry, JobData, JobDb, StatPointRow, DEFAULT_ASPD_BASE, JOB_DATA_FILE,
    JOB_DATA_TYPE, JOB_DATA_VERSION, STATPOINT_DATA_FILE, STATPOINT_DATA_TYPE,
    STATPOINT_DATA_VERSION,
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
pub use refine::{
    RefineBonus, RefineDb, RefineGroup, RefineRow, MAX_REFINE, REFINE_DATA_FILE, REFINE_DATA_TYPE,
    REFINE_DATA_VERSION,
};
