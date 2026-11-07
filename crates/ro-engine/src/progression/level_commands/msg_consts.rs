//! Reply lines of the level, job and reset commands (our wording).

pub(super) const MSG_BLVL_USAGE: &str = "Usage: @blvl <levels> (a negative number lowers)";

pub(super) const MSG_JLVL_USAGE: &str = "Usage: @jlvl <levels> (a negative number lowers)";

pub(super) const MSG_JOB_USAGE: &str = "Usage: @job <job name or id>";

pub(super) const MSG_BASE_AT_MAX: &str = "Your base level is already the highest your job allows.";

pub(super) const MSG_BASE_AT_MIN: &str = "Your base level is already 1.";

pub(super) const MSG_JOB_AT_MAX: &str = "Your job level is already the highest your job allows.";

pub(super) const MSG_JOB_AT_MIN: &str = "Your job level is already 1.";

pub(super) const MSG_NO_JOB_DATA: &str = "There is no job data for your job.";

pub(super) const MSG_SAME_JOB: &str = "You already have that job.";

pub(super) const MSG_STATS_RESET: &str = "Your stats have been reset.";
