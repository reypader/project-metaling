//! Shared domain contracts, IDs, events, and repository traits.
//! Owned exclusively by the Main Orchestrator Agent.

pub mod account;
pub mod http;
pub mod ids;
pub mod map;
pub mod packet;

pub use account::*;
pub use http::*;
pub use ids::*;
pub use map::*;
pub use packet::*;

/// Target client version from `reference/rathena/src/config/packets.hpp`.
pub const TARGET_PACKETVER: u32 = 20260219;
