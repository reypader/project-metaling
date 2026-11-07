//! A read-only view of one mob, for the map-server.

use ro_core::MapPosition;
use ro_db::{MobId, MobSpawn};

/// A read-only view of one spawned mob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobView<'a> {
    pub gid: u32,
    pub mob_id: MobId,
    pub name: &'a str,
    pub level: u16,
    pub hp: u32,
    pub max_hp: u32,
    pub pos: MapPosition,
    pub speed: u16,
    /// In the spatial grid (seen by players in range).
    pub visible: bool,
    /// The spawn line the mob came from.
    pub spawn: &'a MobSpawn,
}
