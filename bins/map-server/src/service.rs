use std::{
    collections::HashMap,
    mem,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::{Duration, Instant},
};

use bytes::Bytes;
use ro_contracts::{
    character::{CharacterRepository, DEFAULT_WALK_SPEED},
    ids::{AccountId, CharId, MapPosition, SessionId, Sex},
    packet::{ClientIntent, EventEncoder, ServerEvent, SessionIntent, UnitAppearance},
    TARGET_PACKETVER,
};
use ro_packets::Phase0EventEncoder;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Commands sent to a long-lived `MapWorldActor`.
pub enum WorldCommand {
    JoinSession {
        session_id: SessionId,
        account_id: AccountId,
        char_id: CharId,
        spawn_pos: MapPosition,
        outbound_tx: mpsc::Sender<Bytes>,
        initial_intent: SessionIntent,
    },
    PlayerIntent(SessionIntent),
    LeaveSession {
        session_id: SessionId,
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
    pub fn new(char_repo: Arc<dyn CharacterRepository>) -> Self {
        let handle = Self {
            next_session_id: Arc::new(AtomicU64::new(1)),
            char_repo,
            worlds: Arc::new(RwLock::new(HashMap::new())),
            pending_transfers: Arc::new(RwLock::new(HashMap::new())),
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

    /// Resolves the target map and spawn position for an entering character.
    pub async fn resolve_world_for_character(
        &self,
        char_id: CharId,
    ) -> (mpsc::Sender<WorldCommand>, MapPosition) {
        let pending = {
            let mut transfers = match self.pending_transfers.write() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    warn!("Pending transfers write lock poisoned during resolve_world_for_character. Recovering inner state.");
                    poisoned.into_inner()
                }
            };
            transfers.remove(&char_id)
        };

        let default_pos = MapPosition {
            x: 156,
            y: 191,
            dir: 0,
        };

        let (map_name, spawn_pos) = match pending {
            Some((map, Some(pos))) => (map, pos),
            Some((map, None)) => match self.char_repo.find_by_id(char_id).await {
                Ok(Some(ch)) => (map, ch.last_pos),
                _ => (map, default_pos),
            },
            None => match self.char_repo.find_by_id(char_id).await {
                Ok(Some(ch)) => (ch.last_map, ch.last_pos),
                _ => ("prontera.gat".to_string(), default_pos),
            },
        };

        (self.get_or_spawn_world(&map_name), spawn_pos)
    }

    pub fn get_or_spawn_world(&self, map_name: &str) -> mpsc::Sender<WorldCommand> {
        let worlds_guard = match self.worlds.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Map worlds read lock poisoned during get_or_spawn_world. Recovering inner state.");
                poisoned.into_inner()
            }
        };
        if let Some(tx) = worlds_guard.get(map_name) {
            return tx.clone();
        }
        drop(worlds_guard);

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
        let world_actor = MapWorldActor::new(map_name.to_string(), Arc::clone(&self.char_repo));
        tokio::spawn(async move {
            world_actor.run(rx).await;
        });
        worlds.insert(map_name.to_string(), tx.clone());
        tx
    }
}

struct ConnectedPlayer {
    account_id: AccountId,
    char_id: CharId,
    pos: MapPosition,
    outbound_tx: mpsc::Sender<Bytes>,
}

/// Long-lived actor owning a single map instance, its connected player sessions, and its tick loop.
struct MapWorldActor {
    map_name: String,
    default_pos: MapPosition,
    server_tick: u32,
    char_repo: Arc<dyn CharacterRepository>,
    sessions: HashMap<SessionId, ConnectedPlayer>,
    pending_intents: Vec<SessionIntent>,
    scratch_intents: Vec<SessionIntent>,
    encoder: Phase0EventEncoder,
}

