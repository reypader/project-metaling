//! Messages between sessions, the service and map actors.

use std::sync::Arc;

use bytes::Bytes;
use ro_core::{AccountId, CharId, CharacterSpawn, MapPosition, SessionId, SessionIntent};
use tokio::sync::{mpsc, oneshot};

use crate::claim::Claim;

/// How a session enters a map.
pub enum Entry {
    /// After `CZ_ENTER`: `ZC_ACCEPT_ENTER` and the friends list now, the entry burst on
    /// `NotifyActorInit`.
    Login,
    /// By a cross-map warp: `0x0091` with the resolved cell, no `ZC_ACCEPT_ENTER`.
    Warp { dest: (u16, u16), rewarp_count: u8 },
}

/// A session joining a map. Dropping it (a failed send, a rejected join) releases `claim` and
/// drops `outbound_tx`, which ends the session.
pub struct JoinSession {
    pub(crate) session_id: SessionId,
    pub(crate) account_id: AccountId,
    pub(crate) spawn: CharacterSpawn,
    pub(crate) claim: Claim,
    pub(crate) outbound_tx: mpsc::Sender<Bytes>,
    /// The session task's control channel; the actor's `Session` keeps this clone.
    pub(crate) control_tx: mpsc::Sender<SessionControl>,
    pub(crate) entry: Entry,
}

/// Messages from a map actor to a session task.
pub enum SessionControl {
    /// The character left this map through a warp: join `dest_map`.
    ChangeMap(MapHandoff),
}

/// What a session needs to join the warp's destination map. Dropping it releases the claim
/// (persisting the trigger cell set on it) and drops `outbound_tx`.
pub struct MapHandoff {
    pub(crate) claim: Claim,
    pub(crate) spawn: CharacterSpawn,
    pub(crate) dest_map: Arc<str>,
    pub(crate) dest: (u16, u16),
    pub(crate) rewarp_count: u8,
    pub(crate) outbound_tx: mpsc::Sender<Bytes>,
}

/// Commands sent to a long-lived `MapWorldActor`.
pub enum WorldCommand {
    // Boxed: a spawn (inventory, skills) dwarfs the other variants.
    JoinSession(Box<JoinSession>),
    PlayerIntent(SessionIntent),
    LeaveSession {
        session_id: SessionId,
    },
    CharacterPosition {
        char_id: CharId,
        reply: oneshot::Sender<Option<MapPosition>>,
    },
    AoiCharacters {
        center: MapPosition,
        reply: oneshot::Sender<Vec<CharId>>,
    },
}
