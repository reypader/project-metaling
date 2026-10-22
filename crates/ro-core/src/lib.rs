//! Vocabulary shared by more than one crate: IDs, domain DTOs, client intents and server events,
//! and internal HTTP DTOs. Crate-specific errors, traits and wire details live in their own crates.

pub mod account;
pub mod character;
pub mod http;
pub mod ids;
pub mod message;
pub mod repository;

pub use account::*;
pub use character::*;
pub use http::*;
pub use ids::*;
pub use message::*;
pub use repository::*;

pub const TARGET_PACKETVER: u32 = 20260219;
