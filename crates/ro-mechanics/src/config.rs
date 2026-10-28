//! The battle and player config knobs the formulas read (catalog `mechanics/battle_config.md`).
//! Keys are rAthena's knob names; every key is optional and defaults to rAthena's **shipped**
//! value (`conf/battle/*.conf`), which differs from its compiled default for some knobs.

use serde::Deserialize;

use crate::ClassFlags;

/// `BL_PC` in the `enable_*` masks.
pub(crate) const BL_PC: u32 = 0x1;

/// rAthena's `AMOTION_ZERO_ASPD` and `AMOTION_INTERVAL`: ASPD `a` is amotion `2000 - a * 10`.
const AMOTION_ZERO_ASPD: u32 = 2000;
const AMOTION_INTERVAL: u32 = 10;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BattleConfig {
    // Derived-stat enables (BL_* masks).
    pub enable_baseatk_renewal: u32,
    pub enable_critical: u32,
    pub enable_perfect_flee: u32,
    // HIT rate and the multi-attacker penalties (read by combat, later).
    pub min_hitrate: u32,
    pub max_hitrate: u32,
    pub agi_penalty_type: u32,
    pub agi_penalty_target: u32,
    pub agi_penalty_count: u32,
    pub agi_penalty_num: u32,
    pub vit_penalty_type: u32,
    pub vit_penalty_target: u32,
    pub vit_penalty_count: u32,
    pub vit_penalty_num: u32,
    pub weapon_defense_type: u32,
    // ASPD caps, in ASPD points (100..=199).
    pub max_aspd: u32,
    pub max_third_aspd: u32,
    pub max_extended_aspd: u32,
    pub max_summoner_aspd: u32,
    // MaxHP / MaxSP / MaxAP.
    pub hp_rate: u32,
    pub sp_rate: u32,
    pub max_hp_lv99: u32,
    pub max_hp_lv150: u32,
    pub max_hp: u32,
    pub max_sp: u32,
    pub max_ap: u32,
    // Weight.
    pub natural_heal_weight_rate: u32,
    pub major_overweight_rate: u32,
    pub max_cart_weight: u32,
    // Natural regen intervals (ms).
    pub natural_healhp_interval: u32,
    pub natural_healsp_interval: u32,
    pub natural_heal_skill_interval: u32,
    // Max stat per class group, and the trait cap.
    pub max_parameter: u16,
    pub max_trans_parameter: u16,
    pub max_third_parameter: u16,
    pub max_third_trans_parameter: u16,
    pub max_baby_parameter: u16,
    pub max_baby_third_parameter: u16,
    pub max_extended_parameter: u16,
    pub max_summoner_parameter: u16,
    pub max_fourth_parameter: u16,
    pub max_trait_parameter: u16,
    // Status points and level-up (`start_status_points` is in `inter_athena.conf` there).
    pub start_status_points: u32,
    pub use_statpoint_table: bool,
    pub multi_level_up: bool,
    pub multi_level_up_base: u32,
    pub multi_level_up_job: u32,
}

impl Default for BattleConfig {
    fn default() -> Self {
        Self {
            enable_baseatk_renewal: 0x29F,
            enable_critical: 0x11,
            enable_perfect_flee: 0x1,
            min_hitrate: 5,
            max_hitrate: 100,
            agi_penalty_type: 1,
            agi_penalty_target: BL_PC,
            agi_penalty_count: 3,
            agi_penalty_num: 10,
            vit_penalty_type: 1,
            vit_penalty_target: BL_PC,
            vit_penalty_count: 3,
            vit_penalty_num: 5,
            weapon_defense_type: 0,
            max_aspd: 190,
            max_third_aspd: 193,
            max_extended_aspd: 193,
            max_summoner_aspd: 193,
            hp_rate: 100,
            sp_rate: 100,
            max_hp_lv99: 330_000,
            max_hp_lv150: 660_000,
            max_hp: 1_100_000,
            max_sp: 1_000_000,
            max_ap: 1000,
            natural_heal_weight_rate: 70,
            major_overweight_rate: 90,
            max_cart_weight: 8000,
            natural_healhp_interval: 6000,
            natural_healsp_interval: 8000,
            natural_heal_skill_interval: 10000,
            max_parameter: 99,
            max_trans_parameter: 99,
            max_third_parameter: 130,
            max_third_trans_parameter: 130,
            max_baby_parameter: 80,
            max_baby_third_parameter: 117,
            max_extended_parameter: 130,
            max_summoner_parameter: 130,
            max_fourth_parameter: 130,
            max_trait_parameter: 110,
            start_status_points: 48,
            use_statpoint_table: true,
            multi_level_up: false,
            multi_level_up_base: 0,
            multi_level_up_job: 0,
        }
    }
}

