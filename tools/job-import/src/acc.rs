//! One job's entries merged across the job files, and the per-level tables built from them.

use std::collections::BTreeMap;

use ro_db::{JobBonusEntry, JobData, WeaponType};

use crate::output_dto::Report;
use crate::raw_dto::RawEntry;

/// rAthena `MAX_LEVEL`: the max base and job level of a job with no `job_exp` entry.
const MAX_LEVEL: u16 = 275;
/// Defaults for a job no entry sets (catalog db/job_stats.md).
const DEFAULT_MAX_WEIGHT: u32 = 20000;
const DEFAULT_HP_INCREASE: u32 = 500;
const DEFAULT_SP_INCREASE: u32 = 100;
/// The constant term of the fallback HP and SP formulas.
const FALLBACK_HP_BASE: u32 = 35;
const FALLBACK_SP_BASE: u32 = 10;
/// The fallback base AP is this constant from `FALLBACK_AP_LEVEL` on, for every job (catalog
/// `mechanics/trait_stats.md` §2).
const FALLBACK_AP_BASE: u32 = 200;
const FALLBACK_AP_LEVEL: u16 = 200;
/// The Super Novice line (`mapid & MAPID_SECONDMASK == MAPID_SUPER_NOVICE`: Super Novice, Super
/// Baby, their expanded forms and Hyper Novice). rAthena's fallback base HP adds
/// `SUPER_NOVICE_HP_STEP` from base level 99 and again from 150 for them (catalog
/// `mechanics/stats.md` "Hyper Novice").
const SUPER_NOVICE_LINE: [u16; 5] = [23, 4045, 4190, 4191, 4307];
const SUPER_NOVICE_HP_STEP: u32 = 2000;
const SUPER_NOVICE_HP_LEVELS: [u16; 2] = [99, 150];

/// One job's keys merged over every entry naming it.
#[derive(Debug, Default)]
pub(crate) struct Acc {
    name: String,
    max_weight: Option<u32>,
    hp_factor: Option<u32>,
    hp_increase: Option<u32>,
    sp_factor: Option<u32>,
    sp_increase: Option<u32>,
    ap_factor: Option<u32>,
    ap_increase: Option<u32>,
    bonus: BTreeMap<u16, JobBonusEntry>,
    aspd: BTreeMap<WeaponType, u16>,
    shield_aspd: Option<u16>,
    max_base_level: Option<u16>,
    max_job_level: Option<u16>,
    hp: BTreeMap<u16, u32>,
    sp: BTreeMap<u16, u32>,
    ap: BTreeMap<u16, u32>,
    base_exp: BTreeMap<u16, u64>,
    job_exp: BTreeMap<u16, u64>,
}

impl Acc {
    pub(crate) fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Self::default()
        }
    }

    pub(crate) fn merge(&mut self, entry: &RawEntry, label: &str, warnings: &mut Vec<String>) {
        take(&mut self.max_weight, entry.max_weight);
        take(&mut self.hp_factor, entry.hp_factor);
        take(&mut self.hp_increase, entry.hp_increase);
        take(&mut self.sp_factor, entry.sp_factor);
        take(&mut self.sp_increase, entry.sp_increase);
        take(&mut self.ap_factor, entry.ap_factor);
        take(&mut self.ap_increase, entry.ap_increase);
        take(&mut self.max_base_level, entry.max_base_level);
        take(&mut self.max_job_level, entry.max_job_level);
        for gain in entry.bonus_stats.iter().flatten() {
            if self.bonus.insert(gain.level, *gain).is_some() {
                warnings.push(format!(
                    "{}: BonusStats level {} set twice ({label}); the later one wins",
                    self.name, gain.level
                ));
            }
        }
        for (weapon, &value) in entry.base_aspd.iter().flatten() {
            if weapon == "Shield" {
                self.shield_aspd = Some(value);
            } else if let Some(kind) = weapon_type(weapon) {
                self.aspd.insert(kind, value);
            } else {
                warnings.push(format!(
                    "{}: unknown BaseASPD weapon '{weapon}' ({label}), skipped",
                    self.name
                ));
            }
        }
        for row in entry.base_hp.iter().flatten() {
            self.hp.insert(row.level, row.hp);
        }
        for row in entry.base_sp.iter().flatten() {
            self.sp.insert(row.level, row.sp);
        }
        for row in entry.base_ap.iter().flatten() {
            self.ap.insert(row.level, row.ap);
        }
        for (rows, table) in [
            (&entry.base_exp, &mut self.base_exp),
            (&entry.job_exp, &mut self.job_exp),
        ] {
            for row in rows.iter().flatten() {
                if row.level == 0 || row.level > MAX_LEVEL {
                    warnings.push(format!(
                        "{}: exp row Level {} outside 1..={MAX_LEVEL} ({label}), skipped",
                        self.name, row.level
                    ));
                } else {
                    table.insert(row.level, row.exp);
                }
            }
        }
    }

    /// The job, and its base and job exp tables (empty when the files gave none).
    pub(crate) fn finish(self, id: u16, report: &mut Report) -> (JobData, Vec<u64>, Vec<u64>) {
        let (max_base_level, max_job_level) = match (self.max_base_level, self.max_job_level) {
            (Some(base), Some(job)) => (base, job),
            (base, job) => {
                report.warnings.push(format!(
                    "{} ({id}): no MaxBaseLevel/MaxJobLevel in job_exp, using {MAX_LEVEL}",
                    self.name
                ));
                (base.unwrap_or(MAX_LEVEL), job.unwrap_or(MAX_LEVEL))
            }
        };
        let hp_factor = self.hp_factor.unwrap_or(0);
        let hp_increase = self.hp_increase.unwrap_or(DEFAULT_HP_INCREASE);
        let sp_factor = self.sp_factor.unwrap_or(0);
        let sp_increase = self.sp_increase.unwrap_or(DEFAULT_SP_INCREASE);
        let super_novice = SUPER_NOVICE_LINE.contains(&id);
        let (base_hp, hp_filled) = table(&self.hp, max_base_level, |level| {
            let steps = if super_novice {
                SUPER_NOVICE_HP_LEVELS
                    .iter()
                    .filter(|&&l| level >= l)
                    .count() as u32
            } else {
                0
            };
            fallback(level, FALLBACK_HP_BASE, hp_factor, hp_increase) + steps * SUPER_NOVICE_HP_STEP
        });
        let (base_sp, sp_filled) = table(&self.sp, max_base_level, |level| {
            fallback(level, FALLBACK_SP_BASE, sp_factor, sp_increase)
        });
        let ap_factor = self.ap_factor.unwrap_or(0);
        let ap_increase = self.ap_increase.unwrap_or(0);
        let (mut base_ap, _) = table(&self.ap, max_base_level, |level| {
            let base = if level >= FALLBACK_AP_LEVEL {
                FALLBACK_AP_BASE
            } else {
                0
            };
            fallback(level, base, ap_factor, ap_increase)
        });
        if base_ap.iter().all(|&ap| ap == 0) {
            base_ap.clear();
        }
        if hp_filled > 0 || sp_filled > 0 {
            report
                .fallback
                .push((self.name.clone(), hp_filled, sp_filled));
        }
        let bonus_stats = self
            .bonus
            .into_values()
            .filter(|entry| entry.level <= max_job_level)
            .collect();
        let base_exp = exp_table(&self.base_exp, max_base_level);
        let job_exp = exp_table(&self.job_exp, max_job_level);
        let job = JobData {
            id,
            name: self.name,
            max_weight: self.max_weight.unwrap_or(DEFAULT_MAX_WEIGHT),
            max_base_level,
            max_job_level,
            bonus_stats,
            base_aspd: self.aspd,
            shield_aspd: self.shield_aspd,
            base_hp,
            base_sp,
            base_ap,
        };
        (job, base_exp, job_exp)
    }
}

