//! Static game data: map walkability grids (`.gat`), per-map NPC and mob spawn lists (YAML), and
//! the item, mob, job, refine and skill registries (YAML). Read-only at runtime.

mod cell_type;
mod db_load_error;
mod element;
mod item_data;
mod item_db;
mod item_type_dto;
mod job;
mod job_db;
mod job_dto;
mod map;
mod map_data_error;
mod map_mob_spawns;
mod map_npcs;
mod mob_data;
mod mob_db;
mod mob_dto;
mod mob_mode;
mod mob_modes;
mod mob_spawn;
mod npc_dto;
mod refine_db;
mod refine_dto;
mod skill_data;
mod skill_db;
mod skill_element;
mod skill_flags_dto;
mod skill_tree_db;
mod skill_tree_dto;
mod yaml;

pub use cell_type::CellType;
pub use db_load_error::DbLoadError;
pub use element::{Element, MAX_ELEMENT_LEVEL};
pub use item_data::{ItemData, ALL_CLASSES, ALL_JOBS};
pub use item_db::{ItemDb, ITEM_DATA_FILES, ITEM_DATA_TYPE, ITEM_DATA_VERSION};
pub use item_type_dto::{AmmoType, Gender, ItemType, ItemUse, WeaponType};
pub use job::{Job, DEFAULT_ASPD_BASE};
pub use job_db::{
    JobDb, JOB_DATA_FILE, JOB_DATA_TYPE, JOB_DATA_VERSION, JOB_EXP_DATA_FILE, JOB_EXP_DATA_TYPE,
    JOB_EXP_DATA_VERSION, NEXT_BASE_EXP_AT_MAX, NEXT_JOB_EXP_AT_MAX, STATPOINT_DATA_FILE,
    STATPOINT_DATA_TYPE, STATPOINT_DATA_VERSION,
};
pub use job_dto::{ExpGroup, ExpKind, JobBonus, JobBonusEntry, JobData, StatPointRow};
pub use map::MapGrid;
pub use map_data_error::MapDataError;
pub use map_mob_spawns::{MapMobSpawns, MOB_SPAWN_DB_TYPE, MOB_SPAWN_DB_VERSION};
pub use map_npcs::{MapNpcs, NPC_SPAWN_DB_TYPE, NPC_SPAWN_DB_VERSION};
pub use mob_data::{MobData, DEFAULT_MOB_MOTION, DEFAULT_MOB_WALK_SPEED};
pub use mob_db::{
    MobDb, MAX_DROP_RATE, MAX_MOB_DROPS, MAX_MVP_DROPS, MOB_DATA_FILE, MOB_DATA_TYPE,
    MOB_DATA_VERSION,
};
pub use mob_dto::{MobClass, MobDrop, MobId, MobRace, MobSize};
pub use mob_mode::MobMode;
pub use mob_modes::MobModes;
pub use mob_spawn::MobSpawn;
pub use npc_dto::{NpcKind, NpcSpawn, NpcState, WarpDest};
pub use refine_db::{
    RefineDb, MAX_REFINE, REFINE_DATA_FILE, REFINE_DATA_TYPE, REFINE_DATA_VERSION,
};
pub use refine_dto::{RefineBonus, RefineGroup, RefineRow};
pub use skill_data::SkillData;
pub use skill_db::{
    SkillDb, MAX_SKILL_LEVEL, SKILL_DATA_FILE, SKILL_DATA_TYPE, SKILL_DATA_VERSION,
};
pub use skill_element::SkillElement;
pub use skill_flags_dto::{CastTimeFlags, DamageFlags, SkillFlags, SkillItemCost};
pub use skill_tree_db::{
    SkillTreeDb, SKILL_TREE_DATA_FILE, SKILL_TREE_DATA_TYPE, SKILL_TREE_DATA_VERSION,
};
pub use skill_tree_dto::{JobTree, SkillRequirement, TreeEntry};
