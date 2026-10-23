use std::{
    collections::{HashMap, HashSet},
    mem,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, MutexGuard, OnceLock, RwLock,
    },
    time::{Duration, Instant},
};

use bytes::{Bytes, BytesMut};
use ro_core::{
    bare_map_name, AccountId, BroadcastScope, CharId, CharacterSpawn, CharacterSummary,
    ClientIntent, MapPosition, ScopedServerEvent, ServerEvent, SessionId, SessionIntent,
    TARGET_PACKETVER,
};
use ro_db::{MapGrid, MapNpcs};
use ro_engine::{EngineError, MapInstance};
use ro_packets::{EventEncoder, Phase0EventEncoder};
use ro_persistence::CharacterRepository;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

/// Engine tick period (BIN-SRV-01).
pub const TICK_INTERVAL: Duration = Duration::from_millis(20);
/// Side of the all-walkable grid used when a map's `.gat` is unavailable (BIN-CFG-01).
pub const FALLBACK_MAP_SIZE: u16 = 512;
/// Most commands an actor handles before it lets a tick run.
const MAX_COMMANDS_PER_WAKE: usize = 256;

/// State shared by the service handle and every map actor.
#[derive(Default)]
struct SharedState {
    /// Characters claimed by an entering or joined session, on any map.
    online: Mutex<HashSet<CharId>>,
    /// Last map/position of characters whose leave is still being persisted.
    recent_positions: Mutex<HashMap<CharId, (String, MapPosition)>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl SharedState {
    /// Claims `char_id` for one session; false if it is already online somewhere.
    fn claim(&self, char_id: CharId) -> bool {
        lock(&self.online).insert(char_id)
    }

    fn release(&self, char_id: CharId) {
        lock(&self.online).remove(&char_id);
    }
}

/// Static data a map world starts with.
#[derive(Clone)]
pub struct MapData {
    pub grid: Arc<MapGrid>,
    pub npcs: Arc<MapNpcs>,
}

/// Builds a map's static data from its name (`prontera.gat`).
pub type MapDataLoader = Arc<dyn Fn(&str) -> MapData + Send + Sync>;

/// Loads `<dir>/<map_name>` as a `.gat`. A missing directory, missing file, or bad file logs a
/// warning and falls back to an all-walkable `FALLBACK_MAP_SIZE` square grid.
pub fn load_walkability(map_data_dir: Option<&Path>, map_name: &str) -> Arc<MapGrid> {
    if let Some(dir) = map_data_dir {
        match MapGrid::load_gat(&dir.join(map_name)) {
            Ok(grid) => return Arc::new(grid),
            Err(err) => warn!("Map '{}': {}; using fallback grid", map_name, err),
        }
    } else {
        warn!(
            "Map '{}': MAP_DATA_DIR not set; using fallback grid",
            map_name
        );
    }
    fallback_grid()
}

/// One fallback grid shared by every map without a `.gat`; `MapInstance::set_cell` copies
/// it before changing a cell, so maps never see each other's changes.
fn fallback_grid() -> Arc<MapGrid> {
    static FALLBACK: OnceLock<Arc<MapGrid>> = OnceLock::new();
    let grid = FALLBACK.get_or_init(|| {
        let grid =
            MapGrid::all_walkable(FALLBACK_MAP_SIZE, FALLBACK_MAP_SIZE).unwrap_or_else(|err| {
                // Unreachable: the fallback dimensions are non-zero constants.
                error!("fallback grid: {}", err);
                MapGrid::blocked()
            });
        Arc::new(grid)
    });
    Arc::clone(grid)
}

/// Loads `<dir>/<map>.yml` (map name without `.gat`). No directory or no file means no NPCs;
/// a bad file logs a warning and also means no NPCs (BIN-NPC-01).
pub fn load_npcs(npc_data_dir: Option<&Path>, map_name: &str) -> Arc<MapNpcs> {
    let Some(dir) = npc_data_dir else {
        return Arc::new(MapNpcs::default());
    };
    let stem = bare_map_name(map_name);
    match MapNpcs::load_yaml(&dir.join(format!("{stem}.yml"))) {
        Ok(Some(npcs)) => Arc::new(npcs),
        Ok(None) => Arc::new(MapNpcs::default()),
        Err(err) => {
            warn!("Map '{}': {}; spawning no NPCs", map_name, err);
            Arc::new(MapNpcs::default())
        }
    }
}

/// Loader reading `.gat` files from `map_data_dir` and NPC lists from `npc_data_dir`.
pub fn map_data_loader(
    map_data_dir: Option<PathBuf>,
    npc_data_dir: Option<PathBuf>,
) -> MapDataLoader {
    Arc::new(move |map_name: &str| MapData {
        grid: load_walkability(map_data_dir.as_deref(), map_name),
        npcs: load_npcs(npc_data_dir.as_deref(), map_name),
    })
}

/// Commands sent to a long-lived `MapWorldActor`.
pub enum WorldCommand {
    JoinSession {
        session_id: SessionId,
        account_id: AccountId,
        character: CharacterSummary,
        spawn_pos: MapPosition,
        outbound_tx: mpsc::Sender<Bytes>,
    },
    PlayerIntent(SessionIntent),
    LeaveSession {
        session_id: SessionId,
    },
    CharacterPosition {
        char_id: CharId,
        reply: oneshot::Sender<Option<MapPosition>>,
    },
    AoiCharacters {
        center: MapPosition,
        reply: oneshot::Sender<Vec<CharId>>,
    },
}

/// Reserved transfers: char -> (map name, optional entry position).
type PendingTransfers = HashMap<CharId, (String, Option<MapPosition>)>;

/// Shared coordinator managing long-lived per-map `MapWorldActor` tasks and transfer reservations.
#[derive(Clone)]
pub struct MapServiceHandle {
    next_session_id: Arc<AtomicU64>,
    char_repo: Arc<dyn CharacterRepository>,
    worlds: Arc<RwLock<HashMap<String, mpsc::Sender<WorldCommand>>>>,
    pending_transfers: Arc<RwLock<PendingTransfers>>,
    loader: MapDataLoader,
    shared: Arc<SharedState>,
    /// Process clock origin; every map's `now_ms` counts from here.
    start_time: Instant,
}

pub const MAX_HOSTED_WORLDS: usize = 128;
pub const MAX_PENDING_TRANSFERS: usize = 1024;

pub fn is_valid_map_name(map_name: &str) -> bool {
    if map_name.is_empty() || map_name.len() > 32 {
        return false;
    }
    if !map_name.ends_with(".gat") {
        return false;
    }
    let stem = &map_name[..map_name.len() - 4];
    !stem.is_empty()
        && stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

impl MapServiceHandle {
    pub fn new(char_repo: Arc<dyn CharacterRepository>, loader: MapDataLoader) -> Self {
        let handle = Self {
            next_session_id: Arc::new(AtomicU64::new(1)),
            char_repo,
            worlds: Arc::new(RwLock::new(HashMap::new())),
            pending_transfers: Arc::new(RwLock::new(HashMap::new())),
            loader,
            shared: Arc::new(SharedState::default()),
            start_time: Instant::now(),
        };
        let _ = handle.get_or_spawn_world("prontera.gat");
        handle
    }

    pub fn char_repo(&self) -> &Arc<dyn CharacterRepository> {
        &self.char_repo
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    pub fn allocate_session_id(&self) -> SessionId {
        SessionId(self.next_session_id.fetch_add(1, Ordering::Relaxed))
    }

    pub fn accept_transfer(
        &self,
        char_id: CharId,
        destination_map: String,
        destination_pos: Option<MapPosition>,
    ) -> bool {
        if !is_valid_map_name(&destination_map) {
            warn!(
                "MapService rejected invalid destination map '{}' for character id {}",
                destination_map, char_id.0
            );
            return false;
        }

        let worlds_guard = match self.worlds.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!(
                    "Map worlds read lock poisoned during accept_transfer. Recovering inner state."
                );
                poisoned.into_inner()
            }
        };
        let world_exists = worlds_guard.contains_key(&destination_map);
        let world_count = worlds_guard.len();
        drop(worlds_guard);

        if !world_exists && world_count >= MAX_HOSTED_WORLDS {
            warn!(
                "MapService rejected transfer to map '{}' for character id {}: maximum world count ({}) reached.",
                destination_map, char_id.0, MAX_HOSTED_WORLDS
            );
            return false;
        }

        info!(
            "MapService confirmed transfer reservation for character id {} to map {}",
            char_id.0, destination_map
        );
        let _ = self.get_or_spawn_world(&destination_map);

        let mut transfers = match self.pending_transfers.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Pending transfers write lock poisoned during accept_transfer. Recovering inner state.");
                poisoned.into_inner()
            }
        };
        if !transfers.contains_key(&char_id) && transfers.len() >= MAX_PENDING_TRANSFERS {
            warn!(
                "MapService rejected transfer for character id {}: pending transfers limit ({}) reached.",
                char_id.0, MAX_PENDING_TRANSFERS
            );
            return false;
        }
        transfers.insert(char_id, (destination_map.clone(), destination_pos));

