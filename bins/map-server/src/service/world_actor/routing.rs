//! Encoding engine events and routing them to sessions.

use std::mem;

use bytes::Bytes;
use ro_core::{
    BroadcastScope, CharId, ScopedServerEvent, ServerEvent, SessionId, TARGET_PACKETVER,
};
use ro_packets::EventEncoder;
use tracing::{error, warn};

use super::{Departure, MapWorldActor};

impl MapWorldActor {
    /// Routes `events` (draining it), sends each session its batch, then removes sessions whose
    /// queues failed and routes the resulting vanish events too.
    pub(super) fn flush(&mut self, events: &mut Vec<ScopedServerEvent>) {
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

    pub(super) fn encode(&self, event: &ServerEvent) -> Option<Bytes> {
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

    pub(super) fn send_to_session(&mut self, session_id: SessionId, bytes: &Bytes) {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.pending.extend_from_slice(bytes);
        }
    }
}
