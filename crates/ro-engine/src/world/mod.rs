//! Per-map entity storage: dense columns indexed by `Entity::index()` plus sparse components.

use std::sync::Arc;

use ro_core::{CharId, CharacterSpawn, MapPosition, UnitLooks};
use rustc_hash::FxHashMap;

use crate::combat::AttackState;
use crate::ecs::{Entity, EntityAllocator, SparseSet, SpatialGrid8x8};
use crate::equipment::GearStatus;
use crate::error::EngineError;
use crate::item::PlayerInventory;
use crate::movement::WalkingPath;
use crate::skill::SkillSet;

mod components_dto;

pub(crate) use components_dto::{EntityKind, FloorItem, MobInfo, NpcInfo, PlayerInfo, WarpTrigger};

const ORIGIN: MapPosition = MapPosition { x: 0, y: 0, dir: 0 };

#[derive(Debug)]
pub(crate) struct World {
    pub(crate) entities: EntityAllocator,
    // Dense columns, one entry per allocator slot.
    pub(crate) positions: Vec<MapPosition>,
    pub(crate) kinds: Vec<EntityKind>,
    /// Wire id: account id (players), NPC/mob id, floor item id.
    pub(crate) gids: Vec<u32>,
    /// rAthena `dead_sit`: 0 standing, 2 sitting.
    pub(crate) dead_sit: Vec<u8>,
    pub(crate) head_dir: Vec<u16>,
    pub(crate) speed: Vec<u16>,
    /// Whether the entity is in the spatial grid (visible). Set at `NotifyActorInit`.
    pub(crate) in_grid: Vec<bool>,
    /// The latest move request received this tick (last one wins).
    pub(crate) pending_move: Vec<Option<(u16, u16)>>,
    // Sparse components.
    pub(crate) players: SparseSet<PlayerInfo>,
    pub(crate) inventories: SparseSet<PlayerInventory>,
    pub(crate) npcs: SparseSet<NpcInfo>,
    pub(crate) mobs: SparseSet<MobInfo>,
    pub(crate) floor_items: SparseSet<FloorItem>,
    pub(crate) warps: SparseSet<WarpTrigger>,
    pub(crate) walking_paths: SparseSet<WalkingPath>,
    /// Attack state of every unit that has attacked (kept after the attack stops: the delays
    /// outlive it).
    pub(crate) attacks: SparseSet<AttackState>,
    // Network id lookups.
    char_to_entity: FxHashMap<CharId, Entity>,
    gid_to_entity: FxHashMap<u32, Entity>,
    pub(crate) grid: SpatialGrid8x8,
}

impl World {
    pub(crate) fn new(width: u16, height: u16, capacity: u32) -> Self {
        Self {
            entities: EntityAllocator::with_capacity(capacity),
            positions: Vec::new(),
            kinds: Vec::new(),
            gids: Vec::new(),
            dead_sit: Vec::new(),
            head_dir: Vec::new(),
            speed: Vec::new(),
            in_grid: Vec::new(),
            pending_move: Vec::new(),
            players: SparseSet::new(),
            inventories: SparseSet::new(),
            npcs: SparseSet::new(),
            mobs: SparseSet::new(),
            floor_items: SparseSet::new(),
            warps: SparseSet::new(),
            walking_paths: SparseSet::new(),
            attacks: SparseSet::new(),
            char_to_entity: FxHashMap::default(),
            gid_to_entity: FxHashMap::default(),
            grid: SpatialGrid8x8::new(width, height),
        }
    }

    pub(crate) fn entity_by_char(&self, char_id: CharId) -> Option<Entity> {
        self.char_to_entity.get(&char_id).copied()
    }

    pub(crate) fn entity_by_gid(&self, gid: u32) -> Option<Entity> {
        self.gid_to_entity.get(&gid).copied()
    }

    /// Creates a player entity (not in the grid yet). A char id or account (gid) that is
    /// already present is rejected, so the lookup maps are never overwritten.
    pub(crate) fn spawn_player(&mut self, spawn: &CharacterSpawn) -> Result<Entity, EngineError> {
        if self.char_to_entity.contains_key(&spawn.char_id)
            || self.gid_to_entity.contains_key(&spawn.account_id.0)
        {
            return Err(EngineError::CharacterAlreadyEntered(spawn.char_id));
        }
        let entity = self.allocate(
            EntityKind::Player,
            spawn.account_id.0,
            spawn.pos,
            spawn.speed,
        )?;
        self.players.insert(
            entity,
            PlayerInfo {
                account_id: spawn.account_id,
                char_id: spawn.char_id,
                name: Arc::from(spawn.name.as_str()),
                sex: spawn.sex,
                job: spawn.job_class,
                hair_style: spawn.hair_style,
                hair_color: spawn.hair_color,
                clevel: spawn.base_level,
                status: spawn.status,
                group_id: spawn.group_id,
                derived: ro_mechanics::Status::default(),
                allocated: [0; 12],
                gear: GearStatus::default(),
                stat_flags: 0,
                followups: Vec::new(),
                looks: UnitLooks::default(),
                rewarp_count: 0,
                connect_new: false,
                changemap: false,
                skills: SkillSet::from_spawn(&spawn.skills, &spawn.hidden_skills),
                change_level: spawn.change_level,
                canuseitem_at: 0,
            },
        );
        self.char_to_entity.insert(spawn.char_id, entity);
        self.gid_to_entity.insert(spawn.account_id.0, entity);
        Ok(entity)
    }