        if let Some(pos) = destination_pos {
            let repo = Arc::clone(&self.char_repo);
            let dest_map = destination_map.clone();
            tokio::spawn(async move {
                if let Err(err) = repo.update_position(char_id, &dest_map, pos).await {
                    error!(
                        "Failed to update position on transfer for char {}: {}",
                        char_id.0, err
                    );
                }
            });
        }

        true
    }

    /// Resolves where an entering character goes: a pending transfer reservation wins over the
    /// stored map/position (or a just-left position still being persisted). `None` (unknown
    /// character, character of another account, character already online, or a bad stored map
    /// name) means the session must close without sending anything. On success the character is
    /// claimed; the map actor releases it when the session leaves or the join is rejected, and
    /// the caller must call `release_character` if it can't hand the join to the actor.
    pub async fn resolve_entry(
        &self,
        account_id: AccountId,
        char_id: CharId,
    ) -> Option<(mpsc::Sender<WorldCommand>, CharacterSummary, MapPosition)> {
        let character = match self.char_repo.find_by_id(char_id).await {
            Ok(Some(character)) => character,
            Ok(None) => {
                warn!("Map entry rejected: unknown character {}", char_id.0);
                return None;
            }
            Err(err) => {
                error!(
                    "Map entry rejected: lookup of character {} failed: {}",
                    char_id.0, err
                );
                return None;
            }
        };
        if character.account_id != account_id {
            warn!(
                "Map entry rejected: character {} belongs to account {}, not {}",
                char_id.0, character.account_id.0, account_id.0
            );
            return None;
        }

        let pending = self.read_transfers().get(&char_id).cloned();
        let recent = lock(&self.shared.recent_positions).get(&char_id).cloned();
        let (map_name, pos) = match (pending, recent) {
            (Some((map, Some(pos))), _) => (map, pos),
            (Some((map, None)), _) => (map, character.last_pos),
            (None, Some((map, pos))) => (map, pos),
            (None, None) => (character.last_map.clone(), character.last_pos),
        };
        if !is_valid_map_name(&map_name) {
            warn!(
                "Map entry rejected: character {} has invalid map '{}'",
                char_id.0, map_name
            );
            return None;
        }
        if !self.shared.claim(char_id) {
            warn!(
                "Map entry rejected: character {} is already online",
                char_id.0
            );
            return None;
        }
        // Only a successful entry uses up the reservation.
        self.write_transfers().remove(&char_id);
        Some((self.get_or_spawn_world(&map_name), character, pos))
    }

    /// Releases a claim made by `resolve_entry` that never reached a map actor.
    pub fn release_character(&self, char_id: CharId) {
        self.shared.release(char_id);
    }

    fn read_transfers(&self) -> std::sync::RwLockReadGuard<'_, PendingTransfers> {
        self.pending_transfers
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_transfers(&self) -> std::sync::RwLockWriteGuard<'_, PendingTransfers> {
        self.pending_transfers
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn existing_world(&self, map_name: &str) -> Option<mpsc::Sender<WorldCommand>> {
        let worlds = match self.worlds.read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        worlds.get(map_name).cloned()
    }

    /// Current position of a character on `map_name`, if that map runs and holds it.
    pub async fn character_position(&self, map_name: &str, char_id: CharId) -> Option<MapPosition> {
        let world = self.existing_world(map_name)?;
        let (reply, rx) = oneshot::channel();
        world
            .send(WorldCommand::CharacterPosition { char_id, reply })
            .await
            .ok()?;
        rx.await.ok().flatten()
    }

    /// Visible characters within AoI range of `center` on `map_name`.
    pub async fn aoi_characters(&self, map_name: &str, center: MapPosition) -> Vec<CharId> {
        let Some(world) = self.existing_world(map_name) else {
            return Vec::new();
        };
        let (reply, rx) = oneshot::channel();
        if world
            .send(WorldCommand::AoiCharacters { center, reply })
            .await
            .is_err()
        {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }

    pub fn get_or_spawn_world(&self, map_name: &str) -> mpsc::Sender<WorldCommand> {
        if let Some(tx) = self.existing_world(map_name) {
            return tx;
        }
        // Read the map files and build the world (NPC spawns included) before taking the write
        // lock so other sessions aren't blocked on it. A racing spawn of the same map wins and
        // this one is dropped.
        let data = (self.loader)(map_name);
        let world_actor = MapWorldActor::new(
            map_name.to_string(),
            Arc::clone(&self.char_repo),
            data,
            Arc::clone(&self.shared),
            self.start_time,
        );

        let mut worlds = match self.worlds.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Map worlds write lock poisoned during get_or_spawn_world. Recovering inner state.");
                poisoned.into_inner()
            }
        };
        if let Some(tx) = worlds.get(map_name) {
            return tx.clone();
        }

        let (tx, rx) = mpsc::channel(256);
        tokio::spawn(async move {
            world_actor.run(rx).await;
        });
        worlds.insert(map_name.to_string(), tx.clone());
        tx
    }
}

