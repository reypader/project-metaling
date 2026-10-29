//! `MapInstance`: one map's simulation, and its lifecycle/tick API.
//!
//! `step_tick` order: expire floor items -> advance every due step (AoI diff per step, pending
//! redirect repath at step end) -> drain intents FIFO (the last move per entity per tick wins)
//! -> plan paths -> the stat pass (recalc of dirty players, then their follow-ups) -> return the
//! events. Rules: spec `specs/engine-foundation/spec.md` §0, `specs/stats/spec.md` §2.1.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::mem;
use std::sync::Arc;

use ro_core::{
    gat_map_name, BroadcastScope, CharId, CharacterSpawn, ClientIntent, InventoryItem, MapPosition,
    ScopedServerEvent, ServerEvent, SessionIntent, Sex, UnitAppearance, UnitLooks, UnitObjectType,
};
use ro_db::{
    CellType, ItemDb, JobDb, MapGrid, MapNpcs, MobDb, NpcKind, NpcSpawn, NpcState, RefineDb,
};
use ro_mechanics::BattleConfig;
use smallvec::SmallVec;
use tracing::{debug, warn};

use crate::entity::Entity;
use crate::error::EngineError;
use crate::floor_item::MIN_FLOOR_ID;
use crate::inventory::PlayerInventory;
use crate::movement::WalkingPath;
use crate::path::{AStarScratchBuffer, MAX_WALKPATH, MAX_WALK_PATH_CLICK};
use crate::spatial::{step_visibility, AREA_SIZE};

/// `CZ_REQUEST_ACT` attack types; every other action stops a walk first (rAthena
/// `clif_parse_ActionRequest_sub`).
const ACTION_ATTACK: u8 = 0;
const ACTION_ATTACK_REPEAT: u8 = 7;
use crate::mob::SpawnGroup;
use crate::rng::SplitMix64;
use crate::settings::Arrival;
use crate::warp::{CellBits, MapTransfer, PlaySnapshot};
use crate::world::{NpcInfo, WarpTrigger, World};

/// Default entity capacity of one map instance.
/// Capacity is only a limit: the columns grow as entities are created. Floor items count too.
pub const DEFAULT_ENTITY_CAPACITY: u32 = 16_384;

/// `ZC_PAR_CHANGE` var ids (rAthena `SP_WEIGHT`, `SP_MAXWEIGHT`), in 0.1 weight units.
pub(crate) const SP_WEIGHT: u16 = 24;
pub(crate) const SP_MAXWEIGHT: u16 = 25;

/// `CZ_REQUEST_ACT` action types.
const ACTION_SIT: u8 = 2;
const ACTION_STAND: u8 = 3;
/// rAthena `dead_sit` values.
const STANDING: u8 = 0;
const SITTING: u8 = 2;

/// rAthena `START_NPC_NUM`: first NPC unit id. Above every account id (< 100,000,000).
pub const START_NPC_ID: u32 = 110_000_000;
/// rAthena `DEFAULT_NPC_WALK_SPEED`, sent as an NPC's speed.
const NPC_SPEED: u16 = 200;
/// NPC classes the client is never sent: `JT_FAKENPC` and `JT_INVISIBLE`.
const FAKE_NPC_SPRITE: i16 = -1;
const INVISIBLE_SPRITE: i16 = 32767;
/// `effect_state` bits (`sc.option`) for hidden and cloaked NPCs.
const OPTION_HIDE: i32 = 0x2;
const OPTION_CLOAK: i32 = 0x4;

/// `ZC_NOTIFY_VANISH` types.
pub(crate) const VANISH_OUT_OF_SIGHT: u8 = 0;
const VANISH_LOGGED_OUT: u8 = 2;

type PathCells = SmallVec<[(u16, u16); MAX_WALKPATH]>;

