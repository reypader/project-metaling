//! Domain crate: `ro-persistence`.
//! See local `AGENTS.md` for strict subagent read/write boundaries.

pub mod postgres;

pub use postgres::PostgresRepo;
pub use ro_core;
