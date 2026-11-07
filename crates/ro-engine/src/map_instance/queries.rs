//! Read-only lookups on entities and players, for the systems and the map-server.

use std::sync::Arc;

use ro_core::{CharId, MapPosition, Sex, UnitAppearance, UnitLooks, UnitObjectType};

use super::MapInstance;
use crate::ecs::{Entity, AREA_SIZE};
use crate::item::PlayerInventory;
use crate::warp::PlaySnapshot;

impl MapInstance {
    /// Whether the character currently has a walk in progress.
    pub fn is_walking(&self, char_id: CharId) -> bool {
        self.world
            .entity_by_char(char_id)
            .is_some_and(|e| self.world.walking_paths.contains(e))
    }

    /// The character's `dead_sit` state (0 standing, 2 sitting).
    pub fn dead_sit(&self, char_id: CharId) -> Option<u8> {
        let e = self.world.entity_by_char(char_id)?;
        self.world.dead_sit.get(e.index()).copied()
    }

    pub(crate) fn gid(&self, e: Entity) -> u32 {
        // The dense column has no generation: a stale handle would read the slot's new occupant.
        debug_assert!(self.world.entities.is_alive(e), "gid of a dead entity");
        self.world.gids.get(e.index()).copied().unwrap_or(0)
    }

    pub(crate) fn char_of(&self, e: Entity) -> Option<CharId> {
        self.world.players.get(e).map(|p| p.char_id)
    }

    pub(crate) fn pos(&self, e: Entity) -> MapPosition {
        self.world.positions[e.index()]
    }

    pub(crate) fn cell(&self, e: Entity) -> (u16, u16) {
        let p = self.pos(e);
        (p.x, p.y)
    }

    pub(crate) fn appearance(&self, e: Entity) -> Option<UnitAppearance> {
        let i = e.index();
        let Some(info) = self.world.players.get(e) else {
            let Some(npc) = self.world.npcs.get(e) else {
                let mob = self.world.mobs.get(e)?;
                return Some(UnitAppearance {
                    object_type: UnitObjectType::Mob,
                    gid: mob.gid,
                    char_id: None,
                    name: Arc::clone(&mob.name),
                    // Valid mob ids (<= 31998) fit the wire's u16.
                    job: u16::try_from(mob.mob_id.0).unwrap_or(0),
                    sex: Sex::Female,
                    hair_style: 0,
                    hair_color: 0,
                    head_dir: 0,
                    pos: self.world.positions[i],
                    speed: self.world.speed[i],
                    dead_sit: 0,
                    clevel: mob.level,
                    is_boss: mob.is_boss,
                    effect_state: 0,
                    looks: UnitLooks::default(),
                });
            };
            return Some(UnitAppearance {
                object_type: npc.object_type,
                gid: npc.gid,
                char_id: None,
                name: Arc::clone(&npc.name),
                job: npc.sprite,
                sex: Sex::Female,
                hair_style: 0,
                hair_color: 0,
                head_dir: 0,
                pos: self.world.positions[i],
                speed: self.world.speed[i],
                dead_sit: 0,
                clevel: 0,
                is_boss: 0,
                effect_state: npc.effect_state,
                looks: UnitLooks::default(),
            });
        };
        Some(UnitAppearance {
            object_type: UnitObjectType::Pc,
            gid: info.account_id.0,
            char_id: Some(info.char_id),
            name: Arc::clone(&info.name),
            job: info.job,
            sex: info.sex,
            hair_style: info.hair_style,
            hair_color: info.hair_color,
            head_dir: self.world.head_dir[i],
            pos: self.world.positions[i],
            speed: self.world.speed[i],
            dead_sit: self.world.dead_sit[i],
            clevel: info.clevel,
            is_boss: 0,
            effect_state: 0,
            looks: info.looks,
        })
    }

    pub fn map_name(&self) -> &str {
        &self.map_name
    }

    /// A copy of what a save writes: the inventory, the char-select looks of the worn gear and
    /// the progress.
    pub fn play_snapshot(&self, char_id: CharId) -> Option<PlaySnapshot> {
        let progress = self.progress_snapshot(char_id)?;
        let e = self.world.entity_by_char(char_id)?;
        let skills = self.world.players.get(e)?.skills.saved_rows();
        self.world.inventories.get(e).map(|inventory| PlaySnapshot {
            inventory: inventory.snapshot(),
            looks: inventory.gear(&self.items).character_looks(),
            progress,
            skills,
        })
    }

    /// Changes whenever the character's saved skill rows may have changed (0 after spawn). The
    /// map-server compares it with the revision it last saved.
    pub fn skills_revision(&self, char_id: CharId) -> Option<u32> {
        let e = self.world.entity_by_char(char_id)?;
        self.world.players.get(e).map(|info| info.skills.revision())
    }

    /// Changes whenever the character's inventory changes (0 after spawn). The map-server
    /// compares it with the revision it last saved.
    pub fn inventory_revision(&self, char_id: CharId) -> Option<u64> {
        let e = self.world.entity_by_char(char_id)?;
        self.world.inventories.get(e).map(PlayerInventory::revision)
    }

    /// Moves the characters flows asked to save right away (`@job`) into `out`.
    pub fn drain_save_requests(&mut self, out: &mut Vec<CharId>) {
        out.append(&mut self.save_requests);
    }

    /// Current and max weight (0.1 units).
    pub fn weight(&self, char_id: CharId) -> Option<(u32, u32)> {
        let e = self.world.entity_by_char(char_id)?;
        self.world
            .inventories
            .get(e)
            .map(|i| (i.weight(), i.max_weight()))
    }

    /// Clears `out`, then fills it with the characters visible from `center` (Chebyshev range
    /// `AREA_SIZE`). The caller reuses `out` across calls so routing doesn't allocate per event.
    pub fn query_aoi_characters(&self, center: MapPosition, out: &mut Vec<CharId>) {
        out.clear();
        let players = &self.world.players;
        self.world
            .grid
            .visit_radius(center.x, center.y, AREA_SIZE, &self.world.positions, |e| {
                if let Some(info) = players.get(e) {
                    out.push(info.char_id);
                }
            });
    }

    pub fn character_position(&self, char_id: CharId) -> Option<MapPosition> {
        let e = self.world.entity_by_char(char_id)?;
        self.world.positions.get(e.index()).copied()
    }
}