/// One map's world plus the reusable buffers its tick needs.
pub struct MapInstance {
    pub(crate) map_name: String,
    /// `map_name` as a `.gat` name, sent in `0x0091`.
    pub(crate) gat_name: Arc<str>,
    pub(crate) world: World,
    /// Shared with other instances of the same map until `set_cell` copies it.
    pub(crate) walkability: Arc<MapGrid>,
    /// Walkable cells inside any warp's trigger area.
    pub(crate) warp_cells: CellBits,
    /// Random warp destinations; seeded from the map name.
    pub(crate) rng: SplitMix64,
    intents: Vec<SessionIntent>,
    /// Entities with a `pending_move` set this tick (the column holds the last request).
    moves: Vec<Entity>,
    /// Scratch for resolving AoI recipients at emission.
    recipients: Vec<Entity>,
    walkers: Vec<Entity>,
    /// Units near a stepping mover (radius `AREA_SIZE + 1` around its old cell).
    step_candidates: Vec<Entity>,
    aoi_after: Vec<Entity>,
    left: Vec<Entity>,
    entered: Vec<Entity>,
    scratch: AStarScratchBuffer,
    pub(crate) events: Vec<ScopedServerEvent>,
    /// Players that left through a cross-map warp, until `drain_transfers`.
    pub(crate) transfers: Vec<MapTransfer>,
    /// Next NPC/mob unit id (per instance; a client only sees one map at a time). NPCs and
    /// mobs share it, like rAthena's single pool.
    pub(crate) next_npc_id: u32,
    /// Normalized mob spawn lines; each mob keeps the index of its line.
    pub(crate) spawn_groups: Vec<SpawnGroup>,
    /// Floor item expiry times `(expire_at, floor id)`, soonest first. Entries for items already
    /// gone are skipped when popped.
    pub(crate) floor_expiry: BinaryHeap<Reverse<(u64, u32)>>,
    /// Next floor item id to try (cycles through `[2, 2_000_000)`).
    pub(crate) next_floor_id: u32,
    /// Global registries shared by every instance (empty until `set_registries`).
    pub(crate) items: Arc<ItemDb>,
    pub(crate) mob_db: Arc<MobDb>,
    pub(crate) jobs: Arc<JobDb>,
    pub(crate) refine: Arc<RefineDb>,
    pub(crate) battle: Arc<BattleConfig>,
    /// Players the tick's stat pass owes a recalc or follow-ups, in marking order (each once;
    /// `PlayerInfo::stat_flags` has `LISTED`). Reused across ticks.
    pub(crate) stat_queue: Vec<Entity>,
    /// Characters a flow asked to save right away (`@job`); the map-server drains them.
    pub(crate) save_requests: Vec<CharId>,
}

/// The read-only data every map instance shares (`MapInstance::set_registries`).
#[derive(Debug, Clone, Default)]
pub struct Registries {
    pub items: Arc<ItemDb>,
    pub mobs: Arc<MobDb>,
    pub jobs: Arc<JobDb>,
    pub refine: Arc<RefineDb>,
    pub battle: Arc<BattleConfig>,
}

/// rAthena direction of a one-cell step (0 N, 1 NW, 2 W, 3 SW, 4 S, 5 SE, 6 E, 7 NE; y+ is north).
fn step_dir(from: (u16, u16), to: (u16, u16)) -> u8 {
    let dx = i32::from(to.0) - i32::from(from.0);
    let dy = i32::from(to.1) - i32::from(from.1);
    match (dx.signum(), dy.signum()) {
        (0, 1) => 0,
        (-1, 1) => 1,
        (-1, 0) => 2,
        (-1, -1) => 3,
        (0, -1) => 4,
        (1, -1) => 5,
        (1, 0) => 6,
        (1, 1) => 7,
        _ => 0,
    }
}

impl MapInstance {
    pub fn new(map_name: impl Into<String>, walkability: Arc<MapGrid>) -> Self {
        Self::with_capacity(map_name, walkability, DEFAULT_ENTITY_CAPACITY)
    }

    pub fn with_capacity(
        map_name: impl Into<String>,
        walkability: Arc<MapGrid>,
        capacity: u32,
    ) -> Self {
        let map_name = map_name.into();
        let world = World::new(walkability.width(), walkability.height(), capacity);
        Self {
            gat_name: Arc::from(gat_map_name(&map_name)),
            warp_cells: CellBits::new(walkability.width(), walkability.height()),
            rng: SplitMix64::from_name(&map_name),
            map_name,
            world,
            walkability,
            intents: Vec::new(),
            moves: Vec::new(),
            recipients: Vec::new(),
            walkers: Vec::new(),
            step_candidates: Vec::new(),
            aoi_after: Vec::new(),
            left: Vec::new(),
            entered: Vec::new(),
            scratch: AStarScratchBuffer::new(),
            events: Vec::new(),
            transfers: Vec::new(),
            next_npc_id: START_NPC_ID,
            spawn_groups: Vec::new(),
            floor_expiry: BinaryHeap::new(),
            next_floor_id: MIN_FLOOR_ID,
            items: Arc::new(ItemDb::default()),
            mob_db: Arc::new(MobDb::default()),
            jobs: Arc::new(JobDb::default()),
            refine: Arc::new(RefineDb::default()),
            battle: Arc::new(BattleConfig::default()),
            stat_queue: Vec::new(),
            save_requests: Vec::new(),
        }
    }

    /// Shares the global registries with this instance. Call before spawning characters:
    /// weights and the status are computed at spawn.
    pub fn set_registries(&mut self, registries: Registries) {
        self.items = registries.items;
        self.mob_db = registries.mobs;
        self.jobs = registries.jobs;
        self.refine = registries.refine;
        self.battle = registries.battle;
    }

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

    // ---------- helpers ----------

    pub(crate) fn gid(&self, e: Entity) -> u32 {
        // The dense column has no generation: a stale handle would read the slot's new occupant.
        debug_assert!(self.world.entities.is_alive(e), "gid of a dead entity");
        self.world.gids.get(e.index()).copied().unwrap_or(0)
    }

    fn char_of(&self, e: Entity) -> Option<CharId> {
        self.world.players.get(e).map(|p| p.char_id)
    }

    pub(crate) fn pos(&self, e: Entity) -> MapPosition {
        self.world.positions[e.index()]
    }

    pub(crate) fn cell(&self, e: Entity) -> (u16, u16) {
        let p = self.pos(e);
        (p.x, p.y)
    }

