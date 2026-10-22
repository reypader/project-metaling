//! Vocabulary shared by more than one crate: IDs, domain DTOs, client intents and server events,
//! and internal HTTP DTOs. Crate-specific errors, traits and wire details live in their own crates.

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

pub const TARGET_PACKETVER: u32 = 20260219;
