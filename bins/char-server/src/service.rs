use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, RwLock,
};

use ro_contracts::{
    ids::{AccountId, CharId, CharacterSummary, MapPosition, SessionId, Sex},
    packet::{ClientIntent, ServerEvent, SessionIntent},
};
use tokio::sync::mpsc;
use tracing::{info, warn};

#[derive(Clone)]
struct RegisteredMapServer {
    maps: Vec<String>,
    host: [u8; 4],
    port: u16,
}

struct CharSharedState {
    characters: Vec<CharacterSummary>,
    map_servers: Vec<RegisteredMapServer>,
}

/// Shared handle used by `char-server` TCP listener and internal HTTP endpoints.
#[derive(Clone)]
pub struct CharServiceHandle {
    next_session_id: Arc<AtomicU64>,
    shared: Arc<RwLock<CharSharedState>>,
}

impl CharServiceHandle {
    pub fn new() -> Self {
        let stub_character = CharacterSummary {
            char_id: CharId(150001),
            account_id: AccountId(2000000),
            slot: 0,
            name: "Novice".to_string(),
            sex: Sex::Male,
            job_class: 0,
            base_level: 1,
            job_level: 1,
            hair_style: 1,
            hair_color: 1,
            last_map: "prontera.gat".to_string(),
            last_pos: MapPosition {
                x: 156,
                y: 191,
                dir: 0,
            },
        };

        let default_map_server = RegisteredMapServer {
            maps: vec!["prontera.gat".to_string()],
            host: [127, 0, 0, 1],
            port: 5121,
        };

        Self {
            next_session_id: Arc::new(AtomicU64::new(1)),
            shared: Arc::new(RwLock::new(CharSharedState {
                characters: vec![stub_character],
                map_servers: vec![default_map_server],
            })),
        }
    }

    pub fn allocate_session_id(&self) -> SessionId {
        SessionId(self.next_session_id.fetch_add(1, Ordering::Relaxed))
    }

    pub fn register_map_server(&self, name: String, maps: Vec<String>, host: [u8; 4], port: u16) {
        info!(
            "CharService registered map server '{}' at {}.{}.{}.{}:{} with {} maps",
            name,
            host[0],
            host[1],
            host[2],
            host[3],
            port,
            maps.len()
        );
        if let Ok(mut state) = self.shared.write() {
            state.map_servers.push(RegisteredMapServer { maps, host, port });
        }
    }

    /// Spawns an ephemeral per-session logic actor dedicated to a single client connection.
    pub fn spawn_session_actor(
        &self,
        session_id: SessionId,
        outbound_tx: mpsc::Sender<ServerEvent>,
    ) -> mpsc::Sender<SessionIntent> {
        let (intent_tx, intent_rx) = mpsc::channel(16);
        let actor = CharSessionActor {
            session_id,
            authenticated_account: None,
            shared: Arc::clone(&self.shared),
            outbound_tx,
        };
        tokio::spawn(async move {
            actor.run(intent_rx).await;
        });
        intent_tx
    }
}

impl Default for CharServiceHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Ephemeral logic actor processing `SessionIntent` items sequentially for one char-server client.
struct CharSessionActor {
    session_id: SessionId,
    authenticated_account: Option<AccountId>,
    shared: Arc<RwLock<CharSharedState>>,
    outbound_tx: mpsc::Sender<ServerEvent>,
}

impl CharSessionActor {
    async fn run(mut self, mut rx: mpsc::Receiver<SessionIntent>) {
        while let Some(session_intent) = rx.recv().await {
            if !self.handle_intent(session_intent).await {
                break;
            }
        }
    }

    async fn handle_intent(&mut self, session_intent: SessionIntent) -> bool {
        match session_intent.intent {
            ClientIntent::EnterCharServer { account_id, .. } => {
                info!(
                    "Session {:?} loading characters for account id {}",
                    self.session_id, account_id.0
                );
                self.authenticated_account = Some(account_id);

                let characters = self
                    .shared
                    .read()
                    .map(|s| s.characters.clone())
                    .unwrap_or_default();

                let event = ServerEvent::CharacterListLoaded {
                    account_id,
                    total_slots: 3,
                    characters,
                };
                if self.outbound_tx.try_send(event).is_err() {
                    warn!(
                        "Session {:?} outbound queue full or closed; dropping char session",
                        self.session_id
                    );
                    return false;
                }
                true
            }
            ClientIntent::SelectCharacter { slot } => {
                let Some(account_id) = self.authenticated_account.or(session_intent.account_id)
                else {
                    warn!(
                        "Session {:?} sent SelectCharacter before EnterCharServer; ignoring",
                        self.session_id
                    );
                    return false;
                };

                info!(
                    "Session {:?} (account {}) selecting character in slot {}",
                    self.session_id, account_id.0, slot
                );

                let (target_char, map_host, map_port) = {
                    let state = self.shared.read().expect("CharSharedState lock poisoned");
                    let ch = state
                        .characters
                        .iter()
                        .find(|c| c.slot == slot)
                        .cloned()
                        .unwrap_or_else(|| state.characters[0].clone());

                    let (host, port) = state
                        .map_servers
                        .iter()
                        .find(|srv| srv.maps.contains(&ch.last_map))
                        .map(|srv| (srv.host, srv.port))
                        .unwrap_or(([127, 0, 0, 1], 5121));

                    (ch, host, port)
                };

                let event = ServerEvent::CharacterSelected {
                    char_id: target_char.char_id,
                    map_name: target_char.last_map,
                    map_host,
                    map_port,
                };
                if self.outbound_tx.try_send(event).is_err() {
                    warn!(
                        "Session {:?} outbound queue full or closed on CharacterSelected",
                        self.session_id
                    );
                }
                false
            }
            unhandled => {
                warn!(
                    "Session {:?} received unhandled char intent: {:?}",
                    self.session_id, unhandled
                );
                true
            }
        }
    }
}