    fn appearance(&self, e: Entity) -> Option<UnitAppearance> {
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
            effect_state: 0,
            looks: info.looks,
        })
    }

    /// Characters that see `e` right now (Chebyshev 14, visible only), sorted, optionally
    /// without `e` itself. Resolved at emission like rAthena's `clif_send(AREA)`.
    fn aoi_recipients(&mut self, e: Entity, exclude_self: bool) -> Vec<CharId> {
        self.recipients_within(e, AREA_SIZE, exclude_self)
    }

    /// Characters within Chebyshev `radius` of `e` (visible only), sorted.
    pub(crate) fn recipients_within(
        &mut self,
        e: Entity,
        radius: u16,
        exclude_self: bool,
    ) -> Vec<CharId> {
        let (x, y) = self.cell(e);
        self.world
            .grid
            .query_radius(x, y, radius, &self.world.positions, &mut self.recipients);
        let mut chars = Vec::with_capacity(self.recipients.len());
        for &r in &self.recipients {
            if exclude_self && r == e {
                continue;
            }
            if let Some(info) = self.world.players.get(r) {
                chars.push(info.char_id);
            }
        }
        chars.sort_unstable();
        chars
    }

    pub(crate) fn emit_self(&mut self, e: Entity, event: ServerEvent) {
        if let Some(char_id) = self.char_of(e) {
            self.events.push(ScopedServerEvent::new(
                BroadcastScope::SelfOnly(char_id),
                event,
            ));
        }
    }

    pub(crate) fn emit_aoi(&mut self, e: Entity, exclude_self: bool, event: ServerEvent) {
        let chars = self.aoi_recipients(e, exclude_self);
        if !chars.is_empty() {
            self.events.push(ScopedServerEvent::new(
                BroadcastScope::Characters(chars),
                event,
            ));
        }
    }

    /// Final cell of `e`'s walk (a pending redirect target wins), if walking.
    fn walk_dest(&self, e: Entity) -> Option<(u16, u16)> {
        let path = self.world.walking_paths.get(e)?;
        Some(path.pending_target.unwrap_or_else(|| path.dest()))
    }

    /// How `subject` looks to an observer that just started seeing it.
    fn entry_event(&self, subject: Entity, tick: u32) -> Option<ServerEvent> {
        if self.is_floor_item(subject) {
            return self
                .floor_appearance(subject)
                .map(|item| ServerEvent::FloorItemEntered { item });
        }
        let unit = self.appearance(subject)?;
        Some(match self.walk_dest(subject) {
            Some(dest) => ServerEvent::UnitWalking {
                start: (unit.pos.x, unit.pos.y),
                unit,
                dest,
                move_start_tick: tick,
            },
            None => ServerEvent::UnitStanding { unit },
        })
    }

    fn query_into_after(&mut self, center: (u16, u16)) {
        self.world.grid.query_radius(
            center.0,
            center.1,
            AREA_SIZE,
            &self.world.positions,
            &mut self.aoi_after,
        );
    }

    // ---------- intents ----------

    fn handle_intent(&mut self, intent: SessionIntent, now_ms: u64) {
        let Some(e) = intent
            .char_id
            .and_then(|char_id| self.world.entity_by_char(char_id))
        else {
            debug!(map = %self.map_name, ?intent, "dropping intent for unknown character");
            return;
        };
        let visible = self.world.in_grid[e.index()];
        match intent.intent {
            ClientIntent::RequestTime { .. } => self.emit_self(
                e,
                ServerEvent::TimeSynchronized {
                    server_tick: now_ms as u32,
                },
            ),
            ClientIntent::NotifyActorInit => self.actor_init(e, now_ms),
            _ if !visible => {
                debug!(map = %self.map_name, "ignoring gameplay intent before actor init");
            }
            ClientIntent::RequestMove { dest_x, dest_y } => {
                let slot = &mut self.world.pending_move[e.index()];
                if slot.is_none() {
                    self.moves.push(e);
                }
                *slot = Some((dest_x, dest_y));
            }
            ClientIntent::ChangeDirection { head_dir, body_dir } => {
                let i = e.index();
                self.world.head_dir[i] = head_dir;
                self.world.positions[i].dir = body_dir;
                let gid = self.gid(e);
                self.emit_aoi(
                    e,
                    true,
                    ServerEvent::DirectionChanged {
                        gid,
                        head_dir,
                        body_dir,
                    },
                );
            }
            ClientIntent::RequestAction { action_type, .. } => {
                if !matches!(action_type, ACTION_ATTACK | ACTION_ATTACK_REPEAT) {
                    self.stop_walking(e);
                }
                match action_type {
                    ACTION_SIT => self.sit(e),
                    ACTION_STAND => self.stand(e),
                    other => debug!(map = %self.map_name, action = other, "ignoring action"),
                }
            }
            ClientIntent::RequestEntityName { entity_id } => self.name_request(e, entity_id),
            ClientIntent::PickUpItem { floor_id } => self.pick_up(e, floor_id, now_ms),
            ClientIntent::Chat { text } => self.chat(e, &text),
            ClientIntent::DropItem { index, amount } => self.drop_item(e, index, amount, now_ms),
            ClientIntent::EquipItem { index, position } => self.equip_item(e, index, position),
            ClientIntent::UnequipItem { index } => self.unequip_item(e, index),
            ClientIntent::UnequipAll { location } => self.unequip_all(e, location),
            ClientIntent::RaiseStat { var, amount } => self.raise_stat(e, var, amount),
            ClientIntent::RaiseTrait { var, amount } => self.raise_trait(e, var, amount),
            ClientIntent::BlockingPlayCancel => self.emit_self(e, ServerEvent::LoadConfirmed),
            other => debug!(map = %self.map_name, intent = ?other, "intent not handled by engine"),
        }
    }

    /// `CZ_NOTIFY_ACTORINIT`: the inventory lists and weights (on every one, repeats included);
    /// the first time also the `0x01d7` look before them, then into the grid, spawn to others,
    /// `0x099b`, others to self, the entry settings.
    fn actor_init(&mut self, e: Entity, now_ms: u64) {
        let entering = !self.world.in_grid[e.index()];
        if entering {
            self.send_weapon_look(e);
        }
        self.send_inventory(e);
        if !entering {
            return;
        }
        self.world.show(e);
        if let Some(unit) = self.appearance(e) {
            self.emit_aoi(e, true, ServerEvent::UnitSpawned { unit });
        }
        self.send_map_property(e);
        self.query_into_after(self.cell(e));
        let tick = now_ms as u32;
        for k in 0..self.aoi_after.len() {
            let other = self.aoi_after[k];
            if other == e {
                continue;
            }
            if let Some(event) = self.entry_event(other, tick) {
                self.emit_self(e, event);
            }
        }
        self.send_entry_settings(e);
        // Arrival cell: warps chain from here (ENG-WARP-05).
        self.touch_warps(e);
    }

    /// The entry lists in captured order: start, stackables (if any), non-stackables (if any),
    /// end, the empty equip-switch list, SP_WEIGHT, SP_MAXWEIGHT.
    fn send_inventory(&mut self, e: Entity) {
        let Some(inventory) = self.world.inventories.get(e) else {
            return;
        };
        let (normal, equip) = inventory.list_entries(&self.items);
        let (weight, max_weight) = (inventory.weight(), inventory.max_weight());
        self.emit_self(e, ServerEvent::InventoryStart);
        if !normal.is_empty() {
            self.emit_self(e, ServerEvent::InventoryNormalItems { items: normal });
        }
        self.send_worn_ammo(e);
        if !equip.is_empty() {
            self.emit_self(e, ServerEvent::InventoryEquipItems { items: equip });
        }
        self.emit_self(e, ServerEvent::InventoryEnd);
        self.emit_self(e, ServerEvent::EquipSwitchList);
        self.emit_weight(e, SP_WEIGHT, weight);
        self.emit_weight(e, SP_MAXWEIGHT, max_weight);
    }

    /// Creates the player entity, moves the spawn's inventory into its component and sets
    /// which entry settings its first actor init sends (`arrival`).
    pub(crate) fn spawn_player(
        &mut self,
        spawn: &mut CharacterSpawn,
        arrival: Arrival,
    ) -> Result<Entity, EngineError> {
        let e = self.world.spawn_player(spawn)?;
        if let Some(info) = self.world.players.get_mut(e) {
            info.connect_new = arrival == Arrival::Login;
            info.changemap = true;
        }
        let inventory = mem::take(&mut spawn.inventory);
        // The status sets the max weight right below.
        let inventory = PlayerInventory::new(inventory, spawn.char_id, 0, &self.items);
        let looks = inventory.gear(&self.items).unit_looks();
        self.world.inventories.insert(e, inventory);
        if let Some(info) = self.world.players.get_mut(e) {
            info.looks = looks;
        }
        self.init_status(e);
        Ok(e)
    }

    pub(crate) fn emit_weight(&mut self, e: Entity, var_id: u16, value: u32) {
        self.emit_self(
            e,
            ServerEvent::ParameterChanged {
                var_id,
                value: i32::try_from(value).unwrap_or(i32::MAX),
            },
        );
    }

    /// Adds an item (`item.amount` of it) to `e`'s inventory: `0x0b41` and SP_WEIGHT on
    /// success, the `0x0b41` failure form on failure. Returns the `0x0b41` result code.
    pub(crate) fn give_item_to(&mut self, e: Entity, item: &InventoryItem) -> u8 {
        match self.add_item(e, item) {
            Ok(()) => 0,
            Err(result) => {
                self.emit_self(e, ServerEvent::ItemAddFailed { result });
                result
            }
        }
    }

    /// rAthena `pc_additem`: on success sends `0x0b41` and SP_WEIGHT; on failure sends nothing
    /// and returns the result code (the caller decides on the failure ack).
    pub(crate) fn add_item(&mut self, e: Entity, item: &InventoryItem) -> Result<(), u8> {
        let Some(inventory) = self.world.inventories.get_mut(e) else {
            return Err(crate::inventory::ADD_INVALID);
        };
        let added = inventory.add(item, u32::from(item.amount), &self.items)?;
        let entry = inventory.pickup_entry(added.slot, added.amount, &self.items);
        let weight = inventory.weight();
        if let Some(item) = entry {
            self.emit_self(e, ServerEvent::ItemAdded { item });
        }
        self.emit_weight(e, SP_WEIGHT, weight);
        Ok(())
    }

    /// Vanishes player `e` to its AoI (excluding itself), drops its queued intents and frees
    /// the entity. Shared by logout (type 2) and cross-map warps (type 0).
    pub(crate) fn despawn_player(
        &mut self,
        e: Entity,
        char_id: CharId,
        vanish_type: u8,
    ) -> Result<(), EngineError> {
        if self.world.in_grid[e.index()] {
            let gid = self.gid(e);
            self.emit_aoi(e, true, ServerEvent::UnitVanished { gid, vanish_type });
        }
        // Intents queued by the old session must not reach a later spawn of this character.
        self.intents.retain(|i| i.char_id != Some(char_id));
        self.world.despawn(e)
    }

    /// Stops a walk in place: `ZC_STOPMOVE` to the AoI including the unit.
    fn stop_walking(&mut self, e: Entity) {
        if self.world.walking_paths.remove(e).is_some() {
            let (x, y) = self.cell(e);
            let gid = self.gid(e);
            self.emit_aoi(e, false, ServerEvent::UnitStopped { gid, x, y });
        }
    }

    fn sit(&mut self, e: Entity) {
        self.world.dead_sit[e.index()] = SITTING;
        self.emit_action(e, ACTION_SIT);
    }

    fn stand(&mut self, e: Entity) {
        self.world.dead_sit[e.index()] = STANDING;
        self.emit_action(e, ACTION_STAND);
    }

    fn emit_action(&mut self, e: Entity, action_type: u8) {
        let source_id = self.gid(e);
        self.emit_aoi(
            e,
            false,
            ServerEvent::EntityAction {
                source_id,
                target_id: 0,
                action_type,
                action_tick: 0,
            },
        );
    }

    fn name_request(&mut self, e: Entity, gid: u32) {
        let Some(target) = self.world.entity_by_gid(gid) else {
            return;
        };
        if !self.world.in_grid[target.index()] {
            return;
        }
        let (me, them) = (self.pos(e), self.pos(target));
        if me.x.abs_diff(them.x) > AREA_SIZE || me.y.abs_diff(them.y) > AREA_SIZE {
            return;
        }
        // Mobs answer like NPCs; mob_db `GroupId`/`Title` are not loaded yet.
        let npc_like = self
            .world
            .npcs
            .get(target)
            .map(|n| &n.name)
            .or_else(|| self.world.mobs.get(target).map(|m| &m.name));
        if let Some(name) = npc_like {
            let name = Arc::clone(name);
            self.emit_self(e, ServerEvent::AckNpcName { gid, name });
            return;
        }
        let Some(name) = self.world.players.get(target).map(|p| p.name.to_string()) else {
            return;
        };
        self.emit_self(
            e,
            ServerEvent::AckEntityName {
                gid,
                name,
                party_name: String::new(),
                guild_name: String::new(),
                position_name: String::new(),
                title_id: 0,
            },
        );
    }

    // ---------- movement ----------

    fn find_path(
        &mut self,
        from: (u16, u16),
        to: (u16, u16),
        max_steps: usize,
    ) -> Option<PathCells> {
        self.scratch
            .find_path(&self.walkability, from, to, max_steps)
            .map(PathCells::from_slice)
    }

    /// Starts (or restarts) a walk along `cells` from `from` at `start_ms`, and announces it.
    fn start_walk(&mut self, e: Entity, cells: &[(u16, u16)], from: (u16, u16), start_ms: u64) {
        let speed = self.world.speed[e.index()];
        let path = WalkingPath::new(cells, start_ms, speed, from);
        let dest = path.dest();
        self.world.walking_paths.insert(e, path);
        self.world.head_dir[e.index()] = 0;
        let tick = start_ms as u32;
        self.emit_self(
            e,
            ServerEvent::PlayerMoved {
                start: from,
                dest,
                move_start_tick: tick,
            },
        );
        if let Some(unit) = self.appearance(e) {
            self.emit_aoi(
                e,
                true,
                ServerEvent::UnitWalking {
                    unit,
                    start: from,
                    dest,
                    move_start_tick: tick,
                },
            );
        }
    }

    fn plan_moves(&mut self, now_ms: u64) {
        let mut moves = mem::take(&mut self.moves);
        for e in moves.drain(..) {
            if !self.world.entities.is_alive(e) {
                continue;
            }
            let Some(dest) = self.world.pending_move[e.index()].take() else {
                continue;
            };
            if self.world.dead_sit[e.index()] == SITTING {
                continue;
            }
            let from = self.cell(e);
            // rAthena checks `max_walk_path` after a successful search; same outcome.
            let Some(cells) = self.find_path(from, dest, MAX_WALK_PATH_CLICK) else {
                continue;
            };
            if let Some(path) = self.world.walking_paths.get_mut(e) {
                // Redirect: validated now, applied when the in-flight step ends.
                path.pending_target = Some(dest);
            } else if cells.is_empty() {
                self.emit_self(
                    e,
                    ServerEvent::PlayerMoved {
                        start: from,
                        dest: from,
                        move_start_tick: now_ms as u32,
                    },
                );
            } else {
                self.start_walk(e, &cells, from, now_ms);
            }
        }
        self.moves = moves;
    }

    fn advance_walkers(&mut self, now_ms: u64) {
        self.walkers.clear();
        self.walkers
            .extend_from_slice(self.world.walking_paths.entities());
        for k in 0..self.walkers.len() {
            let e = self.walkers[k];
            while let Some(path) = self.world.walking_paths.get(e) {
                if path.next_step_at > now_ms {
                    break;
                }
                let step_end = path.next_step_at;
                let Some(next) = path.next_cell() else {
                    self.world.walking_paths.remove(e);
                    break;
                };
                self.step(e, next, step_end);
                if self.world.players.contains(e) {
                    // A warp clears the walk (or despawns `e`), which ends this loop.
                    self.touch_warps(e);
                }
                let Some(path) = self.world.walking_paths.get_mut(e) else {
                    break;
                };
                if let Some(target) = path.pending_target.take() {
                    self.repath(e, target, step_end);
                } else if path.next_cell().is_none() {
                    self.world.walking_paths.remove(e);
                }
            }
        }
    }

    /// Completes one step into `next` (ending at `step_end`) and emits visibility changes.
    fn step(&mut self, e: Entity, next: (u16, u16), step_end: u64) {
        let i = e.index();
        let old = self.cell(e);
        let visible = self.world.in_grid[i];
        if visible {
            self.world.grid.query_radius(
                old.0,
                old.1,
                AREA_SIZE + 1,
                &self.world.positions,
                &mut self.step_candidates,
            );
            self.world.grid.move_entity(e, old, next);
        }
        self.world.positions[i] = MapPosition {
            x: next.0,
            y: next.1,
            dir: step_dir(old, next),
        };
        let speed = self.world.speed[i];
        if let Some(path) = self.world.walking_paths.get_mut(e) {
            path.advance(speed, next);
        }
        if !visible {
            return;
        }
        // A unit whose walk ends on this step is shown standing.
        let finished = self
            .world
            .walking_paths
            .get(e)
            .is_some_and(|p| p.next_cell().is_none() && p.pending_target.is_none());
        step_visibility(
            e,
            old,
            next,
            &self.step_candidates,
            &self.world.positions,
            &mut self.left,
            &mut self.entered,
        );
        let gid = self.gid(e);
        for k in 0..self.left.len() {
            let other = self.left[k];
            let other_gid = self.gid(other);
            let gone = if self.is_floor_item(other) {
                ServerEvent::FloorItemVanished { id: other_gid }
            } else {
                ServerEvent::UnitVanished {
                    gid: other_gid,
                    vanish_type: VANISH_OUT_OF_SIGHT,
                }
            };
            self.emit_self(e, gone);
            // NPCs see nothing; skip building events for them.
            if !self.world.players.contains(other) {
                continue;
            }
            self.emit_self(
                other,
                ServerEvent::UnitVanished {
                    gid,
                    vanish_type: VANISH_OUT_OF_SIGHT,
                },
            );
        }
        let tick = step_end as u32;
        // Only players see anything: skip building entry events for a mob mover.
        let mover_sees = self.world.players.contains(e);
        for k in 0..self.entered.len() {
            let other = self.entered[k];
            if mover_sees {
                if let Some(event) = self.entry_event(other, tick) {
                    self.emit_self(e, event);
                }
            }
            if !self.world.players.contains(other) {
                continue;
            }
            let about_mover = if finished {
                self.appearance(e)
                    .map(|unit| ServerEvent::UnitStanding { unit })
            } else {
                self.entry_event(e, tick)
            };
            if let Some(event) = about_mover {
                self.emit_self(other, event);
            }
        }
    }

    /// Applies a pending redirect from the cell just reached. No `max_walk_path` check here.
    fn repath(&mut self, e: Entity, target: (u16, u16), step_end: u64) {
        let from = self.cell(e);
        match self.find_path(from, target, MAX_WALKPATH) {
            None => self.stop_walking(e),
            Some(cells) if cells.is_empty() => {
                // Redirected onto the reached cell: the mover resyncs, observers (who were told
                // about the old destination) get a stop.
                self.world.walking_paths.remove(e);
                self.emit_self(
                    e,
                    ServerEvent::PlayerMoved {
                        start: from,
                        dest: from,
                        move_start_tick: step_end as u32,
                    },
                );
                let gid = self.gid(e);
                self.emit_aoi(
                    e,
                    true,
                    ServerEvent::UnitStopped {
                        gid,
                        x: from.0,
                        y: from.1,
                    },
                );
            }
            Some(cells) => self.start_walk(e, &cells, from, step_end),
        }
    }
}

