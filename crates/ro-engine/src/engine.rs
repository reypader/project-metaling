//! `MapInstance`: one map's simulation, and its lifecycle/tick API.
//!
//! `step_tick` order: drain intents FIFO (the last move per entity per tick wins) -> plan
//! paths -> advance every due step (AoI diff per step, pending redirect repath at step end)
//! -> return the events. Rules: spec `specs/engine-foundation/spec.md` §0.

use std::mem;
use std::sync::Arc;

use ro_core::{
    BroadcastScope, CharId, CharacterSpawn, ClientIntent, MapPosition, ScopedServerEvent,
    ServerEvent, SessionIntent, UnitAppearance,
};
use ro_db::MapWalkability;
use smallvec::SmallVec;
use tracing::debug;

use crate::entity::Entity;
use crate::error::EngineError;
use crate::movement::WalkingPath;
use crate::path::{AStarScratchBuffer, MAX_WALKPATH, MAX_WALK_PATH_CLICK};
use crate::spatial::{step_visibility, AREA_SIZE};

/// `CZ_REQUEST_ACT` attack types; every other action stops a walk first (rAthena
/// `clif_parse_ActionRequest_sub`).
const ACTION_ATTACK: u8 = 0;
const ACTION_ATTACK_REPEAT: u8 = 7;
use crate::world::World;

/// Default entity capacity of one map instance.
pub const DEFAULT_ENTITY_CAPACITY: u32 = 4096;

/// `ZC_PAR_CHANGE` var ids sent at map entry (rAthena `SP_WEIGHT`, `SP_MAXWEIGHT`).
const SP_WEIGHT: u16 = 24;
const SP_MAXWEIGHT: u16 = 25;
/// Placeholder weights in 0.1 units until inventory/stats exist: empty bag, and the max weight
/// rAthena gives a STR 1 novice (capture `login_sequence-rathena.log`).
const PLACEHOLDER_WEIGHT: i32 = 0;
const PLACEHOLDER_MAX_WEIGHT: i32 = 20_300;

/// `CZ_REQUEST_ACT` action types.
const ACTION_SIT: u8 = 2;
const ACTION_STAND: u8 = 3;
/// rAthena `dead_sit` values.
const STANDING: u8 = 0;
const SITTING: u8 = 2;

/// `ZC_NOTIFY_VANISH` types.
const VANISH_OUT_OF_SIGHT: u8 = 0;
const VANISH_LOGGED_OUT: u8 = 2;

type PathCells = SmallVec<[(u16, u16); MAX_WALKPATH]>;

/// One map's world plus the reusable buffers its tick needs.
pub struct MapInstance {
    map_name: String,
    world: World,
    walkability: Arc<dyn MapWalkability>,
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
    events: Vec<ScopedServerEvent>,
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
    pub fn new(map_name: impl Into<String>, walkability: Arc<dyn MapWalkability>) -> Self {
        Self::with_capacity(map_name, walkability, DEFAULT_ENTITY_CAPACITY)
    }

