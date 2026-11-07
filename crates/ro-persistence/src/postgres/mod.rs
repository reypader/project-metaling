//! PostgreSQL implementation of repository contracts using `sqlx`.

pub mod account;
pub mod character;
mod convert;
pub mod inventory;
mod mapping;
pub mod pool;

pub use pool::PostgresRepo;
