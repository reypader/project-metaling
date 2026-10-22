//! PostgreSQL persistence: the repository contracts and their `sqlx` implementation.

mod postgres;
mod repository;

pub use postgres::PostgresRepo;
pub use repository::{
    AccountRepository, AuthError, CancelDeleteOutcome, CharacterRepository, CreateCharacterError,
    DeleteReserveOutcome, RepositoryError,
};
