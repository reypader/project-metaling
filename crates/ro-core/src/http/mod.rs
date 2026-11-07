//! Inter-server HTTP contracts and DTOs for Axum endpoints.
//!
//! Enables structured REST communication between login-server, char-server, and map-server.

mod server_registry_dto;
mod session_dto;

pub use server_registry_dto::*;
pub use session_dto::*;