/// Lifecycle and tick API driven by the map-server actor. All times are the actor's monotonic
/// `now_ms`; wire ticks are `now_ms as u32`.
impl MapInstance {
    pub fn map_name(&self) -> &str {
        &self.map_name
    }

    /// Creates the entity for a login (not yet visible; it is announced on `NotifyActorInit`) and
    /// returns what the player gets before its first `0x007d`, to send after `ZC_ACCEPT_ENTER` and
    /// the friends list: the MOTD, `0x0091` to its own cell and the login status packets.
    /// A duplicate char_id returns `Err(CharacterAlreadyEntered)`.
    pub fn spawn_character(
        &mut self,
        mut spawn: CharacterSpawn,
        _now_ms: u64,
    ) -> Result<Vec<ScopedServerEvent>, EngineError> {
        let (x, y) = (spawn.pos.x, spawn.pos.y);
        if x >= self.walkability.width() || y >= self.walkability.height() {
            return Err(EngineError::InvalidCell { x, y });
        }
        let e = self.spawn_player(&mut spawn, Arrival::Login)?;
        let queued = self.events.len();
        self.send_login_burst(e);
        Ok(self.events.split_off(queued))
    }

    /// Adds `item.amount` of `item` to the character's inventory and sends the `0x0b41` result
    /// (and SP_WEIGHT on success). Returns the `0x0b41` result code: 0 on success.
    pub fn give_item(&mut self, char_id: CharId, item: &InventoryItem) -> Result<u8, EngineError> {
        let e = self
            .world
            .entity_by_char(char_id)
            .ok_or(EngineError::UnknownCharacter(char_id))?;
        Ok(self.give_item_to(e, item))
    }