impl MapWorldActor {
    fn new(map_name: String, char_repo: Arc<dyn CharacterRepository>) -> Self {
        Self {
            map_name,
            default_pos: MapPosition {
                x: 156,
                y: 191,
                dir: 0,
            },
            server_tick: 1000,
            char_repo,
            sessions: HashMap::new(),
            pending_intents: Vec::with_capacity(64),
            scratch_intents: Vec::with_capacity(64),
            encoder: Phase0EventEncoder,
        }
    }

    async fn run(mut self, mut rx: mpsc::Receiver<WorldCommand>) {
        info!("MapWorldActor started for map '{}'", self.map_name);
        let mut tick_interval = tokio::time::interval(Duration::from_millis(20));

        loop {
            tokio::select! {
                 biased;
                Some(cmd) = rx.recv() => {
                    self.enqueue_command(cmd);
                    while let Ok(next_cmd) = rx.try_recv() {
                        self.enqueue_command(next_cmd);
                    }
                    self.drain_intents();
                }
                _ = tick_interval.tick() => {
                    self.server_tick = self.server_tick.wrapping_add(20);
                    self.drain_intents();
                }
                else => break,
            }
        }
        info!("MapWorldActor terminated for map '{}'", self.map_name);
    }

    fn enqueue_command(&mut self, cmd: WorldCommand) {
        match cmd {
            WorldCommand::JoinSession {
                session_id,
                account_id,
                char_id,
                spawn_pos,
                outbound_tx,
                initial_intent,
            } => {
                self.sessions.insert(
                    session_id,
                    ConnectedPlayer {
                        account_id,
                        char_id,
                        pos: spawn_pos,
                        outbound_tx,
                    },
                );
                self.pending_intents.push(initial_intent);
            }
            WorldCommand::PlayerIntent(session_intent) => {
                self.pending_intents.push(session_intent);
            }
            WorldCommand::LeaveSession { session_id } => {
                self.remove_and_persist_player(session_id);
            }
        }
    }

    fn remove_and_persist_player(&mut self, session_id: SessionId) -> Option<ConnectedPlayer> {
        let player = self.sessions.remove(&session_id)?;
        let repo = Arc::clone(&self.char_repo);
        let char_id = player.char_id;
        let map_name = self.map_name.clone();
        let pos = player.pos;
        tokio::spawn(async move {
            if let Err(err) = repo.update_position(char_id, &map_name, pos).await {
                error!(
                    "Failed to persist position on disconnect for char {}: {}",
                    char_id.0, err
                );
            }
        });
        Some(player)
    }

    fn drain_intents(&mut self) {
        if self.pending_intents.is_empty() {
            return;
        }
        mem::swap(&mut self.pending_intents, &mut self.scratch_intents);
        let mut scratch = mem::take(&mut self.scratch_intents);
        for session_intent in scratch.drain(..) {
            self.process_intent(session_intent);
        }
        scratch.clear();
        self.scratch_intents = scratch;
    }

