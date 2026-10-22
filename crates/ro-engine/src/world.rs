//! Per-map entity storage: dense columns indexed by `Entity::index()` plus sparse components.

use std::sync::Arc;

use ro_core::{AccountId, CharId, CharacterSpawn, MapPosition, Sex, UnitObjectType};
use ro_db::{NpcState, WarpDest};
use rustc_hash::FxHashMap;

use crate::entity::{Entity, EntityAllocator};
use crate::error::EngineError;
use crate::movement::WalkingPath;
use crate::sparse_set::SparseSet;
use crate::spatial::SpatialGrid8x8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntityKind {
    /// Unused slot.
    Free,
    Player,
    Npc,
}

/// Static appearance and identity of a player entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlayerInfo {
    pub(crate) account_id: AccountId,
    pub(crate) char_id: CharId,
    pub(crate) name: Arc<str>,
    pub(crate) sex: Sex,
    pub(crate) job: u16,
    pub(crate) hair_style: u16,
    pub(crate) hair_color: u16,
    pub(crate) clevel: u16,
}

/// Static appearance and identity of an NPC entity (warps included).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NpcInfo {
    pub(crate) gid: u32,
    pub(crate) name: Arc<str>,
    pub(crate) object_type: UnitObjectType,
    /// Class id sent as `job`.
    pub(crate) sprite: u16,
    /// `effect_state` (`sc.option`): hidden/cloaked NPCs.
    pub(crate) effect_state: i32,
    /// Initial state. Warp map change must skip `Disabled` and `Cloaked` warps
    /// (rAthena `npc_touch_areanpc`).
    pub(crate) state: NpcState,
}

/// A warp's touch area and destination (used by warp map change).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WarpTrigger {
    /// `(xs, ys)`: the area is `(2xs+1) x (2ys+1)` around the warp's cell.
    pub(crate) half_extents: (u16, u16),
    pub(crate) dest: WarpDest,
}

const ORIGIN: MapPosition = MapPosition { x: 0, y: 0, dir: 0 };

