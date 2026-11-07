mod handle;
mod map_data;
mod map_data_dto;
mod messages_dto;
mod options;
mod world_actor;

pub use handle::{is_valid_map_name, MapServiceHandle, MAX_HOSTED_WORLDS, MAX_PENDING_TRANSFERS};
pub use map_data::{
    load_battle_config, load_mob_spawns, load_npcs, load_registries, load_walkability,
    map_data_loader, FALLBACK_MAP_SIZE,
};
pub use map_data_dto::{MapData, MapDataLoader, RegistryPaths};
pub use messages_dto::{Entry, JoinSession, MapHandoff, SessionControl, WorldCommand};
pub use options::{MapServiceOptions, AUTOSAVE_INTERVAL};
pub use world_actor::TICK_INTERVAL;

#[cfg(test)]
mod tests;
