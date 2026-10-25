use std::{
    collections::HashMap,
    mem,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock, RwLock,
    },
    time::{Duration, Instant},
};

use bytes::{Bytes, BytesMut};
use ro_core::{
    bare_map_name, gat_map_name, AccountId, BroadcastScope, CharId, CharacterSpawn, ClientIntent,
    MapPosition, ScopedServerEvent, ServerEvent, SessionId, SessionIntent, TARGET_PACKETVER,
};
use ro_db::{ItemDb, MapGrid, MapMobSpawns, MapNpcs, MobDb, NpcKind};
use ro_engine::{EngineError, InventorySnapshot, MapInstance, MapTransfer};
use ro_packets::{EventEncoder, Phase0EventEncoder};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

use crate::claim::{Claim, MapRepository, SharedState};

/// Engine tick period (BIN-SRV-01).
pub const TICK_INTERVAL: Duration = Duration::from_millis(20);
/// Side of the all-walkable grid used when a map's `.gat` is unavailable (BIN-CFG-01).
pub const FALLBACK_MAP_SIZE: u16 = 512;
/// Most commands an actor handles before it lets a tick run.
const MAX_COMMANDS_PER_WAKE: usize = 256;
/// rAthena `autosave_time`: how often an online character's changed inventory is saved.
pub const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(300);
/// How often a map actor looks for due autosaves.
const AUTOSAVE_CHECK_MS: u64 = 1000;

/// The global registries every map shares (loaded once at startup).
#[derive(Clone, Default)]
pub struct Registries {
    pub items: Arc<ItemDb>,
    pub mobs: Arc<MobDb>,
}