#[derive(Debug)]
pub(crate) struct World {
    pub(crate) entities: EntityAllocator,
    // Dense columns, one entry per allocator slot.
    pub(crate) positions: Vec<MapPosition>,
    pub(crate) kinds: Vec<EntityKind>,
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
    pub(crate) npcs: SparseSet<NpcInfo>,
    pub(crate) warps: SparseSet<WarpTrigger>,
    pub(crate) walking_paths: SparseSet<WalkingPath>,
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
            dead_sit: Vec::new(),
            head_dir: Vec::new(),
            speed: Vec::new(),
            in_grid: Vec::new(),
            pending_move: Vec::new(),
            players: SparseSet::new(),
            npcs: SparseSet::new(),
            warps: SparseSet::new(),
            walking_paths: SparseSet::new(),
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
        let entity = self.allocate(EntityKind::Player, spawn.pos, spawn.speed)?;
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
        let entity = self.allocate(EntityKind::Npc, pos, speed)?;
        self.gid_to_entity.insert(info.gid, entity);
        self.npcs.insert(entity, info);
        if let Some(warp) = warp {
            self.warps.insert(entity, warp);
        }
        Ok(entity)
    }

    /// Allocates a slot and resets its dense columns.
    fn allocate(
        &mut self,
        kind: EntityKind,
        pos: MapPosition,
        speed: u16,
    ) -> Result<Entity, EngineError> {
        let entity = self.entities.allocate()?;
        let slots = self.entities.slot_count();
        if self.positions.len() < slots {
            self.positions.resize(slots, ORIGIN);
            self.kinds.resize(slots, EntityKind::Free);
            self.dead_sit.resize(slots, 0);
            self.head_dir.resize(slots, 0);
            self.speed.resize(slots, 0);
            self.in_grid.resize(slots, false);
            self.pending_move.resize(slots, None);
        }
        let i = entity.index();
        self.positions[i] = pos;
        self.kinds[i] = kind;
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

    /// Removes the entity everywhere and frees its slot.
    pub(crate) fn despawn(&mut self, entity: Entity) -> Result<(), EngineError> {
        if !self.entities.is_alive(entity) {
            return Err(EngineError::EntityNotFound(entity.index() as u32));
        }
        let i = entity.index();
        if self.in_grid[i] {
            let pos = self.positions[i];
            self.grid.remove(entity, pos.x, pos.y);
            self.in_grid[i] = false;
        }
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
        self.warps.remove(entity);
        self.walking_paths.remove(entity);
        self.pending_move[i] = None;
        self.kinds[i] = EntityKind::Free;
        self.entities.free(entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ro_core::{AccountId, CharId, CharacterSpawn, MapPosition, Sex};

    fn spawn(account: u32, char_id: u32, x: u16, y: u16) -> CharacterSpawn {
        CharacterSpawn {
            account_id: AccountId(account),
            char_id: CharId(char_id),
            name: format!("c{char_id}"),
            sex: Sex::Male,
            job_class: 0,
            base_level: 1,
            hair_style: 1,
            hair_color: 1,
            pos: MapPosition { x, y, dir: 4 },
            speed: 150,
        }
    }

    #[test]
    fn eng_02_dense_columns_indexed_by_entity_index() {
        let mut world = World::new(200, 200, 16);
        let a = world
            .spawn_player(&spawn(2000001, 150001, 100, 100))
            .expect("spawn a");
        let b = world
            .spawn_player(&spawn(2000002, 150002, 103, 100))
            .expect("spawn b");
        assert_eq!(
            world.positions[a.index()],
            MapPosition {
                x: 100,
                y: 100,
                dir: 4
            }
        );
        assert_eq!(world.positions[b.index()].x, 103);
        assert_eq!(world.kinds[a.index()], EntityKind::Player);
        assert_eq!(world.speed[b.index()], 150);
        assert_eq!(world.dead_sit[a.index()], 0);
        assert_eq!(world.entity_by_char(CharId(150002)), Some(b));
        assert_eq!(world.entity_by_gid(2000001), Some(a));
        assert_eq!(world.players.get(a).map(|p| &*p.name), Some("c150001"));
        assert!(
            !world.in_grid[a.index()],
            "spawned entities are not visible yet"
        );
    }

    #[test]
    fn eng_02_despawn_clears_lookups_and_stale_handles() {
        let mut world = World::new(200, 200, 16);
        let a = world
            .spawn_player(&spawn(2000001, 150001, 100, 100))
            .expect("spawn");
        world.show(a);
        assert!(world.in_grid[a.index()]);
        world.despawn(a).expect("despawn live entity");
        assert_eq!(world.kinds[a.index()], EntityKind::Free);
        assert_eq!(world.entity_by_char(CharId(150001)), None);
        assert_eq!(world.entity_by_gid(2000001), None);
        assert!(world.players.get(a).is_none());
        assert!(!world.in_grid[a.index()]);
        assert!(world.despawn(a).is_err(), "stale handle");
        let b = world
            .spawn_player(&spawn(2000002, 150002, 1, 1))
            .expect("reuse slot");
        assert_eq!(b.index(), a.index());
        assert!(world.players.get(a).is_none(), "old handle sees nothing");
    }

    #[test]
    fn duplicate_ids_are_rejected_and_never_clobber_lookups() {
        let mut world = World::new(200, 200, 16);
        let a = world
            .spawn_player(&spawn(2000001, 150001, 1, 1))
            .expect("spawn");
        assert!(
            world.spawn_player(&spawn(2000001, 150009, 1, 1)).is_err(),
            "same account"
        );
        assert!(
            world.spawn_player(&spawn(2000009, 150001, 1, 1)).is_err(),
            "same char"
        );
        assert_eq!(world.entity_by_gid(2000001), Some(a));
        assert_eq!(world.entity_by_char(CharId(150001)), Some(a));
        assert_eq!(
            world.entities.slot_count(),
            1,
            "rejected spawns allocate nothing"
        );
    }

    #[test]
    fn eng_02_capacity_is_enforced() {
        let mut world = World::new(50, 50, 1);
        world.spawn_player(&spawn(1, 1, 1, 1)).expect("first");
        assert!(world.spawn_player(&spawn(2, 2, 1, 1)).is_err());
    }
}