/// A session that has joined this map.
struct Session {
    account_id: AccountId,
    char_id: CharId,
    outbound_tx: mpsc::Sender<Bytes>,
    /// Frames routed to this session during the current flush; sent as one `Bytes` at its end
    /// so a busy tick costs one queue slot, not one per packet.
    pending: BytesMut,
}

/// Long-lived actor owning one map's `MapInstance`, its sessions, and its 20 ms tick loop.
struct MapWorldActor {
    map_name: String,
    char_repo: Arc<dyn CharacterRepository>,
    engine: MapInstance,
    map_size: (u16, u16),
    shared: Arc<SharedState>,
    /// Process clock origin shared by every map.
    started: Instant,
    sessions: HashMap<SessionId, Session>,
    by_char: HashMap<CharId, SessionId>,
    /// Senders of sessions that left the map but whose client hasn't closed yet (restart to
    /// char select): kept so the server doesn't close the socket first.
    detached: HashMap<SessionId, mpsc::Sender<Bytes>>,
    encoder: Phase0EventEncoder,
    /// Reused per-tick buffers.
    events: Vec<ScopedServerEvent>,
    aoi: Vec<CharId>,
    evict: Vec<SessionId>,
}

/// Why a session leaves the map.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Departure {
    /// The client went away (or its queue failed): drop the sender.
    Gone,
    /// Back to character select: keep the socket open until the client closes it.
    Restart,
}

