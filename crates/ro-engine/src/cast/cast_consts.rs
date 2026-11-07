//! Constants shared by the cast request and pipeline: target-type bits and failure causes.

/// rAthena `inf` bits (the data's target type).
pub(super) const INF_ATTACK: u8 = 1;

pub(super) const INF_GROUND: u8 = 2;

pub(super) const INF_SELF: u8 = 4;

/// `0x0110` causes (`USESKILL_FAIL_*`).
pub(super) const FAIL_LEVEL: u8 = 0;

pub(super) const FAIL_SP: u8 = 1;

pub(super) const FAIL_HP: u8 = 2;

pub(super) const FAIL_MONEY: u8 = 5;

pub(super) const FAIL_WEAPON: u8 = 6;

pub(super) const FAIL_WEIGHT: u8 = 9;