    fn process_intent(&mut self, session_intent: SessionIntent) {
        let session_id = session_intent.session_id;
        match session_intent.intent {
            ClientIntent::EnterMap {
                account_id,
                char_id,
                ..
            } => {
                info!(
                    "Map '{}' accepted entry for session {:?} (account {}, char {})",
                    self.map_name, session_id, account_id.0, char_id.0
                );
                let (resolved_account_id, pos) = self
                    .sessions
                    .get(&session_id)
                    .map(|p| (p.account_id, p.pos))
                    .unwrap_or((account_id, self.default_pos));

                let enter_event = ServerEvent::AcceptEnter {
                    account_id: resolved_account_id,
                    server_tick: self.server_tick,
                    pos,
                };
                self.send_unicast(session_id, &enter_event);
            }
            ClientIntent::NotifyActorInit => {
                let Some(player) = self.sessions.get(&session_id) else {
                    warn!(
                        "Map '{}' received NotifyActorInit for unknown session {:?}",
                        self.map_name, session_id
                    );
                    return;
                };
                let char_id = player.char_id;
                let pos = player.pos;

                info!(
                    "Map '{}' synchronizing actor init for session {:?} (char {})",
                    self.map_name, session_id, char_id.0
                );
                // Transitional (M1): ids and appearance come from the engine in M6.
                let appear_event = ServerEvent::UnitSpawned {
                    unit: UnitAppearance {
                        gid: char_id.0,
                        char_id: Some(char_id),
                        name: "Novice".to_string(),
                        job: 0,
                        sex: Sex::Male,
                        hair_style: 1,
                        hair_color: 1,
                        head_dir: 0,
                        pos,
                        speed: DEFAULT_WALK_SPEED,
                        dead_sit: 0,
                        clevel: 1,
                    },
                };
                self.send_unicast(session_id, &appear_event);
                self.broadcast_aoi(session_id, pos, &appear_event);

                let par_event = ServerEvent::ParameterChanged {
                    var_id: 0,
                    value: 150,
                };
                self.send_unicast(session_id, &par_event);
            }
            ClientIntent::RequestTime { client_tick } => {
                let time_event = ServerEvent::TimeSynchronized {
                    server_tick: client_tick,
                };
                self.send_unicast(session_id, &time_event);
            }
            ClientIntent::Restart { restart_type } => {
                if restart_type == 1 {
                    info!(
                        "Map '{}' returning session {:?} to character selection",
                        self.map_name, session_id
                    );
                    let restart_ack = ServerEvent::RestartAck { result: 1 };
                    self.send_unicast(session_id, &restart_ack);
                    self.remove_and_persist_player(session_id);
                } else {
                    warn!(
                        "Map '{}' received unsupported restart_type {} from {:?}",
                        self.map_name, restart_type, session_id
                    );
                }
            }
            unhandled => {
                warn!(
                    "Map '{}' received unhandled intent from {:?}: {:?}",
                    self.map_name, session_id, unhandled
                );
            }
        }
    }

    /// Encodes a `ServerEvent` once into frozen `Bytes` and dispatches via non-blocking `try_send`.
    fn send_unicast(&mut self, session_id: SessionId, event: &ServerEvent) {
        let bytes = match self.encoder.encode_event(event, TARGET_PACKETVER) {
            Ok(b) => b,
            Err(err) => {
                error!("Failed to encode event {:?}: {}", event, err);
                return;
            }
        };

        if let Some(player) = self.sessions.get(&session_id) {
            if player.outbound_tx.try_send(bytes).is_err() {
                warn!(
                    "Session {:?} (char {}) outbound queue full or closed on map '{}'; removing session",
                    session_id, player.char_id.0, self.map_name
                );
                self.remove_and_persist_player(session_id);
            }
        }
    }

    /// Encodes a `ServerEvent` and broadcasts it to all sessions within AoI (excluding the source session).
    fn broadcast_aoi(&mut self, source_session: SessionId, pos: MapPosition, event: &ServerEvent) {
        let bytes = match self.encoder.encode_event(event, TARGET_PACKETVER) {
            Ok(b) => b,
            Err(err) => {
                error!("Failed to encode broadcast event {:?}: {}", event, err);
                return;
            }
        };

        let mut disconnected = Vec::new();
        for (&session_id, player) in &self.sessions {
            if session_id == source_session {
                continue;
            }
            let dx = (player.pos.x as i32 - pos.x as i32).abs();
            let dy = (player.pos.y as i32 - pos.y as i32).abs();
            if dx <= 14 && dy <= 14 && player.outbound_tx.try_send(bytes.clone()).is_err() {
                warn!(
                    "Session {:?} (char {}) outbound queue full or closed on map '{}'; removing session",
                    session_id, player.char_id.0, self.map_name
                );
                disconnected.push(session_id);
            }
        }
        for session_id in disconnected {
            self.remove_and_persist_player(session_id);
        }
    }
}
