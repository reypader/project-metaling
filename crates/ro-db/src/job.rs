use crate::{JobBonus, WeaponType};

/// ASPD base of a weapon (or shield) the job table doesn't list.
pub const DEFAULT_ASPD_BASE: u16 = 200;
pub(crate) const WEAPON_TYPES: usize = WeaponType::TwoHandStaff as usize + 1;

/// A loaded job, with its job bonus accumulated per job level.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: u16,
    pub name: String,
    pub max_weight: u32,
    pub max_base_level: u16,
    pub max_job_level: u16,
    /// Index `job_level - 1`; `max_job_level` rows.
    pub(crate) bonus: Vec<JobBonus>,
    pub(crate) aspd: [u16; WEAPON_TYPES],
    pub(crate) shield_aspd: u16,
    pub(crate) base_hp: Vec<u32>,
    pub(crate) base_sp: Vec<u32>,
    /// Empty when the job has no AP at any level.
    pub(crate) base_ap: Vec<u32>,
    /// Index into `JobDb::exp_tables`, per `ExpKind`.
    pub(crate) base_exp: Option<usize>,
    pub(crate) job_exp: Option<usize>,
}

impl Job {
    /// Total job bonus at `job_level` (clamped to `1..=max_job_level`).
    pub fn bonus(&self, job_level: u16) -> JobBonus {
        clamped(&self.bonus, job_level).copied().unwrap_or_default()
    }

    /// Base HP from the table at `base_level` (clamped to `1..=max_base_level`).
    pub fn base_hp(&self, base_level: u16) -> u32 {
        clamped(&self.base_hp, base_level).copied().unwrap_or(0)
    }

    pub fn base_sp(&self, base_level: u16) -> u32 {
        clamped(&self.base_sp, base_level).copied().unwrap_or(0)
    }

    /// Base AP (the MaxAP before bonuses) at `base_level`; 0 if the job has no AP table.
    pub fn base_ap(&self, base_level: u16) -> u32 {
        clamped(&self.base_ap, base_level).copied().unwrap_or(0)
    }

    pub fn aspd_base(&self, weapon: WeaponType) -> u16 {
        self.aspd
            .get(weapon as usize)
            .copied()
            .unwrap_or(DEFAULT_ASPD_BASE)
    }

    pub fn shield_aspd(&self) -> u16 {
        self.shield_aspd
    }
}

/// The entry for a 1-based `level`, clamped to the table.
pub(crate) fn clamped<T>(table: &[T], level: u16) -> Option<&T> {
    let last = table.len().checked_sub(1)?;
    table.get(usize::from(level.max(1) - 1).min(last))
}