/// The dense exp table for levels `1..=` the last row at or below `max`; missing rows are 0
/// (rAthena skips rows above the max level).
fn exp_table(rows: &BTreeMap<u16, u64>, max: u16) -> Vec<u64> {
    let last = rows
        .range(..=max)
        .next_back()
        .map_or(0, |(&level, _)| level);
    (1..=last)
        .map(|level| rows.get(&level).copied().unwrap_or(0))
        .collect()
}

/// A later key overrides an earlier one; an absent key keeps it.
fn take<T: Copy>(earlier: &mut Option<T>, later: Option<T>) {
    if later.is_some() {
        *earlier = later;
    }
}

/// The dense table for levels `1..=max`: the file's value where it has a non-zero one, the
/// fallback elsewhere. Returns the table and how many slots were filled.
pub(crate) fn table(
    given: &BTreeMap<u16, u32>,
    max: u16,
    fallback: impl Fn(u16) -> u32,
) -> (Vec<u32>, usize) {
    let mut filled = 0;
    let values = (1..=max)
        .map(|level| match given.get(&level) {
            Some(&value) if value > 0 => value,
            _ => {
                filled += 1;
                fallback(level)
            }
        })
        .collect();
    (values, filled)
}

/// rAthena's base HP/SP/AP when a table slot is missing (catalog mechanics/stats.md,
/// mechanics/trait_stats.md §2):
/// `base + floor(level * increase / 100) + sum over i = 2..=level of round(factor / 100 * i)`.
pub(crate) fn fallback(level: u16, base: u32, factor: u32, increase: u32) -> u32 {
    let per_level = f64::from(factor) / 100.0;
    let mut total = f64::from(base) + (f64::from(level) * f64::from(increase) / 100.0).floor();
    for i in 2..=level {
        total += (per_level * f64::from(i) + 0.5).floor();
    }
    // In range by construction: at most a few hundred thousand for level 275.
    total as u32
}

/// rAthena's `W_*` suffixes as used in `job_aspd.yml`.
fn weapon_type(name: &str) -> Option<WeaponType> {
    Some(match name {
        "Fist" => WeaponType::Fist,
        "Dagger" => WeaponType::Dagger,
        "1hSword" => WeaponType::OneHandSword,
        "2hSword" => WeaponType::TwoHandSword,
        "1hSpear" => WeaponType::OneHandSpear,
        "2hSpear" => WeaponType::TwoHandSpear,
        "1hAxe" => WeaponType::OneHandAxe,
        "2hAxe" => WeaponType::TwoHandAxe,
        "Mace" => WeaponType::Mace,
        "2hMace" => WeaponType::TwoHandMace,
        "Staff" => WeaponType::Staff,
        "Bow" => WeaponType::Bow,
        "Knuckle" => WeaponType::Knuckle,
        "Musical" => WeaponType::Musical,
        "Whip" => WeaponType::Whip,
        "Book" => WeaponType::Book,
        "Katar" => WeaponType::Katar,
        "Revolver" => WeaponType::Revolver,
        "Rifle" => WeaponType::Rifle,
        "Gatling" => WeaponType::Gatling,
        "Shotgun" => WeaponType::Shotgun,
        "Grenade" => WeaponType::Grenade,
        "Huuma" => WeaponType::Huuma,
        "2hStaff" => WeaponType::TwoHandStaff,
        _ => return None,
    })
}
