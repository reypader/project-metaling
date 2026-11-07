//! Logged packets in a session's sequence and in the global one.

use std::{fmt, time::SystemTime};

use ro_core::ServerType;
use ro_packets::PacketOpcode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketDirection {
    ClientToServer,
    ServerToClient,
}

impl fmt::Display for PacketDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClientToServer => write!(f, "CLIENT -> SERVER"),
            Self::ServerToClient => write!(f, "SERVER -> CLIENT"),
        }
    }
}

/// A recorded packet entry in the chronological sequence of a session.
#[derive(Debug, Clone)]
pub struct PacketSequenceEntry {
    pub seq: usize,
    pub direction: PacketDirection,
    pub opcode: PacketOpcode,
    pub length: usize,
    pub detail: String,
}

/// A recorded packet entry in the global sequence across all sessions and servers.
#[derive(Debug, Clone)]
pub struct GlobalSequenceEntry {
    pub global_seq: usize,
    pub session_id: usize,
    pub server_type: ServerType,
    pub direction: PacketDirection,
    pub opcode: PacketOpcode,
    pub length: usize,
    pub detail: String,
    pub timestamp: SystemTime,
}
