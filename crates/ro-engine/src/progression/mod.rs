//! Character progression: exp gain and level-ups, and the GM level, job and reset commands.

mod experience;
mod level_commands;

/// Pushes whose values are read when they are sent (a `FollowUp::Live`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LivePush {
    /// `SP_STATUSPOINT`, `SP_TRAITPOINT`, `SP_BASELEVEL`, `SP_BASEEXP`, `SP_NEXTBASEEXP`.
    BaseTail,
    /// `SP_JOBLEVEL`, `SP_JOBEXP`, `SP_NEXTJOBEXP`, `SP_SKILLPOINT`.
    JobTail,
    /// `0x0acb` of `SP_BASEEXP` or `SP_JOBEXP`.
    Exp(u16),
}
/// Status or skill points, capped at what their `smallint` columns hold so a save is never
/// refused (Finish review m-2).
pub(crate) fn points(value: u32) -> u16 {
    const MAX_POINTS: u16 = i16::MAX.unsigned_abs();
    u16::try_from(value).map_or(MAX_POINTS, |v| v.min(MAX_POINTS))
}
