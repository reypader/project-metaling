//! Per-client actor that runs login intents one at a time.

use ro_core::{ClientIntent, ServerEvent, SessionId, SessionIntent};
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::LoginService;

/// Ephemeral logic actor processing `SessionIntent` messages sequentially for one login client.
pub(super) struct LoginSessionActor {
    pub(super) session_id: SessionId,
    pub(super) service: LoginService,
    pub(super) outbound_tx: mpsc::Sender<ServerEvent>,
    pub(super) login_processed: bool,
}

impl LoginSessionActor {
    pub(super) async fn run(mut self, mut rx: mpsc::Receiver<SessionIntent>) {
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
                ref password,
                client_version,
                client_type,
            } => {
                if self.login_processed {
                    warn!(
                        "Session {:?} received duplicate login intent; ignoring",
                        self.session_id
                    );
                    return true;
                }
                info!(
                    "Session {:?} processing login for user '{}' (version={}, client_type={})",
                    self.session_id, username, client_version, client_type
                );
                let event = self
                    .service
                    .process_login(username, password, client_version, client_type)
                    .await;

                if self.outbound_tx.try_send(event).is_err() {
                    warn!(
                        "Session {:?} outbound queue full or closed; dropping login session",
                        self.session_id
                    );
                    return false;
                }
                self.login_processed = true;
                true
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