impl MapWorldActor {
    fn new(
        map_name: String,
        char_repo: Arc<dyn CharacterRepository>,
        data: MapData,
        shared: Arc<SharedState>,
        started: Instant,
    ) -> Self {
        let map_size = (data.grid.width(), data.grid.height());
        let mut engine = MapInstance::new(map_name.clone(), data.grid);
        let spawned = engine.spawn_npcs(&data.npcs);
        info!("Map '{}': {} NPCs spawned", map_name, spawned);
        Self {
            map_name,
            char_repo,
            engine,
            map_size,
            shared,
            started,
            sessions: HashMap::new(),
            by_char: HashMap::new(),
            detached: HashMap::new(),
            encoder: Phase0EventEncoder,
            events: Vec::new(),
            aoi: Vec::new(),
            evict: Vec::new(),
        }
    }

    fn now_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    async fn run(mut self, mut rx: mpsc::Receiver<WorldCommand>) {
        info!("MapWorldActor started for map '{}'", self.map_name);
        let mut tick_interval = tokio::time::interval(TICK_INTERVAL);
        tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        // Unbiased select plus a per-wake command cap: a flood of commands can't starve the tick.
        loop {
            tokio::select! {
                Some(cmd) = rx.recv() => {
                    self.handle_command(cmd);
                    for _ in 1..MAX_COMMANDS_PER_WAKE {
                        let Ok(next_cmd) = rx.try_recv() else { break };
                        self.handle_command(next_cmd);
                    }
                }
                _ = tick_interval.tick() => self.tick(),
                else => break,
            }
        }
        info!("MapWorldActor terminated for map '{}'", self.map_name);
    }

    fn tick(&mut self) {
        let now = self.now_ms();
        let mut events = mem::take(&mut self.events);
        if let Err(err) = self.engine.step_tick(now, &mut events) {
            error!("Map '{}' tick failed: {}", self.map_name, err);
        }
        self.flush(&mut events);
        self.events = events;
    }