/// Static data a map world starts with.
#[derive(Clone)]
pub struct MapData {
    pub grid: Arc<MapGrid>,
    pub npcs: Arc<MapNpcs>,
    pub mobs: Arc<MapMobSpawns>,
    pub registries: Registries,
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

/// Loads `<dir>/<map>.yml` mob spawn lines (map name without `.gat`). No directory or no file
/// means no mobs; a bad file logs a warning and also means no mobs (BIN-MOB-01).
pub fn load_mob_spawns(mob_spawn_data_dir: Option<&Path>, map_name: &str) -> Arc<MapMobSpawns> {
    let Some(dir) = mob_spawn_data_dir else {
        return Arc::new(MapMobSpawns::default());
    };
    let stem = bare_map_name(map_name);
    match MapMobSpawns::load_yaml(&dir.join(format!("{stem}.yml"))) {
        Ok(Some(mobs)) => Arc::new(mobs),
        Ok(None) => Arc::new(MapMobSpawns::default()),
        Err(err) => {
            warn!("Map '{}': {}; spawning no mobs", map_name, err);
            Arc::new(MapMobSpawns::default())
        }
    }
}

/// Loads the item and mob registries (spec §0). An unset directory warns and gives an empty
/// registry; a set directory with a missing or bad file is an error (startup fails).
pub fn load_registries(
    item_data_dir: Option<&Path>,
    mob_data_dir: Option<&Path>,
) -> Result<Registries, ro_db::DbLoadError> {
    let items = match item_data_dir {
        Some(dir) => ItemDb::load_dir(dir)?,
        None => {
            warn!("ITEM_DATA_DIR not set; the item registry is empty");
            ItemDb::default()
        }
    };
    let mobs = match mob_data_dir {
        Some(dir) => MobDb::load_dir(dir)?,
        None => {
            warn!("MOB_DATA_DIR not set; the mob registry is empty");
            MobDb::default()
        }
    };
    info!("Registries: {} items, {} mobs", items.len(), mobs.len());
    Ok(Registries {
        items: Arc::new(items),
        mobs: Arc::new(mobs),
    })
}

/// Loader reading `.gat` files from `map_data_dir`, NPC lists from `npc_data_dir` and mob
/// spawn lists from `mob_spawn_data_dir`, and handing every map the shared `registries`. Warps
/// that can't be taken lose their trigger before the map spawns its NPCs.
pub fn map_data_loader(
    map_data_dir: Option<PathBuf>,
    npc_data_dir: Option<PathBuf>,
    mob_spawn_data_dir: Option<PathBuf>,
    registries: Registries,
) -> MapDataLoader {
    Arc::new(move |map_name: &str| {
        let mut npcs = Arc::unwrap_or_clone(load_npcs(npc_data_dir.as_deref(), map_name));
        disable_unreachable_warps(&mut npcs, map_data_dir.as_deref(), map_name);
        MapData {
            grid: load_walkability(map_data_dir.as_deref(), map_name),
            npcs: Arc::new(npcs),
            mobs: load_mob_spawns(mob_spawn_data_dir.as_deref(), map_name),
            registries: registries.clone(),
        }
    })
}

/// Clears the trigger of each warp whose destination is invalid or an instance (`@`) map, or,
/// with `map_data_dir` set, has no `.gat` file there (BIN-WARP-04). Warns once per warp.
/// Without `map_data_dir` every map uses the fallback grid, so such warps stay.
fn disable_unreachable_warps(npcs: &mut MapNpcs, map_data_dir: Option<&Path>, map_name: &str) {
    for npc in &mut npcs.npcs {
        let NpcKind::Warp(dest) = &npc.kind else {
            continue;
        };
        if npc.trigger.is_none() {
            continue;
        }
        let dest_map = gat_map_name(&dest.map);
        let reason = if !is_valid_map_name(&dest_map) {
            "invalid or instance destination"
        } else if dest_map.len() > MAX_WIRE_MAP_NAME {
            "destination name too long for 0x0091"
        } else if map_data_dir.is_some_and(|dir| !dir.join(&dest_map).is_file()) {
            "destination .gat not found"
        } else {
            continue;
        };
        warn!(
            "Map '{}': warp '{}' ({}) to '{}' is inert: {}",
            map_name, npc.name, npc.source, dest.map, reason
        );
        npc.trigger = None;
    }
}

/// Between the speaker's name and the message in `0x00f3` text.
const CHAT_SEPARATOR: &[u8] = b" : ";

/// Longest map name `0x0091` carries: a 16-byte field with a NUL.
const MAX_WIRE_MAP_NAME: usize = 15;

/// How a session enters a map.
pub enum Entry {
    /// After `CZ_ENTER`: `ZC_ACCEPT_ENTER` and the friends list now, the entry burst on
    /// `NotifyActorInit`.
    Login,
    /// By a cross-map warp: `0x0091` with the resolved cell, no `ZC_ACCEPT_ENTER`.
    Warp { dest: (u16, u16), rewarp_count: u8 },
}

/// A session joining a map. Dropping it (a failed send, a rejected join) releases `claim` and
/// drops `outbound_tx`, which ends the session.
pub struct JoinSession {
    pub(crate) session_id: SessionId,
    pub(crate) account_id: AccountId,
    pub(crate) spawn: CharacterSpawn,
    pub(crate) claim: Claim,
    pub(crate) outbound_tx: mpsc::Sender<Bytes>,
    /// The session task's control channel; the actor's `Session` keeps this clone.
    pub(crate) control_tx: mpsc::Sender<SessionControl>,
    pub(crate) entry: Entry,
}

/// Messages from a map actor to a session task.
pub enum SessionControl {
    /// The character left this map through a warp: join `dest_map`.
    ChangeMap(MapHandoff),
}

/// What a session needs to join the warp's destination map. Dropping it releases the claim
/// (persisting the trigger cell set on it) and drops `outbound_tx`.
pub struct MapHandoff {
    pub(crate) claim: Claim,
    pub(crate) spawn: CharacterSpawn,
    pub(crate) dest_map: Arc<str>,
    pub(crate) dest: (u16, u16),
    pub(crate) rewarp_count: u8,
    pub(crate) outbound_tx: mpsc::Sender<Bytes>,
}

/// Commands sent to a long-lived `MapWorldActor`.
pub enum WorldCommand {
    JoinSession(JoinSession),
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

/// Tunables of a map service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapServiceOptions {
    /// How often an online character's inventory is saved if it changed.
    pub autosave_interval: Duration,
}

impl Default for MapServiceOptions {
    fn default() -> Self {
        Self {
            autosave_interval: AUTOSAVE_INTERVAL,
        }
    }
}

/// Shared coordinator managing long-lived per-map `MapWorldActor` tasks and transfer reservations.
#[derive(Clone)]
pub struct MapServiceHandle {
    next_session_id: Arc<AtomicU64>,
    char_repo: Arc<dyn MapRepository>,
    options: MapServiceOptions,
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
    /// Starts the writer and the default map; needs a Tokio runtime.
    pub fn new(char_repo: Arc<dyn MapRepository>, loader: MapDataLoader) -> Self {
        Self::with_options(char_repo, loader, MapServiceOptions::default())
    }

