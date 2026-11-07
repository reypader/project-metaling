//! rAthena job and stat-point entries as written.

use std::collections::BTreeMap;

use ro_db::JobBonusEntry;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RathenaFile<T> {
    #[serde(default = "Vec::new")]
    pub(crate) body: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawExp {
    pub(crate) level: u16,
    pub(crate) exp: u64,
}

/// One entry of any job file. Keys this importer doesn't use (outfits) are ignored.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawEntry {
    #[serde(default)]
    pub(crate) jobs: BTreeMap<String, bool>,
    pub(crate) max_weight: Option<u32>,
    pub(crate) hp_factor: Option<u32>,
    pub(crate) hp_increase: Option<u32>,
    pub(crate) sp_factor: Option<u32>,
    pub(crate) sp_increase: Option<u32>,
    pub(crate) ap_factor: Option<u32>,
    pub(crate) ap_increase: Option<u32>,
    pub(crate) bonus_stats: Option<Vec<JobBonusEntry>>,
    #[serde(rename = "BaseASPD")]
    pub(crate) base_aspd: Option<BTreeMap<String, u16>>,
    pub(crate) max_base_level: Option<u16>,
    pub(crate) max_job_level: Option<u16>,
    pub(crate) base_hp: Option<Vec<RawHp>>,
    pub(crate) base_sp: Option<Vec<RawSp>>,
    pub(crate) base_ap: Option<Vec<RawAp>>,
    pub(crate) base_exp: Option<Vec<RawExp>>,
    pub(crate) job_exp: Option<Vec<RawExp>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawHp {
    pub(crate) level: u16,
    pub(crate) hp: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawSp {
    pub(crate) level: u16,
    pub(crate) sp: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawAp {
    pub(crate) level: u16,
    pub(crate) ap: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawStatPoint {
    pub(crate) level: u16,
    pub(crate) points: u32,
    #[serde(default)]
    pub(crate) trait_points: u32,
}
