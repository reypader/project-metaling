//! Char-server state shared by the service handle and the session actors.

use std::sync::Arc;

use ro_persistence::CharacterRepository;

#[derive(Clone)]
pub(super) struct RegisteredMapServer {
    pub(super) name: String,
    pub(super) maps: Vec<String>,
    pub(super) host: [u8; 4],
    pub(super) port: u16,
}

pub(super) struct CharSharedState {
    pub(super) char_repo: Arc<dyn CharacterRepository>,
    pub(super) map_servers: Vec<RegisteredMapServer>,
    pub(super) default_map_port: u16,
}