    pub fn with_capacity(
        map_name: impl Into<String>,
        walkability: Arc<dyn MapWalkability>,
        capacity: u32,
    ) -> Self {
        let world = World::new(walkability.width(), walkability.height(), capacity);
        Self {
            map_name: map_name.into(),
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
        }
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

    fn gid(&self, e: Entity) -> u32 {
        self.world.players.get(e).map_or(0, |p| p.account_id.0)
    }

    fn char_of(&self, e: Entity) -> Option<CharId> {
        self.world.players.get(e).map(|p| p.char_id)
    }

    fn pos(&self, e: Entity) -> MapPosition {
        self.world.positions[e.index()]
    }

    fn cell(&self, e: Entity) -> (u16, u16) {
        let p = self.pos(e);
        (p.x, p.y)
    }

    fn appearance(&self, e: Entity) -> Option<UnitAppearance> {
        let info = self.world.players.get(e)?;
        let i = e.index();
        Some(UnitAppearance {
            gid: info.account_id.0,
            char_id: Some(info.char_id),
            name: info.name.clone(),
            job: info.job,
            sex: info.sex,
            hair_style: info.hair_style,
            hair_color: info.hair_color,
            head_dir: self.world.head_dir[i],
            pos: self.world.positions[i],
            speed: self.world.speed[i],
            dead_sit: self.world.dead_sit[i],
            clevel: info.clevel,
        })
    }

    /// Characters that see `e` right now (Chebyshev 14, visible only), sorted, optionally
    /// without `e` itself. Resolved at emission like rAthena's `clif_send(AREA)`.
    fn aoi_recipients(&mut self, e: Entity, exclude_self: bool) -> Vec<CharId> {
        let (x, y) = self.cell(e);
        self.world
            .grid
            .query_radius(x, y, AREA_SIZE, &self.world.positions, &mut self.recipients);
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

    fn emit_self(&mut self, e: Entity, event: ServerEvent) {
        if let Some(char_id) = self.char_of(e) {
            self.events.push(ScopedServerEvent::new(
                BroadcastScope::SelfOnly(char_id),
                event,
            ));
        }
    }

    fn emit_aoi(&mut self, e: Entity, exclude_self: bool, event: ServerEvent) {
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
            other => debug!(map = %self.map_name, intent = ?other, "intent not handled by engine"),
        }
    }

    /// `CZ_NOTIFY_ACTORINIT`: self stats, then into the grid, spawn to others, others to self.
    fn actor_init(&mut self, e: Entity, now_ms: u64) {
        if self.world.in_grid[e.index()] {
            return;
        }
        self.emit_self(
            e,
            ServerEvent::ParameterChanged {
                var_id: SP_WEIGHT,
                value: PLACEHOLDER_WEIGHT,
            },
        );
        self.emit_self(
            e,
            ServerEvent::ParameterChanged {
                var_id: SP_MAXWEIGHT,
                value: PLACEHOLDER_MAX_WEIGHT,
            },
        );
        self.world.show(e);
        if let Some(unit) = self.appearance(e) {
            self.emit_aoi(e, true, ServerEvent::UnitSpawned { unit });
        }
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
        let Some(name) = self.world.players.get(target).map(|p| p.name.clone()) else {
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
            .find_path(self.walkability.as_ref(), from, to, max_steps)
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
            self.emit_self(
                e,
                ServerEvent::UnitVanished {
                    gid: other_gid,
                    vanish_type: VANISH_OUT_OF_SIGHT,
                },
            );
            self.emit_self(
                other,
                ServerEvent::UnitVanished {
                    gid,
                    vanish_type: VANISH_OUT_OF_SIGHT,
                },
            );
        }
        let tick = step_end as u32;
        for k in 0..self.entered.len() {
            let other = self.entered[k];
            if let Some(event) = self.entry_event(other, tick) {
                self.emit_self(e, event);
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

    /// Creates the entity (not yet visible; it is announced on `NotifyActorInit`).
    /// A duplicate char_id returns `Err(CharacterAlreadyEntered)`.
    pub fn spawn_character(
        &mut self,
        spawn: CharacterSpawn,
        _now_ms: u64,
    ) -> Result<Vec<ScopedServerEvent>, EngineError> {
        let (x, y) = (spawn.pos.x, spawn.pos.y);
        if x >= self.walkability.width() || y >= self.walkability.height() {
            return Err(EngineError::InvalidCell { x, y });
        }
        self.world.spawn_player(&spawn)?;
        Ok(Vec::new())
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
        // Steps that ended before `now` complete first, so this tick's clicks see them.
        self.advance_walkers(now_ms);
        let mut batch = mem::take(&mut self.intents);
        for intent in batch.drain(..) {
            self.handle_intent(intent, now_ms);
        }
        self.intents = batch;
        self.plan_moves(now_ms);
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
        let mut events = Vec::new();
        if self.world.in_grid[e.index()] {
            let chars = self.aoi_recipients(e, true);
            if !chars.is_empty() {
                events.push(ScopedServerEvent::new(
                    BroadcastScope::Characters(chars),
                    ServerEvent::UnitVanished {
                        gid: self.gid(e),
                        vanish_type: VANISH_LOGGED_OUT,
                    },
                ));
            }
        }
        // Intents queued by the old session must not reach a later spawn of this character.
        self.intents.retain(|i| i.char_id != Some(char_id));
        self.world.despawn(e)?;
        Ok(events)
    }
}