impl BattleConfig {
    /// Checks the ranges rAthena enforces at load. The error names the first bad key.
    pub fn validate(&self) -> Result<(), String> {
        let range = |name: &str, value: u32, min: u32, max: u32| {
            if (min..=max).contains(&value) {
                Ok(())
            } else {
                Err(format!("{name} is {value}, expected {min}..={max}"))
            }
        };
        range("max_aspd", self.max_aspd, 100, 199)?;
        range("max_third_aspd", self.max_third_aspd, 100, 199)?;
        range("max_extended_aspd", self.max_extended_aspd, 100, 199)?;
        range("max_summoner_aspd", self.max_summoner_aspd, 100, 199)?;
        range("hp_rate", self.hp_rate, 1, u32::MAX)?;
        range("sp_rate", self.sp_rate, 1, u32::MAX)?;
        range("min_hitrate", self.min_hitrate, 0, 100)?;
        range("max_hitrate", self.max_hitrate, 0, 100)?;
        let short = u32::from(i16::MAX.unsigned_abs());
        for (name, value) in [
            ("max_parameter", self.max_parameter),
            ("max_trans_parameter", self.max_trans_parameter),
            ("max_third_parameter", self.max_third_parameter),
            ("max_third_trans_parameter", self.max_third_trans_parameter),
            ("max_baby_parameter", self.max_baby_parameter),
            ("max_baby_third_parameter", self.max_baby_third_parameter),
            ("max_extended_parameter", self.max_extended_parameter),
            ("max_summoner_parameter", self.max_summoner_parameter),
            ("max_fourth_parameter", self.max_fourth_parameter),
            ("max_trait_parameter", self.max_trait_parameter),
        ] {
            range(name, u32::from(value), 10, short)?;
        }
        Ok(())
    }

    /// The highest STR..LUK a class may reach, in the catalog's first-match order.
    pub fn max_stat(&self, class: &ClassFlags) -> u16 {
        if class.baby && class.third {
            self.max_baby_third_parameter
        } else if class.baby {
            self.max_baby_parameter
        } else if class.summoner {
            self.max_summoner_parameter
        } else if class.extended {
            self.max_extended_parameter
        } else if class.trait_job {
            self.max_fourth_parameter
        } else if class.third && (class.upper || class.fourth) {
            self.max_third_trans_parameter
        } else if class.third {
            self.max_third_parameter
        } else if class.upper {
            self.max_trans_parameter
        } else {
            self.max_parameter
        }
    }

    /// The lowest amotion a class may reach: its ASPD cap as amotion, `2000 - cap * 10`
    /// (rAthena converts the cap to doubled amotion at load and clamps at half of it).
    pub fn amotion_floor(&self, class: &ClassFlags) -> u32 {
        let cap = if class.third {
            self.max_third_aspd
        } else if class.extended {
            self.max_extended_aspd
        } else if class.summoner {
            self.max_summoner_aspd
        } else {
            self.max_aspd
        };
        AMOTION_ZERO_ASPD.saturating_sub(cap * AMOTION_INTERVAL)
    }
}