    /// A copy of what a save writes: the inventory, the char-select looks of the worn gear and
    /// the progress.
    pub fn play_snapshot(&self, char_id: CharId) -> Option<PlaySnapshot> {
        let progress = self.progress_snapshot(char_id)?;
        let e = self.world.entity_by_char(char_id)?;
        self.world.inventories.get(e).map(|inventory| PlaySnapshot {
            inventory: inventory.snapshot(),
            looks: inventory.gear(&self.items).character_looks(),
            progress,
        })
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

    /// Creates the map's static NPCs (spec ENG-NPC-01). Emits nothing: no player is on the map
    /// yet, and each one sees the NPCs when it enters or walks into range. Entries outside the
    /// grid are skipped with a warning. Returns how many were created.
    pub fn spawn_npcs(&mut self, npcs: &MapNpcs) -> usize {
        let mut created = 0;
        for npc in &npcs.npcs {
            match self.spawn_npc(npc) {
                Ok(()) => created += 1,
                Err(err) => warn!(
                    map = %self.map_name,
                    npc = %npc.name,
                    source = %npc.source,
                    "NPC not spawned: {err}"
                ),
            }
        }
        created
    }

    fn spawn_npc(&mut self, npc: &NpcSpawn) -> Result<(), EngineError> {
        if npc.x >= self.walkability.width() || npc.y >= self.walkability.height() {
            return Err(EngineError::InvalidCell { x: npc.x, y: npc.y });
        }
        let gid = self.next_npc_id;
        let object_type = if npc.sprite_is_mob {
            UnitObjectType::MobSpriteNpc
        } else {
            UnitObjectType::Npc
        };
        let effect_state = match npc.state {
            NpcState::Hidden => OPTION_HIDE,
            NpcState::Cloaked => OPTION_CLOAK,
            NpcState::Normal | NpcState::Disabled => 0,
        };
        // A warp without a (non-negative) span has no touch area, like rAthena's `npc_setcells`.
        let warp = match (&npc.kind, npc.trigger) {
            (NpcKind::Warp(dest), Some(half_extents)) => {
                let dest_map = gat_map_name(&dest.map);
                Some(WarpTrigger {
                    half_extents,
                    same_map: *dest_map == *self.gat_name,
                    dest_map: Arc::from(dest_map),
                    dest: (dest.x, dest.y),
                    trigger_while_hidden: dest.trigger_while_hidden,
                })
            }
            _ => None,
        };
        let info = NpcInfo {
            gid,
            name: Arc::from(npc.name.as_str()),
            object_type,
            // -1 is never sent, so its wire value doesn't matter.
            sprite: u16::try_from(npc.sprite).unwrap_or(0),
            effect_state,
            state: npc.state,
        };
        let pos = MapPosition {
            x: npc.x,
            y: npc.y,
            dir: npc.dir,
        };
        let area = warp.as_ref().map(|w| w.half_extents);
        let e = self.world.spawn_npc(info, pos, NPC_SPEED, warp)?;
        if let Some(half_extents) = area {
            self.mark_warp_cells((npc.x, npc.y), half_extents);
        }
        self.next_npc_id = self.next_npc_id.wrapping_add(1);
        let never_sent = npc.sprite == FAKE_NPC_SPRITE
            || npc.sprite == INVISIBLE_SPRITE
            || npc.state == NpcState::Disabled;
        if !never_sent {
            self.world.show(e);
        }
        Ok(())
    }

    /// Changes one cell at runtime (rAthena `setcell`; later Ice Wall and similar). Copies the
    /// grid first if other instances share it. Paths already planned are not re-checked until
    /// their next repath. Returns `false` when out of bounds.
    pub fn set_cell(&mut self, x: u16, y: u16, cell: CellType) -> bool {
        Arc::make_mut(&mut self.walkability).set(x, y, cell)
    }

    /// Queues an intent for the next `step_tick`. `EnterMap` is not accepted here.
    pub fn push_intent(&mut self, intent: SessionIntent) {
        if matches!(intent.intent, ClientIntent::EnterMap { .. }) {
            debug!(map = %self.map_name, "EnterMap is handled by spawn_character, not intents");
            return;
        }
        self.intents.push(intent);
    }

    /// Runs one tick and appends its events to `out` (the caller reuses the buffer).
    /// Bad per-intent input is logged and dropped; `Err` only for engine-wide faults.
    pub fn step_tick(
        &mut self,
        now_ms: u64,
        out: &mut Vec<ScopedServerEvent>,
    ) -> Result<(), EngineError> {
        self.expire_floor_items(now_ms);
        // Steps that ended before `now` complete first, so this tick's clicks see them.
        self.advance_walkers(now_ms);
        let mut batch = mem::take(&mut self.intents);
        for intent in batch.drain(..) {
            self.handle_intent(intent, now_ms);
        }
        self.intents = batch;
        self.plan_moves(now_ms);
        // Stages 3 and 4: one recalc per dirty player, after every mutation of the tick.
        self.run_stat_pass();
        out.append(&mut self.events);
        Ok(())
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

    /// Removes the entity and emits `UnitVanished{type 2}` to the AoI excluding the leaver.
    pub fn remove_character(
        &mut self,
        char_id: CharId,
    ) -> Result<Vec<ScopedServerEvent>, EngineError> {
        let e = self
            .world
            .entity_by_char(char_id)
            .ok_or(EngineError::UnknownCharacter(char_id))?;
        let start = self.events.len();
        let result = self.despawn_player(e, char_id, VANISH_LOGGED_OUT);
        let events = self.events.split_off(start);
        result.map(|()| events)
    }
}

#[cfg(test)]
mod npc_tests {
    use super::*;
    use ro_db::WarpDest;

    /// ENG-NPC-05: a warp keeps its trigger area and destination for map change.
    #[test]
    fn eng_npc_05_warp_data_kept() {
        let grid = Arc::new(MapGrid::all_walkable(50, 50).expect("valid dims"));
        let mut engine = MapInstance::new("prontera.gat", grid);
        let dest = WarpDest {
            map: "geffen".into(),
            x: 119,
            y: 59,
            trigger_while_hidden: true,
        };
        let warp = NpcSpawn {
            name: "prt001".into(),
            unique_name: "prt001".into(),
            x: 10,
            y: 12,
            dir: 0,
            sprite: 45,
            sprite_is_mob: false,
            state: NpcState::Normal,
            trigger: Some((2, 3)),
            duplicate_of: None,
            kind: NpcKind::Warp(dest.clone()),
            source: "t:1".into(),
        };
        assert_eq!(engine.spawn_npcs(&MapNpcs { npcs: vec![warp] }), 1);
        let e = engine
            .world
            .entity_by_gid(START_NPC_ID)
            .expect("warp entity");
        assert_eq!(
            engine.world.warps.get(e),
            Some(&WarpTrigger {
                half_extents: (2, 3),
                dest_map: Arc::from("geffen.gat"),
                dest: (dest.x, dest.y),
                trigger_while_hidden: true,
                same_map: false,
            })
        );
        assert_eq!(engine.world.kinds[e.index()], crate::world::EntityKind::Npc);
    }

    /// Disabled warps keep their state for the trigger check; a warp without a span gets no
    /// trigger at all.
    #[test]
    fn eng_npc_05_disabled_and_spanless_warps() {
        let grid = Arc::new(MapGrid::all_walkable(50, 50).expect("valid dims"));
        let mut engine = MapInstance::new("prontera.gat", grid);
        let warp = |name: &str, state, trigger| NpcSpawn {
            name: name.into(),
            unique_name: name.into(),
            x: 5,
            y: 5,
            dir: 0,
            sprite: 45,
            sprite_is_mob: false,
            state,
            trigger,
            duplicate_of: None,
            kind: NpcKind::Warp(WarpDest {
                map: "geffen".into(),
                x: 1,
                y: 1,
                trigger_while_hidden: false,
            }),
            source: "t:1".into(),
        };
        let npcs = vec![
            warp("off", NpcState::Disabled, Some((1, 1))),
            warp("nospan", NpcState::Normal, None),
        ];
        assert_eq!(engine.spawn_npcs(&MapNpcs { npcs }), 2);
        let off = engine
            .world
            .entity_by_gid(START_NPC_ID)
            .expect("disabled warp");
        assert_eq!(
            engine.world.npcs.get(off).map(|n| n.state),
            Some(NpcState::Disabled)
        );
        assert!(engine.world.warps.contains(off));
        assert!(!engine.world.in_grid[off.index()], "disabled is never sent");
        let nospan = engine
            .world
            .entity_by_gid(START_NPC_ID + 1)
            .expect("spanless warp");
        assert!(!engine.world.warps.contains(nospan));
    }
}
