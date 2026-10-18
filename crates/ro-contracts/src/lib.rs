//! Shared domain contracts, IDs, events, and repository traits.
//! Owned exclusively by the Main Orchestrator Agent.

pub mod account;
pub mod character;
pub mod engine;
pub mod http;
pub mod ids;
pub mod map;
pub mod packet;
pub mod repository;

pub use account::*;
pub use character::*;
pub use engine::*;
pub use http::*;
pub use ids::*;
pub use map::*;
pub use packet::*;
pub use repository::*;

/// Target client version from `reference/rathena/src/config/packets.hpp`.
pub const TARGET_PACKETVER: u32 = 20260219;
