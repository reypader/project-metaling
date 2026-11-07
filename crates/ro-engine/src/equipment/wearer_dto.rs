//! Equip check inputs and refusals.

use ro_core::Sex;

/// The character traits the equip checks read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Wearer {
    pub(crate) base_level: u16,
    pub(crate) sex: Sex,
    /// From [`crate::job::job_mapid`].
    pub(crate) mapid: u32,
}

/// Why an equip request is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// Nothing is sent back.
    NoAck,
    /// `0x0999` with this result.
    Result(u8),
    /// `0x0291` with this message, then `0x0999` result 2.
    Message { msg_id: u16 },
}
