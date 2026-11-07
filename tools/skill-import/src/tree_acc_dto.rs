//! Skill-tree entries merged per job, before inheritance is resolved.

use std::collections::HashMap;

/// One job's own tree, with repeated entries for the same job merged.
#[derive(Debug, Default)]
pub(crate) struct JobAcc {
    pub(crate) name: String,
    /// Parent names (upper case) in first-listed order; a later listing changes the flag.
    pub(crate) inherit: Vec<(String, bool)>,
    pub(crate) own: Vec<EntryAcc>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct EntryAcc {
    pub(crate) name: String,
    pub(crate) max_level: Option<u8>,
    pub(crate) exclude: bool,
    pub(crate) base_level: u16,
    pub(crate) job_level: u16,
    /// Level 0 removes a requirement; kept here and dropped when the tree is resolved.
    pub(crate) requires: Vec<(String, u8)>,
}

pub(crate) struct Jobs {
    pub(crate) list: Vec<JobAcc>,
    pub(crate) by_name: HashMap<String, usize>,
}
