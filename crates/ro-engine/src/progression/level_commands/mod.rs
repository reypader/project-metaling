//! The GM level, job and stat-reset commands (spec `specs/stats/spec.md` §2.5-§2.6,
//! `specs/traits/spec.md` §2.5-§2.8; catalog `mechanics/atcommands.md` "@baselevelup /
//! @joblevelup", `mechanics/job_change.md`, `mechanics/trait_stats.md` §3 and §6). Reply wording
//! is our own.
//!
//! Each command changes the character at once and marks it dirty. What rAthena sends before
//! its recalc goes out at once; what it sends after is queued as follow-ups, so the wire order
//! matches the captures (spec §2.1).

mod args;
mod blvl;
mod jlvl;
mod job;
mod msg_consts;
mod points;
mod reset;

#[cfg(test)]
mod tests;
