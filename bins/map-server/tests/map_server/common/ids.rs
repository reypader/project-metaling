//! Character, account and unit ids the tests use.

pub const MAP: &str = "prontera.gat";

pub fn account(n: u32) -> u32 {
    2_000_000 + n
}

pub fn char_id(n: u32) -> u32 {
    150_000 + n
}
pub const A: u32 = 1;

pub const B: u32 = 2;

/// The first mob gid on a map.
pub const MOB_GID: u32 = 110_000_000;

pub const START_NPC_ID: u32 = 110_000_000;
