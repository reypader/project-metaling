//! Per-client actor that runs char-select intents one at a time.

use std::sync::{Arc, RwLock};

use ro_core::{
    AccountId, ClientIntent, CreateCharacterRequest, ServerEvent, SessionId, SessionIntent,
};
use ro_persistence::{CancelDeleteOutcome, CharacterRepository, DeleteReserveOutcome};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use super::state_dto::CharSharedState;

/// Ephemeral logic actor processing `SessionIntent` items sequentially for one char-server client.
pub(super) struct CharSessionActor {
    pub(super) session_id: SessionId,
    pub(super) authenticated_account: Option<AccountId>,
    pub(super) shared: Arc<RwLock<CharSharedState>>,
    pub(super) char_repo: Arc<dyn CharacterRepository>,
    pub(super) outbound_tx: mpsc::Sender<ServerEvent>,
}

impl CharSessionActor {
    pub(super) async fn run(mut self, mut rx: mpsc::Receiver<SessionIntent>) {
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

                let characters = match self.char_repo.list_by_account(account_id).await {
                    Ok(chars) => chars,
                    Err(err) => {
                        error!(
                            "Session {:?} failed to list characters for account id {}: {}",
                            self.session_id, account_id.0, err
                        );
                        let _ = self
                            .outbound_tx
                            .try_send(ServerEvent::CharacterListRefused { error_code: 0 });
                        return false;
                    }
                };

                let event = ServerEvent::CharacterListLoaded {
                    account_id,
                    total_slots: 15,
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
            ClientIntent::RequestCharacterList => {
                let Some(account_id) = self.authenticated_account.or(session_intent.account_id)
                else {
                    warn!(
                        "Session {:?} sent RequestCharacterList before EnterCharServer; ignoring",
                        self.session_id
                    );
                    return false;
                };

                info!(
                    "Session {:?} loading character page for account id {}",
                    self.session_id, account_id.0
                );

                let characters = match self.char_repo.list_by_account(account_id).await {
                    Ok(chars) => chars,
                    Err(err) => {
                        error!(
                            "Session {:?} failed to list character page for account id {}: {}",
                            self.session_id, account_id.0, err
                        );
                        let _ = self
                            .outbound_tx
                            .try_send(ServerEvent::CharacterListRefused { error_code: 0 });
                        return false;
                    }
                };

                let event = ServerEvent::CharacterPageLoaded { characters };
                if self.outbound_tx.try_send(event).is_err() {
                    warn!(
                        "Session {:?} outbound queue full or closed; dropping char session",
                        self.session_id
                    );
                    return false;
                }
                true
            }
            ClientIntent::CreateCharacter {
                name,
                slot,
                hair_color,
                hair_style,
                starting_job,
                sex,
            } => {
                let Some(account_id) = self.authenticated_account.or(session_intent.account_id)
                else {
                    warn!(
                        "Session {:?} sent CreateCharacter before EnterCharServer; ignoring",
                        self.session_id
                    );
                    return false;
                };

                info!(
                    "Session {:?} (account {}) creating character '{}' in slot {}",
                    self.session_id, account_id.0, name, slot
                );

                let req = CreateCharacterRequest {
                    account_id,
                    name,
                    slot,
                    hair_color,
                    hair_style,
                    starting_job,
                    sex,
                };

                let event = match self.char_repo.create_character(req, 15).await {
                    Ok(character) => ServerEvent::CharacterCreated { character },
                    Err(err) => ServerEvent::CharacterCreateRefused {
                        error_code: err.wire_error_code(),
                    },
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
            ClientIntent::ReserveDeleteCharacter { char_id } => {
                let Some(account_id) = self.authenticated_account.or(session_intent.account_id)
                else {
                    warn!(
                        "Session {:?} sent ReserveDeleteCharacter before EnterCharServer; ignoring",
                        self.session_id
                    );
                    return false;
                };

                info!(
                    "Session {:?} (account {}) requesting deletion reservation for char id {}",
                    self.session_id, account_id.0, char_id.0
                );

                let (result, delete_delay_seconds) = match self
                    .char_repo
                    .reserve_delete(account_id, char_id, 86_400)
                    .await
                {
                    Ok(outcome) => outcome.wire_fields(),
                    Err(err) => {
                        error!(
                            "Session {:?} (account {}) repository error reserving deletion for char id {}: {}",
                            self.session_id, account_id.0, char_id.0, err
                        );
                        DeleteReserveOutcome::NotFoundOrDbError.wire_fields()
                    }
                };

                let event = ServerEvent::CharacterDeleteReserved {
                    char_id,
                    result,
                    delete_delay_seconds,
                };

                if self.outbound_tx.try_send(event).is_err() {
                    warn!(
                        "Session {:?} outbound queue full or closed on CharacterDeleteReserved",
                        self.session_id
                    );
                    return false;
                }
                true
            }
            ClientIntent::CancelDeleteCharacter { char_id } => {
                let Some(account_id) = self.authenticated_account.or(session_intent.account_id)
                else {
                    warn!(
                        "Session {:?} sent CancelDeleteCharacter before EnterCharServer; ignoring",
                        self.session_id
                    );
                    return false;
                };

                info!(
                    "Session {:?} (account {}) cancelling deletion reservation for char id {}",
                    self.session_id, account_id.0, char_id.0
                );

                let result = match self.char_repo.cancel_delete(account_id, char_id).await {
                    Ok(outcome) => outcome.wire_result(),
                    Err(err) => {
                        error!(
                            "Session {:?} (account {}) repository error cancelling deletion for char id {}: {}",
                            self.session_id, account_id.0, char_id.0, err
                        );
                        CancelDeleteOutcome::NotFound.wire_result()
                    }
                };

                let event = ServerEvent::CharacterDeleteCancelled { char_id, result };

                if self.outbound_tx.try_send(event).is_err() {
                    warn!(
                        "Session {:?} outbound queue full or closed on CharacterDeleteCancelled",
                        self.session_id
                    );
                    return false;
                }
                true
            }
            ClientIntent::CharPing { .. } => true,
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

                let target_char = match self.char_repo.find_by_slot(account_id, slot).await {
                    Ok(Some(ch)) => Some(ch),
                    Ok(None) => None,
                    Err(err) => {
                        error!(
                            "Session {:?} failed to query slot {} for account {}: {}",
                            self.session_id, slot, account_id.0, err
                        );
                        None
                    }
                };

                let Some(target_char) = target_char else {
                    warn!(
                        "Session {:?} (account {}) requested slot {} with no character found",
                        self.session_id, account_id.0, slot
                    );
                    return true;
                };

                let (map_host, map_port) = {
                    let state = self.shared.read().unwrap_or_else(|poisoned| {
                        warn!("CharSharedState read lock poisoned during map server resolution. Recovering inner state.");
                        poisoned.into_inner()
                    });
                    state
                        .map_servers
                        .iter()
                        .find(|srv| srv.maps.contains(&target_char.last_map))
                        .map(|srv| (srv.host, srv.port))
                        .unwrap_or(([127, 0, 0, 1], state.default_map_port))
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
