use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::Instant,
};

use ro_core::{AccountId, CharId, CharacterSpawn, MapPosition, SessionId};
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn};

use super::{world_actor::MapWorldActor, MapDataLoader, MapServiceOptions, WorldCommand};
use crate::claim::{Claim, MapRepository, SharedState};

/// Reserved transfers: char -> (map name, optional entry position).
type PendingTransfers = HashMap<CharId, (String, Option<MapPosition>)>;

/// Shared coordinator managing long-lived per-map `MapWorldActor` tasks and transfer reservations.
#[derive(Clone)]
pub struct MapServiceHandle {
    next_session_id: Arc<AtomicU64>,
    char_repo: Arc<dyn MapRepository>,
    options: MapServiceOptions,
    worlds: Arc<RwLock<HashMap<String, mpsc::Sender<WorldCommand>>>>,
    pending_transfers: Arc<RwLock<PendingTransfers>>,
    loader: MapDataLoader,
    pub(super) shared: Arc<SharedState>,
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
        // Read the recent position and play state before the database: once they are gone,
        // their writes have landed.
        let recent = self.shared.recent_position(char_id);
        let recent_play = self.shared.recent_play(char_id);
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
        spawn.inventory = play.inventory;
        spawn.skills = play.skills;
        spawn.change_level = play.change_level;
        if let Some(recent) = recent_play {
            spawn.inventory = recent.inventory;
            spawn.job_class = recent.progress.job_class;
            spawn.base_level = recent.progress.base_level;
            spawn.status = recent.progress.status;
            spawn.change_level = recent.progress.change_level;
            spawn.skills = recent.skills;
        }
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
