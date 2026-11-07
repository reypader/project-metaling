use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::Instant,
};

use ro_core::{ServerEvent, SessionId, SessionIntent};
use ro_persistence::CharacterRepository;
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::{
    session_actor::CharSessionActor,
    state_dto::{CharSharedState, RegisteredMapServer},
};

pub const MAX_MAP_SERVERS: usize = 64;

/// Shared handle used by `char-server` TCP listener and internal HTTP endpoints.
#[derive(Clone)]
pub struct CharServiceHandle {
    next_session_id: Arc<AtomicU64>,
    shared: Arc<RwLock<CharSharedState>>,
    start_time: Instant,
}

impl CharServiceHandle {
    #[allow(dead_code)]
    pub fn new(char_repo: Arc<dyn CharacterRepository>) -> Self {
        Self::with_default_map_port(char_repo, 5121)
    }

    pub fn with_default_map_port(
        char_repo: Arc<dyn CharacterRepository>,
        default_map_port: u16,
    ) -> Self {
        let default_map_server = RegisteredMapServer {
            name: "metaling-map-default".to_string(),
            maps: vec!["prontera.gat".to_string()],
            host: [127, 0, 0, 1],
            port: default_map_port,
        };

        Self {
            next_session_id: Arc::new(AtomicU64::new(1)),
            shared: Arc::new(RwLock::new(CharSharedState {
                char_repo,
                map_servers: vec![default_map_server],
                default_map_port,
            })),
            start_time: Instant::now(),
        }
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    pub fn allocate_session_id(&self) -> SessionId {
        SessionId(self.next_session_id.fetch_add(1, Ordering::Relaxed))
    }

    pub fn register_map_server(
        &self,
        name: String,
        maps: Vec<String>,
        host: [u8; 4],
        port: u16,
    ) -> bool {
        info!(
            "CharService registering map server '{}' at {}.{}.{}.{}:{} with {} maps",
            name,
            host[0],
            host[1],
            host[2],
            host[3],
            port,
            maps.len()
        );
        let mut state = match self.shared.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("CharSharedState write lock poisoned during register_map_server. Recovering inner state.");
                poisoned.into_inner()
            }
        };
        if let Some(existing) = state
            .map_servers
            .iter_mut()
            .find(|srv| (srv.host == host && srv.port == port) || srv.name == name)
        {
            existing.name = name;
            existing.maps = maps;
            existing.host = host;
            existing.port = port;
            true
        } else {
            if state.map_servers.len() >= MAX_MAP_SERVERS {
                warn!(
                    "CharService rejected map server '{}': capacity limit of {} reached.",
                    name, MAX_MAP_SERVERS
                );
                return false;
            }
            state.map_servers.push(RegisteredMapServer {
                name,
                maps,
                host,
                port,
            });
            true
        }
    }

    /// Spawns an ephemeral per-session logic actor dedicated to a single client connection.
    pub fn spawn_session_actor(
        &self,
        session_id: SessionId,
        outbound_tx: mpsc::Sender<ServerEvent>,
    ) -> mpsc::Sender<SessionIntent> {
        let (intent_tx, intent_rx) = mpsc::channel(16);
        let char_repo = {
            let state = self.shared.read().unwrap_or_else(|poisoned| {
                warn!("CharSharedState read lock poisoned during session actor spawn. Recovering inner state.");
                poisoned.into_inner()
            });
            Arc::clone(&state.char_repo)
        };
        let actor = CharSessionActor {
            session_id,
            authenticated_account: None,
            shared: Arc::clone(&self.shared),
            char_repo,
            outbound_tx,
        };
        tokio::spawn(async move {
            actor.run(intent_rx).await;
        });
        intent_tx
    }
}
