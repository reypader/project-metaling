//! Saving sessions: autosave, requested saves and the cross-map handoff.

use ro_engine::{MapInstance, MapTransfer};
use tracing::{info, warn};

use super::{MapWorldActor, Session};
use crate::service::{MapHandoff, SessionControl};

impl MapWorldActor {
    /// Saves the play state of every session whose autosave is due and whose inventory or
    /// progress changed since its last save.
    pub(super) fn autosave(&mut self, now: u64) {
        for session in self.sessions.values_mut() {
            if now < session.next_autosave_ms {
                continue;
            }
            session.next_autosave_ms = now.saturating_add(self.autosave_interval_ms);
            let (Some(revision), Some(progress), Some(skills)) = (
                self.engine.inventory_revision(session.char_id),
                self.engine.progress_snapshot(session.char_id),
                self.engine.skills_revision(session.char_id),
            ) else {
                continue;
            };
            if revision == session.saved_revision
                && progress == session.saved_progress
                && skills == session.saved_skills_revision
            {
                continue;
            }
            Self::save_session(&self.engine, session);
        }
    }

    /// Queues `session`'s play state now and records what was saved.
    fn save_session(engine: &MapInstance, session: &mut Session) {
        let (Some(revision), Some(skills), Some(snapshot)) = (
            engine.inventory_revision(session.char_id),
            engine.skills_revision(session.char_id),
            engine.play_snapshot(session.char_id),
        ) else {
            return;
        };
        session.saved_progress = snapshot.progress;
        session.saved_skills_revision = skills;
        session.claim.save_play(snapshot);
        session.saved_revision = revision;
    }

    /// Saves the characters a flow asked to save now (`@job`; specs `specs/stats/spec.md`
    /// §2.7), once each. One that warped out this tick is skipped: its hand-off already saved
    /// it, after the flow ran.
    pub(super) fn save_requested(&mut self) {
        self.engine.drain_save_requests(&mut self.save_requests);
        self.save_requests.sort_unstable_by_key(|c| c.0);
        self.save_requests.dedup();
        for char_id in self.save_requests.drain(..) {
            if let Some(session) = self
                .by_char
                .get(&char_id)
                .and_then(|id| self.sessions.get_mut(id))
            {
                Self::save_session(&self.engine, session);
            }
        }
    }

    /// Moves a warped character's session out of this map and asks its session task to join
    /// the destination. If the task is gone, the dropped handoff releases the claim.
    pub(super) fn hand_off(&mut self, transfer: MapTransfer) {
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
}
