//! `MapInstance`: one map's simulation, and its lifecycle/tick API.
//!
//! `step_tick` order: expire floor items -> advance every due step (AoI diff per step, pending
//! redirect repath at step end) -> drain intents FIFO (the last move per entity per tick wins)
//! -> plan paths -> the stat pass (recalc of dirty players, then their follow-ups) -> return the
//! events. Rules: spec `specs/engine-foundation/spec.md` §0, `specs/stats/spec.md` §2.1.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use ro_core::{gat_map_name, CharId, ScopedServerEvent, SessionIntent};
use ro_db::{ItemDb, JobDb, MapGrid, MobDb, MobId, RefineDb, SkillDb, SkillTreeDb};
use ro_mechanics::BattleConfig;
use ro_scripting::EffectList;
use rustc_hash::FxHashMap;

use crate::cast::SkillScripts;
use crate::combat::CombatTimers;
use crate::ecs::{Entity, SplitMix64};
use crate::item::MIN_FLOOR_ID;
use crate::mob::{MobStats, SpawnGroup};
use crate::movement::AStarScratchBuffer;
use crate::warp::{CellBits, MapTransfer};
use crate::world::World;

mod emit;
mod intents;
mod npc;
mod player;
mod queries;
mod registries;
mod tick;

pub(crate) use emit::chat_line;
pub use registries::Registries;

/// Default entity capacity of one map instance.
/// Capacity is only a limit: the columns grow as entities are created. Floor items count too.
pub const DEFAULT_ENTITY_CAPACITY: u32 = 16_384;

/// rAthena `START_NPC_NUM`: first NPC unit id. Above every account id (< 100,000,000).
pub const START_NPC_ID: u32 = 110_000_000;

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
    pub(crate) moves: Vec<Entity>,
    /// Scratch for resolving AoI recipients at emission.
    recipients: Vec<Entity>,
    pub(crate) walkers: Vec<Entity>,
    /// Units near a stepping mover (radius `AREA_SIZE + 1` around its old cell).
    pub(crate) step_candidates: Vec<Entity>,
    pub(crate) aoi_after: Vec<Entity>,
    pub(crate) left: Vec<Entity>,
    pub(crate) entered: Vec<Entity>,
    pub(crate) scratch: AStarScratchBuffer,
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
    /// Swings, step actions and delayed hits, soonest first (spec `specs/combat/spec.md` §2.4).
    pub(crate) combat_timers: CombatTimers,
    /// Battle values per mob id, computed at the first spawn of that id.
    pub(crate) mob_stats: FxHashMap<MobId, Arc<MobStats>>,
    /// Global registries shared by every instance (empty until `set_registries`).
    pub(crate) items: Arc<ItemDb>,
    pub(crate) mob_db: Arc<MobDb>,
    pub(crate) jobs: Arc<JobDb>,
    pub(crate) refine: Arc<RefineDb>,
    pub(crate) skills: Arc<SkillDb>,
    pub(crate) skill_tree: Arc<SkillTreeDb>,
    pub(crate) battle: Arc<BattleConfig>,
    pub(crate) scripts: SkillScripts,
    /// The effects of the cast being applied; reused so a cast doesn't allocate.
    pub(crate) effect_buf: EffectList,
    /// Players the tick's stat pass owes a recalc or follow-ups, in marking order (each once;
    /// `PlayerInfo::stat_flags` has `LISTED`). Reused across ticks.
    pub(crate) stat_queue: Vec<Entity>,
    /// Characters a flow asked to save right away (`@job`); the map-server drains them.
    pub(crate) save_requests: Vec<CharId>,
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
            combat_timers: CombatTimers::default(),
            mob_stats: FxHashMap::default(),
            items: Arc::new(ItemDb::default()),
            mob_db: Arc::new(MobDb::default()),
            jobs: Arc::new(JobDb::default()),
            refine: Arc::new(RefineDb::default()),
            skills: Arc::new(SkillDb::default()),
            skill_tree: Arc::new(SkillTreeDb::default()),
            battle: Arc::new(BattleConfig::default()),
            scripts: SkillScripts::default(),
            effect_buf: EffectList::new(),
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
        self.skills = registries.skills;
        self.skill_tree = registries.skill_tree;
        self.battle = registries.battle;
        self.scripts = registries.scripts;
        // Built from the old mob db and battle config.
        self.mob_stats.clear();
    }
}
