//! Vocabulary shared by more than one crate: IDs, domain DTOs, client intents and server events,
//! and internal HTTP DTOs. Crate-specific errors, traits and wire details live in their own crates.

mod account_dto;
mod char_select_dto;
mod character_dto;
mod client_intent;
mod client_ui_dto;
mod envelope_dto;
mod http;
mod ids_dto;
mod item_dto;
mod map_name;
mod server_event;
mod unit_dto;

pub use account_dto::*;
pub use char_select_dto::*;
pub use character_dto::*;
pub use client_intent::*;
pub use client_ui_dto::*;
pub use envelope_dto::*;
pub use http::*;
pub use ids_dto::*;
pub use item_dto::*;
pub use map_name::{bare_map_name, gat_map_name};
pub use server_event::*;
pub use unit_dto::*;

pub const TARGET_PACKETVER: u32 = 20260219;
