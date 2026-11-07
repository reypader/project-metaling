//! The player status calculation (catalog `mechanics/stats.md`; ro-engine DESIGN.md §9.4).
//! One pure rebuild from the inputs: base primary, battle primary, then every secondary
//! stat reading the layer rAthena reads. Integer formulas stay integer in rAthena's operation
//! order; the float ones (soft DEF/MDEF, the ASPD square root, the MaxHP/MaxSP percentages)
//! compute in `f64` and truncate once, as the C code does.

use crate::config::BL_PC;
use crate::{
    BattleConfig, BonusSlot, HandInput, Primary, Status, StatusInputs, Traits, WeaponClass,
};
/// The renewal stat clamp for HIT, FLEE and the DEF values (`SHRT_MAX`).
const SHORT_MAX: i32 = i16::MAX as i32;
/// `MIN_ASPD / 2`: the slowest amotion.
const MAX_AMOTION: i32 = 4000;
/// Player `dmotion`: `800 − 4 × AGI`, at least 400 (catalog `auto_attack.md` §3.2;
/// `player_damage_delay_rate` 100).
const PC_DMOTION_BASE: i32 = 800;
const PC_DMOTION_MIN: i32 = 400;
/// Above this ASPD base a weapon is treated as 200 (`min(base, 200)`).
const ASPD_BASE_CAP: i32 = 200;
/// Level brackets of the MaxHP caps.
const HP_CAP_LV99_BELOW: u16 = 100;
const HP_CAP_LV150_BELOW: u16 = 151;

/// Rebuilds a player's status from scratch. Pure: no allocation, no I/O.
pub fn calc_status(inputs: &StatusInputs<'_>, config: &BattleConfig) -> Status {
    let base_bonus = inputs.base_bonus;
    let battle_bonus = inputs.battle_bonus;
    let flat = |slot| base_bonus.get(slot).saturating_add(battle_bonus.get(slot));
    let gear = inputs.gear;

    let base = inputs
        .job
        .bonus
        .plus(inputs.allocated)
        .plus(Primary::from_bonus(base_bonus))
        .clamped();
    let battle = base.plus(Primary::from_bonus(battle_bonus)).clamped();
    let trait_base = inputs.job.trait_bonus.plus(inputs.traits).clamped();
    let trait_battle = trait_base;
    let Traits {
        pow,
        sta,
        wis,
        spl,
        con,
        crt,
    } = trait_battle;
    let level = i32::from(inputs.base_level);
    let Primary {
        str,
        agi,
        vit,
        int,
        dex,
        luk,
    } = battle;

    let ranged = gear.right.class.is_ranged();
    let batk = if config.enable_baseatk_renewal & BL_PC != 0 {
        let (main, other) = if ranged { (dex, str) } else { (str, dex) };
        (main * 10 + other * 10 / 5 + luk * 10 / 3 + level * 10 / 4) / 10 + 5 * pow
    } else {
        0
    } + flat(BonusSlot::BaseAtk);

    let hands = [Some(gear.right), gear.left];
    let refine_points = |hand: &HandInput| to_i32(hand.refine.bonus / 100);
    let mut watk = flat(BonusSlot::Atk);
    let mut weapon_matk = flat(BonusSlot::Matk);
    for hand in hands.iter().flatten() {
        watk += to_i32(hand.atk) + refine_points(hand);
        weapon_matk += to_i32(hand.matk);
        if gear.right.class != WeaponClass::Bow {
            weapon_matk += refine_points(hand);
        }
    }
    let overrefine = hands.map(|hand| {
        hand.map_or(0, |h| {
            u16::try_from(h.refine.random_bonus / 100).unwrap_or(u16::MAX)
        })
    });
    let status_matk = int + int / 2 + dex / 5 + luk / 3 + level / 4;

    let soft_def = (f64::from(level + vit) / 2.0 + f64::from(agi) / 5.0) as i32;
    let soft_def = (soft_def + flat(BonusSlot::Def2)).clamp(0, SHORT_MAX);
    let soft_mdef = (f64::from(int) + f64::from(level) / 4.0 + f64::from(dex + vit) / 5.0) as i32;
    let soft_mdef = (soft_mdef + flat(BonusSlot::Mdef2)).clamp(0, SHORT_MAX);
    let refine_def = to_i32((gear.armor_refine + 50) / 100);
    let hard_def = gear.item_def + refine_def + flat(BonusSlot::Def);
    let hard_mdef = gear.item_mdef + flat(BonusSlot::Mdef);

    let hit = (flat(BonusSlot::Hit) + level + dex + luk / 3 + 175 + 2 * con).clamp(1, SHORT_MAX);
    let flee = (flat(BonusSlot::Flee) + level + agi + luk / 5 + 100 + 2 * con).clamp(1, SHORT_MAX);
    let derived = |value: i32| value.clamp(0, SHORT_MAX);
    let patk = derived(pow / 3 + con / 5);
    let smatk = derived(spl / 3 + con / 5);
    let res = derived(sta + (sta / 3) * 5);
    let mres = derived(wis + (wis / 3) * 5);
    let hplus = derived(crt);
    let crate_ = derived(crt / 3);
    let cri = if config.enable_critical & BL_PC != 0 {
        let cri = (flat(BonusSlot::Critical) * 10 + level / 10 + 10 + 3 * luk).clamp(1, SHORT_MAX);
        if gear.right.class == WeaponClass::Katar {
            cri * 2
        } else {
            cri
        }
    } else {
        0
    };
    let flee2 = if config.enable_perfect_flee & BL_PC != 0 {
        (flat(BonusSlot::Flee2) * 10 + luk + 10).clamp(0, SHORT_MAX)
    } else {
        0
    };

    let amotion =
        amotion(inputs, ranged, agi, dex, flat(BonusSlot::AspdFixed)) + flat(BonusSlot::AspdAdd);
    let floor = to_i32(config.amotion_floor(&inputs.class));
    let amotion = amotion.clamp(floor, MAX_AMOTION);
    let adelay = 2 * amotion;
    let dmotion = (PC_DMOTION_BASE - agi * 4).clamp(PC_DMOTION_MIN, PC_DMOTION_BASE);

    let class_factor = inputs.class.upper || inputs.class.fourth;
    let max_hp = max_pool(
        inputs.job.base_hp,
        vit,
        class_factor,
        gear.vit + flat(BonusSlot::MaxHp),
        config.hp_rate,
    )
    .min(hp_cap(inputs.base_level, config));
    let max_sp = max_pool(
        inputs.job.base_sp,
        int,
        class_factor,
        gear.int + flat(BonusSlot::MaxSp),
        config.sp_rate,
    )
    .min(config.max_sp.max(1));

    let max_ap = max_ap(inputs.job.base_ap, config);

    let allocated_str = u32::try_from(inputs.allocated.str.max(0)).unwrap_or(0);
    let weight_bonus = flat(BonusSlot::MaxWeight);
    let max_weight =
        (inputs.job.max_weight + allocated_str * 300).saturating_add_signed(weight_bonus);
    let range_bonus = u16::try_from(flat(BonusSlot::Range).max(0)).unwrap_or(0);

    Status {
        base,
        battle,
        trait_base,
        trait_battle,
        batk,
        watk,
        status_matk,
        weapon_matk,
        soft_def,
        hard_def,
        soft_mdef,
        hard_mdef,
        hit,
        flee,
        flee2,
        cri,
        amotion,
        adelay,
        dmotion,
        max_hp,
        max_sp,
        patk,
        smatk,
        res,
        mres,
        hplus,
        crate_,
        max_ap,
        max_weight,
        range: gear.range.saturating_add(range_bonus),
        overrefine,
    }
}

