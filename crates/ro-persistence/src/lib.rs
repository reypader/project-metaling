//! Domain crate: `ro-persistence`.
//! See local `AGENTS.md` for strict subagent read/write boundaries.

pub mod stub;

pub use ro_contracts;
pub use stub::InMemoryStubRepo;
