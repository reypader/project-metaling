//! Per-map entity storage: dense columns indexed by `Entity::index()` plus sparse components.

use std::sync::Arc;

use ro_core::{
    AccountId, CharId, CharacterSpawn, CharacterStatus, InventoryItem, MapPosition, Sex, UnitLooks,
    UnitObjectType,
};
use ro_db::{MobId, NpcState};
use rustc_hash::FxHashMap;

use crate::entity::{Entity, EntityAllocator};
use crate::equipment::GearStatus;
use crate::error::EngineError;
use crate::inventory::PlayerInventory;
use crate::movement::WalkingPath;
use crate::sparse_set::SparseSet;
use crate::spatial::SpatialGrid8x8;
use crate::stats::FollowUp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntityKind {
    /// Unused slot.
    Free,
    Player,
    Npc,
    Mob,
    FloorItem,
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
    /// Allocated stats, job level, points, exp and HP/SP (the persisted progress).
    pub(crate) status: CharacterStatus,
    /// Account permission group; 99 may use every GM command.
    pub(crate) group_id: i32,
    /// The status snapshot committed last (spec `specs/stats/spec.md` §2.1): computed at spawn,
    /// then by the tick's recalc pass, which diffs against it.
    pub(crate) derived: ro_mechanics::Status,
    /// The allocated STR..LUK `derived` was computed from: the diff sends a stat's `0x0141`
    /// when its allocated base or its battle value changed.
    pub(crate) allocated: [u16; 6],
    /// The worn gear's part of the status, rebuilt only when an equip change marks it stale.
    pub(crate) gear: GearStatus,
    /// `stats::DIRTY` / `stats::STALE_GEAR` / `stats::LISTED`: what the tick's recalc pass owes
    /// this player.
    pub(crate) stat_flags: u8,
    /// Packets and actions a flow queued for after this tick's recalc (spec §2.1 "Follow-up
    /// queue"). Drained in Stage 4; the capacity is reused across ticks.
    pub(crate) followups: Vec<FollowUp>,
    /// What other players see, as last sent (spec §2.1). Built at spawn and refreshed by every
    /// equip change; entry packets read it, never the inventory.
    pub(crate) looks: UnitLooks,
    /// Consecutive warps that landed inside a warp area (rAthena `count_rewarp`).
    pub(crate) rewarp_count: u8,
    /// First entry after a login (rAthena `connect_new`): the next actor init sends the skill
    /// list and hotkeys.
    pub(crate) connect_new: bool,
    /// Arrived from login or another map (rAthena `changemap`): the next actor init sends the
    /// config flags and reputation list. Not set by a same-map warp.
    pub(crate) changemap: bool,
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

/// Identity and state of a mob entity. Stats are placeholders until mob_db stats exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MobInfo {
    pub(crate) gid: u32,
    pub(crate) mob_id: MobId,
    /// Shared with the spawn group.
    pub(crate) name: Arc<str>,
    pub(crate) level: u16,
    pub(crate) hp: u32,
    pub(crate) max_hp: u32,
    /// Index into `MapInstance::spawn_groups`: the line this mob respawns from.
    pub(crate) spawn_group: u32,
}

/// An item lying on the floor (rAthena `flooritem_data`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FloorItem {
    /// Object id, `[2, 2_000_000)`; also the entity's gid.
    pub(crate) id: u32,
    /// The item; `amount` is the floor amount.
    pub(crate) item: InventoryItem,
    /// Sub-cell offsets, `rnd[1,4] × 3`.
    pub(crate) sub_x: u8,
    pub(crate) sub_y: u8,
    /// Loot owners by tier (char ids); `None` = nobody.
    pub(crate) owners: [Option<CharId>; 3],
    /// When each tier's wait ends (ms).
    pub(crate) owner_ticks: [u64; 3],
    /// Removal time (ms).
    pub(crate) expire_at: u64,
    /// The mob that dropped it; 0 for player drops.
    pub(crate) mob_id: u32,
}

/// A warp's touch area and destination, resolved at spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WarpTrigger {
    /// `(xs, ys)`: the area is `(2xs+1) x (2ys+1)` around the warp's cell.
    pub(crate) half_extents: (u16, u16),
    /// Destination map as a `.gat` name.
    pub(crate) dest_map: Arc<str>,
    /// Raw destination cell; `(0, 0)` or out of bounds means a random cell.
    pub(crate) dest: (u16, u16),
    /// `warp2`: fires for hidden players too (no hiding state yet).
    pub(crate) trigger_while_hidden: bool,
    /// `dest_map` is this instance's map.
    pub(crate) same_map: bool,
}

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
                allocated: [0; 6],
                gear: GearStatus::default(),
                stat_flags: 0,
                followups: Vec::new(),
                looks: UnitLooks::default(),
                rewarp_count: 0,
                connect_new: false,
                changemap: false,
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
        self.pending_move[i] = None;
        self.kinds[i] = EntityKind::Free;
        self.gids[i] = 0;
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
            status: Default::default(),
            group_id: 0,
            inventory: Default::default(),
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
