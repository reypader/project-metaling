//! Vocabulary shared by more than one crate: IDs, domain DTOs, client intents and server events,
//! and internal HTTP DTOs. Crate-specific errors, traits and wire details live in their own crates.

mod account;
mod character;
mod http;
mod ids;
mod item;
mod map_name;
mod message;

pub use account::*;
pub use character::*;
pub use http::*;
pub use ids::*;
pub use item::*;
pub use map_name::{bare_map_name, gat_map_name};
pub use message::*;

pub const TARGET_PACKETVER: u32 = 20260219;