    fn handle_command(&mut self, cmd: WorldCommand) {
        match cmd {
            WorldCommand::JoinSession {
                session_id,
                account_id,
                character,
                spawn_pos,
                outbound_tx,
            } => self.join(session_id, account_id, &character, spawn_pos, outbound_tx),
            WorldCommand::PlayerIntent(intent) => self.player_intent(intent),
            WorldCommand::LeaveSession { session_id } => {
                self.detached.remove(&session_id);
                self.leave(session_id, Departure::Gone);
            }
            WorldCommand::CharacterPosition { char_id, reply } => {
                let _ = reply.send(self.engine.character_position(char_id));
            }
            WorldCommand::AoiCharacters { center, reply } => {
                let mut chars = Vec::new();
                self.engine.query_aoi_characters(center, &mut chars);
                let _ = reply.send(chars);
            }
        }
    }

    /// Spawns the character and sends the entry burst. On failure the sender is dropped,
    /// which ends the session without sending anything, and the character's claim is released.
    fn join(
        &mut self,
        session_id: SessionId,
        account_id: AccountId,
        character: &CharacterSummary,
        spawn_pos: MapPosition,
        outbound_tx: mpsc::Sender<Bytes>,
    ) {
        let char_id = character.char_id;
        if self.sessions.contains_key(&session_id) {
            warn!(
                "Map '{}' ignoring duplicate join for {:?}",
                self.map_name, session_id
            );
            return;
        }
        let now = self.now_ms();
        let mut spawn = CharacterSpawn::from_summary(character);
        spawn.pos = spawn_pos;
        let mut result = self.engine.spawn_character(spawn.clone(), now);
        if let Err(EngineError::InvalidCell { x, y }) = result {
            // A stored position off this map (e.g. saved on a fallback grid) must not lock the
            // character out: enter at the map centre instead.
            spawn.pos = MapPosition {
                x: self.map_size.0 / 2,
                y: self.map_size.1 / 2,
                dir: spawn_pos.dir,
            };
            warn!(
                "Map '{}': char {} stored at ({}, {}) is off the map; entering at ({}, {})",
                self.map_name, char_id.0, x, y, spawn.pos.x, spawn.pos.y
            );
            result = self.engine.spawn_character(spawn.clone(), now);
        }
        let mut events = match result {
            Ok(events) => events,
            Err(err) => {
                warn!(
                    "Map '{}' rejected entry of char {} for {:?}: {}",
                    self.map_name, char_id.0, session_id, err
                );
                self.shared.release(char_id);
                return;
            }
        };
        info!(
            "Map '{}' accepted entry for {:?} (account {}, char {})",
            self.map_name, session_id, account_id.0, char_id.0
        );
        self.sessions.insert(
            session_id,
            Session {
                account_id,
                char_id,
                outbound_tx,
                pending: BytesMut::new(),
            },
        );
        self.by_char.insert(char_id, session_id);
        let accept = ServerEvent::AcceptEnter {
            account_id,
            server_tick: now as u32,
            pos: spawn.pos,
        };
        if let Some(bytes) = self.encode(&accept) {
            self.send_to_session(session_id, &bytes);
        }
        self.flush(&mut events);
    }

    fn player_intent(&mut self, intent: SessionIntent) {
        let Some(session) = self.sessions.get(&intent.session_id) else {
            debug!(
                "Map '{}' dropping intent from unjoined {:?}",
                self.map_name, intent.session_id
            );
            return;
        };
        let (account_id, char_id) = (session.account_id, session.char_id);
        match intent.intent {
            ClientIntent::EnterMap { .. } => {}
            ClientIntent::Restart { restart_type: 1 } => {
                info!(
                    "Map '{}' returning {:?} to character selection",
                    self.map_name, intent.session_id
                );
                if let Some(bytes) = self.encode(&ServerEvent::RestartAck { result: 1 }) {
                    self.send_to_session(intent.session_id, &bytes);
                }
                self.leave(intent.session_id, Departure::Restart);
            }
            ClientIntent::Restart { restart_type } => warn!(
                "Map '{}' received unsupported restart_type {} from {:?}",
                self.map_name, restart_type, intent.session_id
            ),
            other => {
                // Identity comes from the joined session, never from the packet stream.
                self.engine.push_intent(SessionIntent::new(
                    intent.session_id,
                    Some(account_id),
                    Some(char_id),
                    other,
                ));
            }
        }
    }

    fn leave(&mut self, session_id: SessionId, departure: Departure) {
        let mut events = self.remove_session(session_id, departure);
        self.flush(&mut events);
    }

