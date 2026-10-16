use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, RwLock,
};

use ro_contracts::{
    ids::{AccountId, CharServerInfo, SessionId, Sex},
    packet::{ClientIntent, ServerEvent, SessionIntent},
};
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Shared state accessible by ephemeral `LoginSessionActor` tasks and HTTP handlers.
#[derive(Clone)]
pub struct LoginServiceHandle {
    next_session_id: Arc<AtomicU64>,
    char_servers: Arc<RwLock<Vec<CharServerInfo>>>,
}

impl LoginServiceHandle {
    pub fn new() -> Self {
        let default_char_server = CharServerInfo {
            ip: [127, 0, 0, 1],
            port: 6121,
            name: "Metaling-Char".to_string(),
            users: 0,
            server_type: 0,
            new_server: 0,
        };
        Self {
            next_session_id: Arc::new(AtomicU64::new(1)),
            char_servers: Arc::new(RwLock::new(vec![default_char_server])),
        }
    }

    pub fn allocate_session_id(&self) -> SessionId {
        SessionId(self.next_session_id.fetch_add(1, Ordering::Relaxed))
    }

    pub fn register_char_server(&self, info: CharServerInfo) {
        info!("LoginService registered char server: {}", info.name);
        if let Ok(mut servers) = self.char_servers.write() {
            servers.push(info);
        }
    }

    /// Spawns an ephemeral per-session logic actor dedicated to a single client connection.
    pub fn spawn_session_actor(
        &self,
        session_id: SessionId,
        outbound_tx: mpsc::Sender<ServerEvent>,
    ) -> mpsc::Sender<SessionIntent> {
        let (intent_tx, intent_rx) = mpsc::channel(16);
        let actor = LoginSessionActor {
            session_id,
            char_servers: Arc::clone(&self.char_servers),
            outbound_tx,
        };
        tokio::spawn(async move {
            actor.run(intent_rx).await;
        });
        intent_tx
    }
}

impl Default for LoginServiceHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Ephemeral logic actor processing `SessionIntent` messages sequentially for one login client.
struct LoginSessionActor {
    session_id: SessionId,
    char_servers: Arc<RwLock<Vec<CharServerInfo>>>,
    outbound_tx: mpsc::Sender<ServerEvent>,
}

impl LoginSessionActor {
    async fn run(mut self, mut rx: mpsc::Receiver<SessionIntent>) {
        while let Some(session_intent) = rx.recv().await {
            if !self.handle_intent(session_intent).await {
                break;
            }
        }
    }

    async fn handle_intent(&mut self, session_intent: SessionIntent) -> bool {
        match session_intent.intent {
            ClientIntent::Login {
                ref username,
                client_version,
                client_type,
                ..
            } => {
                info!(
                    "Session {:?} processing login for user '{}' (version={}, client_type={})",
                    self.session_id, username, client_version, client_type
                );
                let char_servers = self
                    .char_servers
                    .read()
                    .map(|list| list.clone())
                    .unwrap_or_default();

                let event = ServerEvent::LoginAccepted {
                    account_id: AccountId(2000000),
                    login_id1: 1,
                    login_id2: 2,
                    sex: Sex::Male,
                    char_servers,
                };
                if self.outbound_tx.try_send(event).is_err() {
                    warn!(
                        "Session {:?} outbound queue full or closed; dropping login session",
                        self.session_id
                    );
                }
                false
            }
            unhandled => {
                warn!(
                    "Session {:?} received unhandled login intent: {:?}",
                    self.session_id, unhandled
                );
                true
            }
        }
    }
}
