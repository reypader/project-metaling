//! Mob systems: spawning from the map's spawn lines, and the mob list for the map-server.

use std::sync::Arc;

use ro_core::MapPosition;
use ro_db::{MapMobSpawns, MobData};
use tracing::warn;

use super::MobView;
use crate::combat::DamageLog;
use crate::error::EngineError;
use crate::map_instance::MapInstance;
use crate::mob::{MobStats, SpawnGroup};
use crate::world::MobInfo;

impl MapInstance {
    /// Creates `Amount` mobs per line, in order, and returns how many it created (ENG-MOB-01).
    /// Lines outside the map are skipped; mobs with no free cell are not created; spawning
    /// stops when the entity capacity runs out. Emits no events.
    pub fn spawn_mobs(&mut self, spawns: &MapMobSpawns) -> usize {
        let mut created = 0;
        for line in &spawns.spawns {
            if line.x >= self.walkability.width() || line.y >= self.walkability.height() {
                warn!(
                    map = %self.map_name,
                    mob = %line.name,
                    source = %line.source,
                    "mob spawn line outside the map ({}, {}); skipped",
                    line.x,
                    line.y
                );
                continue;
            }
            let Ok(index) = u32::try_from(self.spawn_groups.len()) else {
                warn!(map = %self.map_name, "too many mob spawn groups; rest skipped");
                break;
            };
            if self.mob_db.get(line.mob_id).is_none() {
                warn!(
                    map = %self.map_name,
                    mob = line.mob_id.0,
                    source = %line.source,
                    "mob id not in mob_db; spawning with default stats"
                );
            }
            self.spawn_groups
                .push(SpawnGroup::new(line, &self.walkability));
            let mut unplaced = 0;
            for _ in 0..line.amount {
                match self.spawn_mob(index) {
                    Ok(true) => created += 1,
                    Ok(false) => unplaced += 1,
                    Err(EngineError::WorldCapacityExceeded) => {
                        warn!(
                            map = %self.map_name,
                            source = %line.source,
                            "entity capacity reached; remaining mobs not spawned"
                        );
                        return created;
                    }
                    Err(err) => warn!(
                        map = %self.map_name,
                        source = %line.source,
                        "mob not spawned: {err}"
                    ),
                }
            }
            if unplaced > 0 {
                warn!(
                    map = %self.map_name,
                    mob = %line.name,
                    source = %line.source,
                    "{unplaced} of {} mobs found no free cell; not spawned",
                    line.amount
                );
            }
        }
        created
    }

    /// Places and creates one mob of a group. `Ok(false)` when no cell was found.
    fn spawn_mob(&mut self, index: u32) -> Result<bool, EngineError> {
        let Some(group) = self.spawn_groups.get(index as usize) else {
            return Ok(false);
        };
        let center = group.mob_center(&mut self.rng, &self.walkability);
        let Some((x, y)) = group.place(&mut self.rng, &self.walkability, center) else {
            return Ok(false);
        };
        // Never reuse a gid that is still taken (only possible after the counter wraps).
        while self.world.entity_by_gid(self.next_npc_id).is_some() {
            self.next_npc_id = self.next_npc_id.wrapping_add(1);
        }
        let fallback;
        let data = match self.mob_db.get(group.line.mob_id) {
            Some(data) => data,
            None => {
                fallback = MobData::default();
                &fallback
            }
        };
        let mob_id = group.line.mob_id;
        let stats = match self.mob_stats.get(&mob_id) {
            Some(stats) => Arc::clone(stats),
            None => {
                let stats = Arc::new(MobStats::new(data, &self.battle));
                self.mob_stats.insert(mob_id, Arc::clone(&stats));
                stats
            }
        };
        let info = MobInfo {
            gid: self.next_npc_id,
            mob_id,
            name: Arc::clone(&group.name),
            level: group.line.level.unwrap_or(data.level),
            hp: data.hp,
            max_hp: data.hp,
            is_boss: data.boss_type(),
            spawn_group: index,
            stats,
            damage_log: DamageLog::default(),
            dying: false,
        };
        let pos = MapPosition { x, y, dir: 0 };
        let e = self.world.spawn_mob(info, pos, data.walk_speed)?;
        self.next_npc_id = self.next_npc_id.wrapping_add(1);
        self.world.show(e);
        Ok(true)
    }

    /// Every mob on the map, in no particular order.
    pub fn mobs(&self) -> impl Iterator<Item = MobView<'_>> {
        let world = &self.world;
        world
            .mobs
            .entities()
            .iter()
            .zip(world.mobs.values())
            .filter_map(move |(&e, m)| {
                let i = e.index();
                Some(MobView {
                    gid: m.gid,
                    mob_id: m.mob_id,
                    name: &m.name,
                    level: m.level,
                    hp: m.hp,
                    max_hp: m.max_hp,
                    pos: *world.positions.get(i)?,
                    speed: *world.speed.get(i)?,
                    visible: *world.in_grid.get(i)?,
                    spawn: &self.spawn_groups.get(m.spawn_group as usize)?.line,
                })
            })
    }
}
