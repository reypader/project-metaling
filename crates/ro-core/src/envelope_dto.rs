//! Envelopes that route intents and events: who sent an intent, and who receives an event.

use crate::client_intent::ClientIntent;
use crate::ids_dto::{AccountId, CharId, MapPosition, SessionId};
use crate::server_event::ServerEvent;

/// Scope definition for spatial or broadcast packets emitted by map-server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BroadcastScope {
    SelfOnly(CharId),
    /// Explicit recipients, resolved by the emitter at emission time (the engine resolves its
    /// AoI events this way, like rAthena's `clif_send(AREA)`).
    Characters(Vec<CharId>),
    /// Everyone whose character is in AoI range of `center`, minus `exclude`.
    AreaOfInterest {
        map_name: String,
        center: MapPosition,
        exclude: Option<CharId>,
    },
    MapWide {
        map_name: String,
    },
}

/// Session-authenticated envelope wrapping a wire `ClientIntent` for dispatch to server logic actors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionIntent {
    pub session_id: SessionId,
    pub account_id: Option<AccountId>,
    pub char_id: Option<CharId>,
    pub intent: ClientIntent,
}

impl SessionIntent {
    pub fn new(
        session_id: SessionId,
        account_id: Option<AccountId>,
        char_id: Option<CharId>,
        intent: ClientIntent,
    ) -> Self {
        Self {
            session_id,
            account_id,
            char_id,
            intent,
        }
    }
}

/// High-level domain event tagged with its target broadcast scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedServerEvent {
    pub scope: BroadcastScope,
    pub event: ServerEvent,
}

impl ScopedServerEvent {
    pub fn new(scope: BroadcastScope, event: ServerEvent) -> Self {
        Self { scope, event }
    }
}