    /// Creates an NPC entity (not in the grid; call `show` for visible ones). The gid must be
    /// unused.
    pub(crate) fn spawn_npc(
        &mut self,
        info: NpcInfo,
        pos: MapPosition,
        speed: u16,
        warp: Option<WarpTrigger>,
    ) -> Result<Entity, EngineError> {
        if self.gid_to_entity.contains_key(&info.gid) {
            return Err(EngineError::DuplicateGid(info.gid));
        }
        let entity = self.allocate(EntityKind::Npc, info.gid, pos, speed)?;
        self.gid_to_entity.insert(info.gid, entity);
        self.npcs.insert(entity, info);
        if let Some(warp) = warp {
            self.warps.insert(entity, warp);
        }
        Ok(entity)
    }

    /// Creates a mob entity (not in the grid; call `show`). The gid must be unused.
    pub(crate) fn spawn_mob(
        &mut self,
        info: MobInfo,
        pos: MapPosition,
        speed: u16,
    ) -> Result<Entity, EngineError> {
        if self.gid_to_entity.contains_key(&info.gid) {
            return Err(EngineError::DuplicateGid(info.gid));
        }
        let entity = self.allocate(EntityKind::Mob, info.gid, pos, speed)?;
        self.gid_to_entity.insert(info.gid, entity);
        self.mobs.insert(entity, info);
        Ok(entity)
    }

    /// Creates a floor item entity (not in the grid; call `show`). The id must be unused.
    pub(crate) fn spawn_floor_item(
        &mut self,
        item: FloorItem,
        pos: MapPosition,
    ) -> Result<Entity, EngineError> {
        if self.gid_to_entity.contains_key(&item.id) {
            return Err(EngineError::DuplicateGid(item.id));
        }
        let entity = self.allocate(EntityKind::FloorItem, item.id, pos, 0)?;
        self.gid_to_entity.insert(item.id, entity);
        self.floor_items.insert(entity, item);
        Ok(entity)
    }

    /// Whether `gid` is in use by any entity on this map.
    pub(crate) fn gid_taken(&self, gid: u32) -> bool {
        self.gid_to_entity.contains_key(&gid)
    }

    /// Allocates a slot and resets its dense columns.
    fn allocate(
        &mut self,
        kind: EntityKind,
        gid: u32,
        pos: MapPosition,
        speed: u16,
    ) -> Result<Entity, EngineError> {
        let entity = self.entities.allocate()?;
        let slots = self.entities.slot_count();
        if self.positions.len() < slots {
            self.positions.resize(slots, ORIGIN);
            self.kinds.resize(slots, EntityKind::Free);
            self.gids.resize(slots, 0);
            self.dead_sit.resize(slots, 0);
            self.head_dir.resize(slots, 0);
            self.speed.resize(slots, 0);
            self.in_grid.resize(slots, false);
            self.pending_move.resize(slots, None);
        }
        let i = entity.index();
        self.positions[i] = pos;
        self.kinds[i] = kind;
        self.gids[i] = gid;
        self.dead_sit[i] = 0;
        self.head_dir[i] = 0;
        self.speed[i] = speed;
        self.in_grid[i] = false;
        self.pending_move[i] = None;
        Ok(entity)
    }

    /// Puts the entity into the spatial grid at its current cell.
    pub(crate) fn show(&mut self, entity: Entity) {
        let i = entity.index();
        if self.entities.is_alive(entity) && !self.in_grid[i] {
            let pos = self.positions[i];
            self.grid.insert(entity, pos.x, pos.y);
            self.in_grid[i] = true;
        }
    }

    /// Takes the entity out of the spatial grid (the inverse of `show`).
    pub(crate) fn hide(&mut self, entity: Entity) {
        let i = entity.index();
        if self.entities.is_alive(entity) && self.in_grid[i] {
            let pos = self.positions[i];
            self.grid.remove(entity, pos.x, pos.y);
            self.in_grid[i] = false;
        }
    }

    /// Removes the entity everywhere and frees its slot.
    pub(crate) fn despawn(&mut self, entity: Entity) -> Result<(), EngineError> {
        if !self.entities.is_alive(entity) {
            return Err(EngineError::EntityNotFound(entity.index() as u32));
        }
        let i = entity.index();
        self.hide(entity);
        if let Some(info) = self.players.remove(entity) {
            if self.char_to_entity.get(&info.char_id) == Some(&entity) {
                self.char_to_entity.remove(&info.char_id);
            }
            if self.gid_to_entity.get(&info.account_id.0) == Some(&entity) {
                self.gid_to_entity.remove(&info.account_id.0);
            }
        }
        if let Some(info) = self.npcs.remove(entity) {
            if self.gid_to_entity.get(&info.gid) == Some(&entity) {
                self.gid_to_entity.remove(&info.gid);
            }
        }
        if let Some(info) = self.mobs.remove(entity) {
            if self.gid_to_entity.get(&info.gid) == Some(&entity) {
                self.gid_to_entity.remove(&info.gid);
            }
        }
        if let Some(item) = self.floor_items.remove(entity) {
            if self.gid_to_entity.get(&item.id) == Some(&entity) {
                self.gid_to_entity.remove(&item.id);
            }
        }
        self.inventories.remove(entity);
        self.warps.remove(entity);
        self.walking_paths.remove(entity);
        self.attacks.remove(entity);
        self.pending_move[i] = None;
        self.kinds[i] = EntityKind::Free;
        self.gids[i] = 0;
        self.entities.free(entity)
    }
}

#[cfg(test)]
mod tests;