    /// Removes the session and its character, sends what was queued for it, persists the last
    /// position asynchronously and releases the character's claim. Returns the engine's vanish
    /// events for routing.
    fn remove_session(
        &mut self,
        session_id: SessionId,
        departure: Departure,
    ) -> Vec<ScopedServerEvent> {
        let Some(mut session) = self.sessions.remove(&session_id) else {
            return Vec::new();
        };
        if !session.pending.is_empty() {
            let _ = session
                .outbound_tx
                .try_send(session.pending.split().freeze());
        }
        if departure == Departure::Restart {
            self.detached.insert(session_id, session.outbound_tx);
        }
        let char_id = session.char_id;
        if self.by_char.get(&char_id) == Some(&session_id) {
            self.by_char.remove(&char_id);
        }
        let pos = self.engine.character_position(char_id);
        let events = match self.engine.remove_character(char_id) {
            Ok(events) => events,
            Err(err) => {
                warn!(
                    "Map '{}' removing char {}: {}",
                    self.map_name, char_id.0, err
                );
                Vec::new()
            }
        };
        if let Some(pos) = pos {
            self.persist_position(char_id, pos);
        }
        self.shared.release(char_id);
        events
    }

    /// Records the position for a fast re-entry, then writes it to the repository.
    fn persist_position(&self, char_id: CharId, pos: MapPosition) {
        let entry = (self.map_name.clone(), pos);
        lock(&self.shared.recent_positions).insert(char_id, entry.clone());
        let repo = Arc::clone(&self.char_repo);
        let shared = Arc::clone(&self.shared);
        tokio::spawn(async move {
            match repo.update_position(char_id, &entry.0, entry.1).await {
                Ok(()) => {
                    let mut recent = lock(&shared.recent_positions);
                    if recent.get(&char_id) == Some(&entry) {
                        recent.remove(&char_id);
                    }
                }
                Err(err) => error!(
                    "Failed to persist position on leave for char {}: {}",
                    char_id.0, err
                ),
            }
        });
    }

    /// Routes `events` (draining it), sends each session its batch, then removes sessions whose
    /// queues failed and routes the resulting vanish events too.
    fn flush(&mut self, events: &mut Vec<ScopedServerEvent>) {
        loop {
            for event in events.drain(..) {
                self.route(&event);
            }
            self.send_batches();
            let Some(session_id) = self.evict.pop() else {
                break;
            };
            warn!(
                "Map '{}' evicting {:?}: outbound queue full or closed",
                self.map_name, session_id
            );
            events.extend(self.remove_session(session_id, Departure::Gone));
        }
    }

    fn send_batches(&mut self) {
        for (&session_id, session) in &mut self.sessions {
            if session.pending.is_empty() {
                continue;
            }
            let batch = session.pending.split().freeze();
            if session.outbound_tx.try_send(batch).is_err() && !self.evict.contains(&session_id) {
                self.evict.push(session_id);
            }
        }
    }

    fn encode(&self, event: &ServerEvent) -> Option<Bytes> {
        match self.encoder.encode_event(event, TARGET_PACKETVER) {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                error!("Failed to encode event {:?}: {}", event, err);
                None
            }
        }
    }

    /// Encodes once and appends to every recipient's batch.
    fn route(&mut self, scoped: &ScopedServerEvent) {
        let Some(bytes) = self.encode(&scoped.event) else {
            return;
        };
        match &scoped.scope {
            BroadcastScope::SelfOnly(char_id) => self.send_to_char(*char_id, &bytes),
            BroadcastScope::Characters(chars) => {
                for &char_id in chars {
                    self.send_to_char(char_id, &bytes);
                }
            }
            BroadcastScope::AreaOfInterest {
                center, exclude, ..
            } => {
                let mut aoi = mem::take(&mut self.aoi);
                self.engine.query_aoi_characters(*center, &mut aoi);
                for &char_id in &aoi {
                    if Some(char_id) != *exclude {
                        self.send_to_char(char_id, &bytes);
                    }
                }
                self.aoi = aoi;
            }
            BroadcastScope::MapWide { .. } => {
                for session in self.sessions.values_mut() {
                    session.pending.extend_from_slice(&bytes);
                }
            }
        }
    }

    fn send_to_char(&mut self, char_id: CharId, bytes: &Bytes) {
        if let Some(&session_id) = self.by_char.get(&char_id) {
            self.send_to_session(session_id, bytes);
        }
    }

    fn send_to_session(&mut self, session_id: SessionId, bytes: &Bytes) {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.pending.extend_from_slice(bytes);
        }
    }
}
