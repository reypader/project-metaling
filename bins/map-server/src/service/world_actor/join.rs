//! Commands from sessions: joining, player intents and leaving.

use bytes::BytesMut;
use ro_core::{
    gat_map_name, CharacterSpawn, ClientIntent, MapPosition, ScopedServerEvent, ServerEvent,
    SessionId, SessionIntent,
};
use ro_engine::EngineError;
use tracing::{debug, info, warn};

use super::{Departure, MapWorldActor, Session, CHAT_SEPARATOR};
use crate::service::{Entry, JoinSession, WorldCommand};

impl MapWorldActor {
    pub(super) fn handle_command(&mut self, cmd: WorldCommand) {
        match cmd {
            WorldCommand::JoinSession(join) => self.join(*join),
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
    pub(crate) fn join(&mut self, join: JoinSession) {
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
                saved_progress: self.engine.progress_snapshot(char_id).unwrap_or_default(),
                saved_skills_revision: self.engine.skills_revision(char_id).unwrap_or_default(),
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

    pub(crate) fn player_intent(&mut self, intent: SessionIntent) {
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
    pub(super) fn remove_session(
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
                if let Some(snapshot) = self.engine.play_snapshot(char_id) {
                    claim.set_play(snapshot);
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
}
