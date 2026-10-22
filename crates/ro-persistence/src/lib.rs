//! Domain crate: `ro-persistence`.
//! See local `AGENTS.md` for strict subagent read/write boundaries.

pub mod postgres;
pub mod repository;

pub use postgres::PostgresRepo;
pub use repository::{
    AccountRepository, AuthError, CancelDeleteOutcome, CharacterRepository, CreateCharacterError,
    DeleteReserveOutcome, RepositoryError,
};
pub use ro_core;
