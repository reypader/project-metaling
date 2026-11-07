//! Pure recalc inputs: weapon class, ASPD base, per-hand and gear inputs, the job row.

use ro_db::{Job, WeaponType, DEFAULT_ASPD_BASE};
use ro_mechanics::{GearInput, HandInput, JobRow, Primary, RefineInput, Traits, WeaponClass};

use crate::equipment::{GearStatus, WeaponGear};

/// rAthena's `MaxWeight` for a job without its own (catalog `db/job_stats.md`).
const DEFAULT_MAX_WEIGHT: u32 = 20_000;

/// The formulas' weapon class of a weapon type (spec §2.2; plan M2 mapping notes).
pub(crate) fn weapon_class(weapon_type: WeaponType) -> WeaponClass {
    use WeaponType::*;
    match weapon_type {
        Fist => WeaponClass::Fist,
        Katar => WeaponClass::Katar,
        Bow => WeaponClass::Bow,
        Musical | Whip | Revolver | Rifle | Gatling | Shotgun | Grenade => WeaponClass::Ranged,
        _ => WeaponClass::Melee,
    }
}

pub(crate) fn aspd_base(job: Option<&Job>, weapon_type: WeaponType) -> u16 {
    job.map_or(DEFAULT_ASPD_BASE, |j| j.aspd_base(weapon_type))
}

fn hand_input(job: Option<&Job>, weapon: &WeaponGear) -> HandInput {
    HandInput {
        class: weapon_class(weapon.weapon_type),
        atk: weapon.atk,
        matk: weapon.matk,
        refine: RefineInput {
            bonus: weapon.refine.bonus,
            random_bonus: weapon.refine.random_bonus,
        },
        aspd_base: aspd_base(job, weapon.weapon_type),
    }
}

pub(crate) fn gear_input(job: Option<&Job>, gear: &GearStatus) -> GearInput {
    GearInput {
        right: gear.right.as_ref().map_or_else(
            || HandInput::bare(aspd_base(job, WeaponType::Fist)),
            |w| hand_input(job, w),
        ),
        left: gear.left.as_ref().map(|w| hand_input(job, w)),
        shield: gear.shield,
        item_def: i32::try_from(gear.hard_def).unwrap_or(i32::MAX),
        armor_refine: gear.armor_refine,
        // Item MDEF and gear VIT/INT come with item bonuses (spec §5).
        item_mdef: 0,
        vit: 0,
        int: 0,
        range: gear.range,
    }
}

pub(crate) fn job_row(job: Option<&Job>, job_level: u16, base_level: u16) -> JobRow {
    let Some(job) = job else {
        return JobRow {
            max_weight: DEFAULT_MAX_WEIGHT,
            ..JobRow::default()
        };
    };
    let bonus = job.bonus(job_level);
    JobRow {
        bonus: Primary {
            str: bonus.str.into(),
            agi: bonus.agi.into(),
            vit: bonus.vit.into(),
            int: bonus.int.into(),
            dex: bonus.dex.into(),
            luk: bonus.luk.into(),
        },
        trait_bonus: Traits {
            pow: bonus.pow.into(),
            sta: bonus.sta.into(),
            wis: bonus.wis.into(),
            spl: bonus.spl.into(),
            con: bonus.con.into(),
            crt: bonus.crt.into(),
        },
        base_hp: job.base_hp(base_level),
        base_sp: job.base_sp(base_level),
        base_ap: job.base_ap(base_level),
        max_weight: job.max_weight,
        shield_aspd: job.shield_aspd(),
    }
}
