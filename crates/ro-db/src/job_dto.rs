//! Job data as stored in `job_db.yml`, `statpoint.yml` and `job_exp.yml`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::yaml::{flow_list, is_zero};
use crate::WeaponType;

/// Stat bonus from job level: the six primary stats and the six trait stats.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct JobBonus {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub str: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub agi: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub vit: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub int: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub dex: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub luk: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pow: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sta: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub wis: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub spl: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub con: i16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub crt: i16,
}

impl JobBonus {
    pub(crate) fn add(&mut self, other: &JobBonus) {
        self.str += other.str;
        self.agi += other.agi;
        self.vit += other.vit;
        self.int += other.int;
        self.dex += other.dex;
        self.luk += other.luk;
        self.pow += other.pow;
        self.sta += other.sta;
        self.wis += other.wis;
        self.spl += other.spl;
        self.con += other.con;
        self.crt += other.crt;
    }
}

/// One `BonusStats` entry: the stats gained on reaching `level`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct JobBonusEntry {
    pub level: u16,
    #[serde(flatten)]
    pub gain: JobBonus,
}

/// One job as stored in `job_db.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct JobData {
    pub id: u16,
    /// rAthena's `JOB_` suffix as written in its job files (`Lord_Knight`).
    pub name: String,
    pub max_weight: u32,
    pub max_base_level: u16,
    pub max_job_level: u16,
    /// Sparse gains by job level (as in rAthena); accumulated at load.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bonus_stats: Vec<JobBonusEntry>,
    /// Weapons the job lists; any other weapon is `DEFAULT_ASPD_BASE`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub base_aspd: BTreeMap<WeaponType, u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield_aspd: Option<u16>,
    /// Base HP at base level `i + 1`; exactly `max_base_level` entries.
    #[serde(serialize_with = "flow_list")]
    pub base_hp: Vec<u32>,
    #[serde(serialize_with = "flow_list")]
    pub base_sp: Vec<u32>,
    /// Base AP at base level `i + 1`: empty (all zero) or exactly `max_base_level` entries.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub base_ap: Vec<u32>,
}

/// One `statpoint.yml` row: cumulative status and trait points at base level `level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct StatPointRow {
    pub level: u16,
    pub points: u32,
    /// Absent below base level 201.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub trait_points: u32,
}

/// Which level an exp table is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExpKind {
    Base,
    Job,
}

/// One `job_exp.yml` entry: jobs that share one exp table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct ExpGroup {
    pub kind: ExpKind,
    #[serde(serialize_with = "flow_list")]
    pub jobs: Vec<u16>,
    /// Index `level - 1`: the exp needed to go from `level` to `level + 1`. A missing row is 0.
    #[serde(serialize_with = "flow_list")]
    pub exp: Vec<u64>,
}