/// Renewal amotion before the bonus and the clamps (catalog "ASPD (renewal)").
/// `agi` and `dex` are battle stats; `aspd_fixed` is the flat ASPD bonus, scaled by AGI.
fn amotion(inputs: &StatusInputs<'_>, ranged: bool, agi: i32, dex: i32, aspd_fixed: i32) -> i32 {
    let gear = inputs.gear;
    let mut base = i32::from(gear.right.aspd_base);
    if gear.shield {
        base += i32::from(inputs.job.shield_aspd);
    } else if let Some(left) = gear.left {
        base += i32::from(left.aspd_base) / 4;
    }
    let fixed = aspd_fixed * agi / 200;
    let (dex, agi) = (f64::from(dex), f64::from(agi));
    let dex_term = if ranged {
        dex * dex / 7.0
    } else {
        dex * dex / 5.0
    };
    let stat = (dex_term + agi * agi * 0.5).sqrt() * 0.25 + 196.0;
    let points = (stat + f64::from(fixed)) as i32 - base.min(ASPD_BASE_CAP);
    2000 - points * 10
}

/// MaxHP or MaxSP: table × (1 + stat %), ×1.25 for upper/4th classes, plus the flat part,
/// then the config rate (catalog "MaxHP / MaxSP").
fn max_pool(table: u32, stat: i32, class_factor: bool, flat: i32, rate: u32) -> u32 {
    let mut value = f64::from(table);
    if stat > 0 {
        value *= 1.0 + f64::from(stat) * 0.01;
    }
    if class_factor {
        value *= 1.25;
    }
    let value = (value as i64 + i64::from(flat)).max(1);
    let value = u32::try_from(value).unwrap_or(u32::MAX);
    if rate == 100 {
        value
    } else {
        (f64::from(rate) * (f64::from(value) / 100.0)) as u32
    }
    .max(1)
}

/// MaxAP: the table value (no bonuses yet), then the `ap_rate` percentage, capped at `max_ap`
/// (catalog `mechanics/trait_stats.md` §2).
fn max_ap(base_ap: u32, config: &BattleConfig) -> u32 {
    let value = if config.ap_rate == 100 {
        base_ap
    } else {
        (f64::from(config.ap_rate) * (f64::from(base_ap) / 100.0)) as u32
    };
    value.min(config.max_ap)
}

fn hp_cap(base_level: u16, config: &BattleConfig) -> u32 {
    if base_level < HP_CAP_LV99_BELOW {
        config.max_hp_lv99
    } else if base_level < HP_CAP_LV150_BELOW {
        config.max_hp_lv150
    } else {
        config.max_hp
    }
}

fn to_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