    /// `new` with non-default tunables.
    pub fn with_options(
        char_repo: Arc<dyn MapRepository>,
        loader: MapDataLoader,
        options: MapServiceOptions,
    ) -> Self {
        let handle = Self {
            next_session_id: Arc::new(AtomicU64::new(1)),
            char_repo: Arc::clone(&char_repo),
            options,
            worlds: Arc::new(RwLock::new(HashMap::new())),
            pending_transfers: Arc::new(RwLock::new(HashMap::new())),
            loader,
            shared: SharedState::new(Arc::clone(&char_repo)),
            start_time: Instant::now(),
        };
        let _ = handle.get_or_spawn_world("prontera.gat");
        handle
    }

    pub fn char_repo(&self) -> &Arc<dyn MapRepository> {
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
            // Through the ordered writer, so it can't land after a newer write.
            self.shared.persist(char_id, destination_map, pos);
        }

        true
    }

    /// Resolves where an entering character goes: a pending transfer reservation wins over a
    /// just-left position still being written, which wins over the stored map/position.
    /// `None` (character already online, unknown character, character of another account, or a
    /// bad map name) means the session must close without sending anything.
    ///
    /// The character is claimed before any position is read, so a release that races this
    /// entry has already recorded its position. Dropping the returned `Claim` releases it.
    pub async fn resolve_entry(
        &self,
        account_id: AccountId,
        char_id: CharId,
    ) -> Option<(mpsc::Sender<WorldCommand>, CharacterSpawn, Claim)> {
        let Some(claim) = self.shared.claim(char_id) else {
            warn!(
                "Map entry rejected: character {} is already online",
                char_id.0
            );
            return None;
        };
        // Read the recent position and inventory before the database: once they are gone,
        // their writes have landed.
        let recent = self.shared.recent_position(char_id);
        let recent_inventory = self.shared.recent_inventory(char_id);
        let pending = self.read_transfers().get(&char_id).cloned();
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
        let play = match self.char_repo.load_play_state(char_id).await {
            Ok(Some(play)) => play,
            Ok(None) => {
                warn!(
                    "Map entry rejected: no play state for character {}",
                    char_id.0
                );
                return None;
            }
            Err(err) => {
                error!(
                    "Map entry rejected: play state of character {} failed to load: {}",
                    char_id.0, err
                );
                return None;
            }
        };
        // Only a successful entry uses up the reservation.
        self.write_transfers().remove(&char_id);
        let mut spawn = CharacterSpawn::from_summary(&character);
        spawn.pos = pos;
        spawn.status = play.status;
        spawn.group_id = play.group_id;
        spawn.inventory = recent_inventory.unwrap_or(play.inventory);
        Some((self.get_or_spawn_world(&map_name), spawn, claim))
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
        let world_actor =
            MapWorldActor::new(map_name.to_string(), data, self.start_time, self.options);

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
    /// The character's name: chat text must start with `"<name> : "`.
    name: String,
    /// Inventory revision of the last save (the engine starts every spawn at 0, which is what
    /// the database or the hand-off already holds).
    saved_revision: u64,
    /// When this session's next autosave is due (actor clock, ms).
    next_autosave_ms: u64,
    /// Released (with the last position set on it) when the session is dropped.
    claim: Claim,
    outbound_tx: mpsc::Sender<Bytes>,
    control_tx: mpsc::Sender<SessionControl>,
    /// Frames routed to this session during the current flush; sent as one `Bytes` at its end
    /// so a busy tick costs one queue slot, not one per packet.
    pending: BytesMut,
}

/// Long-lived actor owning one map's `MapInstance`, its sessions, and its 20 ms tick loop.
struct MapWorldActor {
    map_name: String,
    engine: MapInstance,
    map_size: (u16, u16),
    /// Process clock origin shared by every map.
    started: Instant,
    sessions: HashMap<SessionId, Session>,
    by_char: HashMap<CharId, SessionId>,
    /// Senders of sessions that left the map but whose client hasn't closed yet (restart to
    /// char select, or exit): kept so the server doesn't close the socket first.
    detached: HashMap<SessionId, mpsc::Sender<Bytes>>,
    encoder: Phase0EventEncoder,
    /// Reused per-tick buffers.
    events: Vec<ScopedServerEvent>,
    aoi: Vec<CharId>,
    evict: Vec<SessionId>,
    transfers: Vec<MapTransfer>,
    autosave_interval_ms: u64,
    next_autosave_check_ms: u64,
}

/// Why a session leaves the map.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Departure {
    /// The client went away (or its queue failed): drop the sender.
    Gone,
    /// Back to character select, or exit to Windows: keep the socket open until the client
    /// closes it.
    ClientCloses,
}

impl MapWorldActor {
    fn new(map_name: String, data: MapData, started: Instant, options: MapServiceOptions) -> Self {
        let map_size = (data.grid.width(), data.grid.height());
        let mut engine = MapInstance::new(map_name.clone(), data.grid);
        engine.set_registries(data.registries.items, data.registries.mobs);
        // NPCs first: they take the low unit ids, like rAthena's load order.
        let npcs = engine.spawn_npcs(&data.npcs);
        let mobs = engine.spawn_mobs(&data.mobs);
        info!(
            "Map '{}': {} NPCs and {} mobs spawned",
            map_name, npcs, mobs
        );
        Self {
            map_name,
            engine,
            map_size,
            started,
            sessions: HashMap::new(),
            by_char: HashMap::new(),
            detached: HashMap::new(),
            encoder: Phase0EventEncoder,
            events: Vec::new(),
            aoi: Vec::new(),
            evict: Vec::new(),
            transfers: Vec::new(),
            autosave_interval_ms: u64::try_from(options.autosave_interval.as_millis())
                .unwrap_or(u64::MAX),
            next_autosave_check_ms: 0,
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
        let mut transfers = mem::take(&mut self.transfers);
        self.engine.drain_transfers(&mut transfers);
        // The entity is gone, so the claim is the only record of where the character was. A
        // session evicted by this flush releases with this position. The inventory is saved
        // now (every map change saves it), before the flush can evict the session and skip
        // the hand-off.
        for transfer in &transfers {
            let char_id = transfer.spawn.char_id;
            if let Some(session) = self
                .by_char
                .get(&char_id)
                .and_then(|id| self.sessions.get_mut(id))
            {
                session.claim.set_last(&self.map_name, transfer.from_pos);
                session.claim.save_inventory(InventorySnapshot {
                    inventory: transfer.spawn.inventory.clone(),
                    looks: transfer.looks,
                });
            }
        }
        // The warping session's last batch from this map goes out before the handoff.
        self.flush(&mut events);
        for transfer in transfers.drain(..) {
            self.hand_off(transfer);
        }
        self.transfers = transfers;
        self.events = events;
        if now >= self.next_autosave_check_ms {
            self.next_autosave_check_ms = now + AUTOSAVE_CHECK_MS;
            self.autosave(now);
        }
    }

    /// Saves the inventory of every session whose autosave is due and whose inventory changed
    /// since its last save.
    fn autosave(&mut self, now: u64) {
        for session in self.sessions.values_mut() {
            if now < session.next_autosave_ms {
                continue;
            }
            session.next_autosave_ms = now.saturating_add(self.autosave_interval_ms);
            let Some(revision) = self.engine.inventory_revision(session.char_id) else {
                continue;
            };
            if revision == session.saved_revision {
                continue;
            }
            if let Some(snapshot) = self.engine.inventory_snapshot(session.char_id) {
                session.claim.save_inventory(snapshot);
                session.saved_revision = revision;
            }
        }
    }

    /// Moves a warped character's session out of this map and asks its session task to join
    /// the destination. If the task is gone, the dropped handoff releases the claim.
    fn hand_off(&mut self, transfer: MapTransfer) {
        let char_id = transfer.spawn.char_id;
        let Some(session_id) = self.by_char.remove(&char_id) else {
            return;
        };
        let Some(session) = self.sessions.remove(&session_id) else {
            return;
        };
        // `flush` just sent this session's batch (or evicted it), so nothing is pending.
        let Session {
            claim,
            outbound_tx,
            control_tx,
            ..
        } = session;
        info!(
            "Map '{}': char {} warps to '{}'",
            self.map_name, char_id.0, transfer.dest_map
        );
        let handoff = MapHandoff {
            claim,
            spawn: transfer.spawn,
            dest_map: transfer.dest_map,
            dest: transfer.dest,
            rewarp_count: transfer.rewarp_count,
            outbound_tx,
        };
        if control_tx
            .try_send(SessionControl::ChangeMap(handoff))
            .is_err()
        {
            warn!(
                "Map '{}': session {:?} gone before its map change",
                self.map_name, session_id
            );
        }
    }

    fn handle_command(&mut self, cmd: WorldCommand) {
        match cmd {
            WorldCommand::JoinSession(join) => self.join(join),
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

    /// Spawns the character and sends its first packets: `ZC_ACCEPT_ENTER`, the friends list and
    /// the engine's login burst on login, `0x0091` on a warp. On failure the join is dropped: that releases the
    /// claim (with the position set on it) and drops the sender, which ends the session.
    fn join(&mut self, join: JoinSession) {
        let JoinSession {
            session_id,
            account_id,
            spawn,
            claim,
            outbound_tx,
            control_tx,
            entry,
        } = join;
        let char_id = spawn.char_id;
        let name = spawn.name.clone();
        if self.sessions.contains_key(&session_id) {
            warn!(
                "Map '{}' ignoring duplicate join for {:?}",
                self.map_name, session_id
            );
            return;
        }
        let now = self.now_ms();
        let login = matches!(entry, Entry::Login);
        let mut login_burst = Vec::new();
        let first = match entry {
            Entry::Login => match self.spawn_login(spawn, now) {
                Ok((pos, burst)) => {
                    login_burst = burst;
                    ServerEvent::AcceptEnter {
                        account_id,
                        server_tick: now as u32,
                        pos,
                    }
                }
                Err(err) => {
                    warn!(
                        "Map '{}' rejected entry of char {} for {:?}: {}",
                        self.map_name, char_id.0, session_id, err
                    );
                    return;
                }
            },
            Entry::Warp { dest, rewarp_count } => {
                match self.engine.enter_by_warp(spawn, dest, rewarp_count, now) {
                    Ok(pos) => ServerEvent::PlayerMapMove {
                        dest_map: gat_map_name(&self.map_name),
                        dest_x: pos.x,
                        dest_y: pos.y,
                    },
                    Err(err) => {
                        warn!(
                            "Map '{}' rejected warp arrival of char {} for {:?}: {}",
                            self.map_name, char_id.0, session_id, err
                        );
                        return;
                    }
                }
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
                name,
                saved_revision: 0,
                next_autosave_ms: now.saturating_add(self.autosave_interval_ms),
                claim,
                outbound_tx,
                control_tx,
                pending: BytesMut::new(),
            },
        );
        self.by_char.insert(char_id, session_id);
        if let Some(bytes) = self.encode(&first) {
            self.send_to_session(session_id, &bytes);
        }
        // On a login rAthena sends the (empty) friends list right after ZC_ACCEPT_ENTER, then the
        // engine's login burst (MOTD, 0x0091 to the own cell, status; specs/map-entry-settings).
        if login {
            if let Some(bytes) = self.encode(&ServerEvent::FriendsList) {
                self.send_to_session(session_id, &bytes);
            }
            for scoped in &login_burst {
                if let Some(bytes) = self.encode(&scoped.event) {
                    self.send_to_session(session_id, &bytes);
                }
            }
        }
        self.flush(&mut Vec::new());
    }

    /// Spawns a logging-in character at its stored cell, or at the map centre if that cell is
    /// off this map (e.g. saved on a fallback grid), so it is never locked out. Returns the cell
    /// and the engine's login burst (all to self).
    fn spawn_login(
        &mut self,
        mut spawn: CharacterSpawn,
        now: u64,
    ) -> Result<(MapPosition, Vec<ScopedServerEvent>), EngineError> {
        let stored = spawn.pos;
        match self.engine.spawn_character(spawn.clone(), now) {
            Err(EngineError::InvalidCell { x, y }) => {
                spawn.pos = MapPosition {
                    x: self.map_size.0 / 2,
                    y: self.map_size.1 / 2,
                    dir: stored.dir,
                };
                warn!(
                    "Map '{}': char {} stored at ({}, {}) is off the map; entering at ({}, {})",
                    self.map_name, spawn.char_id.0, x, y, spawn.pos.x, spawn.pos.y
                );
                let pos = spawn.pos;
                self.engine
                    .spawn_character(spawn, now)
                    .map(|burst| (pos, burst))
            }
            result => result.map(|burst| (stored, burst)),
        }
    }

    fn player_intent(&mut self, intent: SessionIntent) {
        let Some(session) = self.sessions.get(&intent.session_id) else {
            // A restart/exit lost here (e.g. racing a cross-map handoff) leaves the client
            // waiting for its ack, so make it visible (specs/exit-game/spec.md §3).
            if matches!(
                intent.intent,
                ClientIntent::Restart { .. } | ClientIntent::RequestDisconnect { .. }
            ) {
                warn!(
                    "Map '{}' dropping {:?} from unjoined {:?}",
                    self.map_name, intent.intent, intent.session_id
                );
            } else {
                debug!(
                    "Map '{}' dropping intent from unjoined {:?}",
                    self.map_name, intent.session_id
                );
            }
            return;
        };
        let (account_id, char_id) = (session.account_id, session.char_id);
        match intent.intent {
            ClientIntent::EnterMap { .. } => {}
            ClientIntent::Chat { text } => {
                let prefix_len = session.name.len() + CHAT_SEPARATOR.len();
                // Too short to hold a message: ignored, like rAthena.
                if text.len() <= prefix_len {
                    return;
                }
                let prefix_ok = text.starts_with(session.name.as_bytes())
                    && text[session.name.len()..].starts_with(CHAT_SEPARATOR);
                if !prefix_ok {
                    // rAthena treats this as a hacked or desynced client and disconnects.
                    warn!(
                        "Map '{}' closing {:?}: chat without the '<name> : ' prefix",
                        self.map_name, intent.session_id
                    );
                    self.leave(intent.session_id, Departure::Gone);
                    return;
                }
                self.engine.push_intent(SessionIntent::new(
                    intent.session_id,
                    Some(account_id),
                    Some(char_id),
                    ClientIntent::Chat { text },
                ));
            }
            ClientIntent::Restart { restart_type: 1 } => {
                info!(
                    "Map '{}' returning {:?} to character selection",
                    self.map_name, intent.session_id
                );
                if let Some(bytes) = self.encode(&ServerEvent::RestartAck { result: 1 }) {
                    self.send_to_session(intent.session_id, &bytes);
                }
                self.leave(intent.session_id, Departure::ClientCloses);
            }
            ClientIntent::Restart { restart_type } => warn!(
                "Map '{}' received unsupported restart_type {} from {:?}",
                self.map_name, restart_type, intent.session_id
            ),
            // The type isn't checked (only 0 is known). Always allowed until combat adds the
            // logout delay (specs/exit-game/spec.md).
            ClientIntent::RequestDisconnect { disconnect_type } => {
                info!(
                    "Map '{}' {:?} exits the game (type {})",
                    self.map_name, intent.session_id, disconnect_type
                );
                if let Some(bytes) = self.encode(&ServerEvent::DisconnectAck { result: 0 }) {
                    self.send_to_session(intent.session_id, &bytes);
                }
                self.leave(intent.session_id, Departure::ClientCloses);
            }
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

    /// Removes the session and its character and sends what was queued for it. Dropping the
    /// session's claim then persists the last position and releases the character. Returns the
    /// engine's vanish events for routing.
    fn remove_session(
        &mut self,
        session_id: SessionId,
        departure: Departure,
    ) -> Vec<ScopedServerEvent> {
        let Some(session) = self.sessions.remove(&session_id) else {
            return Vec::new();
        };
        let Session {
            char_id,
            mut claim,
            outbound_tx,
            mut pending,
            ..
        } = session;
        let delivered =
            pending.is_empty() || outbound_tx.try_send(pending.split().freeze()).is_ok();
        // If the last batch (e.g. the restart/exit ack) can't be queued, the client would wait
        // forever, so drop the sender and let the session close.
        if departure == Departure::ClientCloses && delivered {
            self.detached.insert(session_id, outbound_tx);
        }
        if self.by_char.get(&char_id) == Some(&session_id) {
            self.by_char.remove(&char_id);
        }
        // A character that warped out this tick is no longer in the engine; its claim already
        // holds the trigger cell, and the hand-off saved its inventory.
        let events = match self.engine.character_position(char_id) {
            None => Vec::new(),
            Some(pos) => {
                claim.set_last(&self.map_name, pos);
                if let Some(snapshot) = self.engine.inventory_snapshot(char_id) {
                    claim.set_inventory(snapshot);
                }
                self.engine.remove_character(char_id).unwrap_or_else(|err| {
                    warn!(
                        "Map '{}' removing char {}: {}",
                        self.map_name, char_id.0, err
                    );
                    Vec::new()
                })
            }
        };
        drop(claim);
        events
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

#[cfg(test)]
mod tests {
    //! `resolve_entry` claiming (spec BIN-WARP-07).

    use std::sync::atomic::Ordering;

    use super::*;
    use crate::claim::tests::{at, eventually, FakeRepo, ACCOUNT, CHAR};

    fn service(repo: Arc<FakeRepo>) -> MapServiceHandle {
        let grid = Arc::new(MapGrid::all_walkable(200, 200).expect("valid dims"));
        let data = MapData {
            grid,
            npcs: Arc::new(MapNpcs::default()),
            mobs: Arc::new(MapMobSpawns::default()),
            registries: Registries::default(),
        };
        MapServiceHandle::new(repo, Arc::new(move |_: &str| data.clone()))
    }

    #[tokio::test]
    async fn bin_warp_07_resolve_entry_claims_before_reading() {
        let repo = FakeRepo::new(true);
        let service = service(repo.clone());
        let held = service.shared.claim(CHAR).expect("claim");
        assert!(service.resolve_entry(ACCOUNT, CHAR).await.is_none());
        assert_eq!(
            repo.finds.load(Ordering::Relaxed),
            0,
            "rejected before any read"
        );
        drop(held);
        let (_, spawn, _claim) = service.resolve_entry(ACCOUNT, CHAR).await.expect("enters");
        assert_eq!(spawn.pos, at(100, 100));
    }

    #[tokio::test]
    async fn bin_warp_07_resolve_entry_uses_a_racing_release() {
        let repo = FakeRepo::new(false);
        let service = service(repo.clone());
        let mut claim = service.shared.claim(CHAR).expect("claim");
        claim.set_last("prontera.gat", at(120, 130));
        drop(claim);

        // The write is still blocked: the recorded position wins over the database.
        let (_, spawn, claim) = service.resolve_entry(ACCOUNT, CHAR).await.expect("enters");
        assert_eq!(spawn.pos, at(120, 130));
        drop(claim);

        // Once the write lands, the database has it.
        repo.gate.add_permits(1);
        assert!(eventually(|| service.shared.recent_position(CHAR).is_none()).await);
        let (_, spawn, _claim) = service.resolve_entry(ACCOUNT, CHAR).await.expect("enters");
        assert_eq!(spawn.pos, at(120, 130));
    }

    /// A handoff whose session task is gone releases its claim (with the trigger cell) and
    /// closes the outbound queue, whether it was queued before or after the receiver dropped.
    #[tokio::test]
    async fn bin_warp_07_handoff_in_a_dropped_channel_releases() {
        let repo = FakeRepo::new(true);
        let service = service(repo.clone());
        for push_before_drop in [true, false] {
            let mut claim = service.shared.claim(CHAR).expect("claim");
            claim.set_last("prontera.gat", at(110, 100));
            let (outbound_tx, mut outbound_rx) = mpsc::channel::<Bytes>(1);
            let handoff = MapHandoff {
                claim,
                spawn: spawn_at(110, 100),
                dest_map: Arc::from("geffen.gat"),
                dest: (100, 100),
                rewarp_count: 0,
                outbound_tx,
            };
            let (control_tx, control_rx) = mpsc::channel::<SessionControl>(1);
            if push_before_drop {
                assert!(control_tx
                    .try_send(SessionControl::ChangeMap(handoff))
                    .is_ok());
                drop(control_rx);
            } else {
                drop(control_rx);
                assert!(control_tx
                    .try_send(SessionControl::ChangeMap(handoff))
                    .is_err());
            }
            drop(control_tx);
            assert_eq!(
                service.shared.recent_position(CHAR),
                Some(("prontera.gat".to_string(), at(110, 100))),
                "trigger cell recorded"
            );
            assert!(outbound_rx.recv().await.is_none(), "outbound queue closed");
            let again = service.shared.claim(CHAR);
            assert!(
                again.is_some(),
                "released (push before drop: {push_before_drop})"
            );
        }
    }

    /// Eviction in the transfer tick (the session's queue is full) releases the claim with the
    /// trigger cell, and no handoff is sent.
    #[tokio::test]
    async fn bin_warp_07_eviction_in_the_transfer_tick_keeps_the_trigger_cell() {
        let shared = SharedState::new(FakeRepo::new(false));
        let warp = ro_db::NpcSpawn {
            name: "w".into(),
            unique_name: "w".into(),
            x: 50,
            y: 50,
            dir: 0,
            sprite: 45,
            sprite_is_mob: false,
            state: ro_db::NpcState::Normal,
            trigger: Some((1, 1)),
            duplicate_of: None,
            kind: NpcKind::Warp(ro_db::WarpDest {
                map: "geffen".into(),
                x: 100,
                y: 100,
                trigger_while_hidden: false,
            }),
            source: "t:1".into(),
        };
        let data = MapData {
            grid: Arc::new(MapGrid::all_walkable(200, 200).expect("valid dims")),
            npcs: Arc::new(MapNpcs { npcs: vec![warp] }),
            mobs: Arc::new(MapMobSpawns::default()),
            registries: Registries::default(),
        };
        let mut actor = MapWorldActor::new(
            "prontera.gat".into(),
            data,
            Instant::now(),
            MapServiceOptions::default(),
        );
        let (outbound_tx, _outbound_rx) = mpsc::channel(1);
        let (control_tx, mut control_rx) = mpsc::channel(1);
        let session_id = SessionId(1);
        actor.join(JoinSession {
            session_id,
            account_id: ACCOUNT,
            spawn: CharacterSpawn {
                inventory: crate::claim::tests::potions(7),
                ..spawn_at(50, 50)
            },
            claim: shared.claim(CHAR).expect("claim"),
            outbound_tx,
            control_tx,
            entry: Entry::Login,
        });
        // ZC_ACCEPT_ENTER filled the one-slot queue, so this tick's batch can't be sent. The
        // arrival cell is a warp cell, so the same tick transfers the character out.
        actor.player_intent(SessionIntent::new(
            session_id,
            Some(ACCOUNT),
            Some(CHAR),
            ClientIntent::NotifyActorInit,
        ));
        actor.tick();
        assert!(control_rx.try_recv().is_err(), "evicted: no handoff");
        assert_eq!(
            shared.recent_position(CHAR),
            Some(("prontera.gat".to_string(), at(50, 50)))
        );
        assert_eq!(
            shared.recent_inventory(CHAR),
            Some(crate::claim::tests::potions(7)),
            "the inventory that left with the warp is saved even though the hand-off never ran"
        );
        assert!(shared.claim(CHAR).is_some(), "released");
    }

    fn spawn_at(x: u16, y: u16) -> CharacterSpawn {
        CharacterSpawn {
            account_id: ACCOUNT,
            char_id: CHAR,
            name: "Char1".into(),
            sex: ro_core::Sex::Female,
            job_class: 0,
            base_level: 1,
            hair_style: 1,
            hair_color: 1,
            pos: at(x, y),
            speed: 150,
            status: Default::default(),
            group_id: 0,
            inventory: Default::default(),
        }
    }

    #[tokio::test]
    async fn bin_warp_07_rejection_after_claim_releases() {
        let repo = FakeRepo::new(true);
        let service = service(repo);
        let foreign = AccountId(ACCOUNT.0 + 1);
        assert!(service.resolve_entry(foreign, CHAR).await.is_none());
        assert!(
            service.resolve_entry(ACCOUNT, CHAR).await.is_some(),
            "the rejected entry released its claim"
        );
    }
}
