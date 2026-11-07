//! Converting one `skill_db.yml` entry to `SkillData`.

use std::collections::{BTreeMap, BTreeSet};

use ro_db::{
    CastTimeFlags, DamageFlags, SkillData, SkillElement, SkillFlags, SkillItemCost, WeaponType,
    MAX_SKILL_LEVEL,
};

use crate::level_expand::{expand, BadLevel};
use crate::raw_skill_dto::{Entry, PerLevel, RawElement, RawItemCost, RawSkill};
use crate::SLOTS;

pub(crate) fn convert_skill(raw: RawSkill, warnings: &mut Vec<String>) -> Option<SkillData> {
    let label = format!("skill {} ({})", raw.id, raw.name);
    if !(1..=MAX_SKILL_LEVEL).contains(&raw.max_level) {
        warnings.push(format!(
            "{label}: MaxLevel {} out of range, skipped",
            raw.max_level
        ));
        return None;
    }
    let inf = match raw.target_type.as_deref() {
        None | Some("Passive") => 0,
        Some("Attack") => 1,
        Some("Ground") => 2,
        Some("Self") => 4,
        Some("Support") => 16,
        Some("Trap") => 32,
        Some(other) => {
            warnings.push(format!(
                "{label}: unknown TargetType '{other}', using Passive"
            ));
            0
        }
    };
    let levels = usize::from(raw.max_level);
    let requires = raw.requires.unwrap_or_default();
    // Expands each per-level key; a bad level drops the whole skill, as in rAthena.
    let mut bad_level = None;
    let sp_cost = slots_of(requires.sp_cost.as_ref(), &mut bad_level);
    let hp_cost = slots_of(requires.hp_cost.as_ref(), &mut bad_level);
    let zeny_cost = slots_of(requires.zeny_cost.as_ref(), &mut bad_level);
    let range = slots_of(raw.range.as_ref(), &mut bad_level);
    let cast_time = slots_of(raw.cast_time.as_ref(), &mut bad_level);
    let fixed_cast_time = slots_of(raw.fixed_cast_time.as_ref(), &mut bad_level);
    let act_delay = slots_of(raw.after_cast_act_delay.as_ref(), &mut bad_level);
    let walk_delay = slots_of(raw.after_cast_walk_delay.as_ref(), &mut bad_level);
    let cooldown = slots_of(raw.cooldown.as_ref(), &mut bad_level);
    let hit_count = slots_of(raw.hit_count.as_ref(), &mut bad_level);
    let element = match raw.element.as_ref().map(|e| elements(e, &label, warnings)) {
        Some(Err(e)) => {
            bad_level.get_or_insert(e);
            None
        }
        Some(Ok(slots)) => Some(slots),
        None => None,
    };
    if let Some(e) = bad_level {
        warnings.push(format!("{label}: {e}, skipped"));
        return None;
    }
    let element = element
        .map(|slots| slots[..levels].to_vec())
        .filter(|list| list.iter().any(|e| *e != SkillElement::Neutral))
        .unwrap_or_default();
    let item_cost = requires
        .item_cost
        .into_iter()
        .filter_map(|cost| item_cost(cost, raw.max_level, &label, warnings))
        .collect();
    let state = requires
        .state
        .filter(|state| !state.eq_ignore_ascii_case("None"));
    let mut flags = SkillFlags::default();
    for (name, &on) in &raw.flags {
        let slot = match name.as_str() {
            "IsQuest" => &mut flags.quest,
            "IsWedding" => &mut flags.wedding,
            "IsSpirit" => &mut flags.spirit,
            "IsNpc" => &mut flags.npc,
            "IsGuild" => &mut flags.guild,
            "AlterRangeVulture" => &mut flags.alter_range_vulture,
            "AlterRangeSnakeEye" => &mut flags.alter_range_snake_eye,
            "AlterRangeRadius" => &mut flags.alter_range_radius,
            "AlterRangeResearchTrap" => &mut flags.alter_range_research_trap,
            "AlterRangeShadowJump" => &mut flags.alter_range_shadow_jump,
            _ => continue,
        };
        *slot = on;
    }
    let mut damage_flags = DamageFlags::default();
    for (name, &on) in &raw.damage_flags {
        let slot = match name.as_str() {
            "NoDamage" => &mut damage_flags.no_damage,
            "Splash" => &mut damage_flags.splash,
            "SplashSplit" => &mut damage_flags.splash_split,
            "IgnoreAtkCard" => &mut damage_flags.ignore_atk_card,
            "IgnoreElement" => &mut damage_flags.ignore_element,
            "IgnoreDefense" => &mut damage_flags.ignore_defense,
            "IgnoreFlee" => &mut damage_flags.ignore_flee,
            "IgnoreDefCard" => &mut damage_flags.ignore_def_card,
            "Critical" => &mut damage_flags.critical,
            "IgnoreLongCard" => &mut damage_flags.ignore_long_card,
            "SimpleDefense" => &mut damage_flags.simple_defense,
            other => {
                warnings.push(format!("{label}: unknown DamageFlags '{other}', ignored"));
                continue;
            }
        };
        *slot = on;
    }
    let mut cast_time_flags = CastTimeFlags::default();
    for (name, &on) in &raw.cast_time_flags {
        let slot = match name.as_str() {
            "IgnoreDex" => &mut cast_time_flags.ignore_dex,
            "IgnoreStatus" => &mut cast_time_flags.ignore_status,
            "IgnoreItemBonus" => &mut cast_time_flags.ignore_item_bonus,
            other => {
                warnings.push(format!("{label}: unknown CastTimeFlags '{other}', ignored"));
                continue;
            }
        };
        *slot = on;
    }
    let weapons = requires
        .weapon
        .map(|map| weapons(&map, &label, warnings))
        .unwrap_or_default();
    Some(SkillData {
        id: raw.id,
        name: raw.name,
        max_level: raw.max_level,
        inf,
        sp_cost: narrow(sp_cost, levels, &label, "SpCost", warnings),
        range: narrow(range, levels, &label, "Range", warnings),
        flags,
        cast_time: narrow(cast_time, levels, &label, "CastTime", warnings),
        fixed_cast_time: narrow(fixed_cast_time, levels, &label, "FixedCastTime", warnings),
        after_cast_act_delay: narrow(act_delay, levels, &label, "AfterCastActDelay", warnings),
        after_cast_walk_delay: narrow(walk_delay, levels, &label, "AfterCastWalkDelay", warnings),
        cooldown: narrow(cooldown, levels, &label, "Cooldown", warnings),
        hit_count: narrow(hit_count, levels, &label, "HitCount", warnings),
        element,
        damage_flags,
        cast_time_flags,
        hp_cost: narrow(hp_cost, levels, &label, "HpCost", warnings),
        zeny_cost: narrow(zeny_cost, levels, &label, "ZenyCost", warnings),
        weapons,
        item_cost,
        state,
    })
}

/// A per-level key's 13 slots; `None` when absent. A bad level is kept in `bad` (first wins).
fn slots_of<T: Entry>(
    key: Option<&PerLevel<T>>,
    bad: &mut Option<BadLevel>,
) -> Option<[i32; SLOTS]> {
    let result = match key? {
        PerLevel::Scalar(value) => Ok([*value; SLOTS]),
        PerLevel::List(list) => expand(&list.iter().map(Entry::pair).collect::<Vec<_>>()),
    };
    match result {
        Ok(slots) => Some(slots),
        Err(e) => {
            bad.get_or_insert(e);
            None
        }
    }
}

/// `Element` slots. A list sets the listed levels; unlisted levels below the highest listed one
/// stay Neutral (rAthena's zeroed slot), and the levels above it repeat it. Element codes aren't
/// numbers to extrapolate, so the `expand` series rule doesn't apply.
fn elements(
    raw: &RawElement,
    label: &str,
    warnings: &mut Vec<String>,
) -> Result<[SkillElement; SLOTS], BadLevel> {
    let mut named = |name: &str| {
        skill_element(name).unwrap_or_else(|| {
            warnings.push(format!("{label}: unknown Element '{name}', using Neutral"));
            SkillElement::Neutral
        })
    };
    let mut out = [SkillElement::Neutral; SLOTS];
    match raw {
        RawElement::Scalar(name) => out = [named(name); SLOTS],
        RawElement::List(list) => {
            let mut top = 0;
            for entry in list {
                let slot = usize::from(entry.level);
                if !(1..=SLOTS).contains(&slot) {
                    return Err(BadLevel(entry.level));
                }
                out[slot - 1] = named(&entry.element);
                top = top.max(slot);
            }
            if top > 0 {
                let last = out[top - 1];
                out[top..].fill(last);
            }
        }
    }
    Ok(out)
}

fn skill_element(name: &str) -> Option<SkillElement> {
    const NAMES: [(&str, SkillElement); 13] = [
        ("Neutral", SkillElement::Neutral),
        ("Water", SkillElement::Water),
        ("Earth", SkillElement::Earth),
        ("Fire", SkillElement::Fire),
        ("Wind", SkillElement::Wind),
        ("Poison", SkillElement::Poison),
        ("Holy", SkillElement::Holy),
        ("Dark", SkillElement::Dark),
        ("Ghost", SkillElement::Ghost),
        ("Undead", SkillElement::Undead),
        ("Weapon", SkillElement::Weapon),
        ("Endowed", SkillElement::Endowed),
        ("Random", SkillElement::Random),
    ];
    NAMES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, e)| *e)
}

/// `Requires.Weapon` as a sorted set; empty = any weapon (`All: true`, or every key false).
fn weapons(
    map: &BTreeMap<String, bool>,
    label: &str,
    warnings: &mut Vec<String>,
) -> Vec<WeaponType> {
    const NAMES: [(&str, WeaponType); 24] = [
        ("Fist", WeaponType::Fist),
        ("Dagger", WeaponType::Dagger),
        ("1hSword", WeaponType::OneHandSword),
        ("2hSword", WeaponType::TwoHandSword),
        ("1hSpear", WeaponType::OneHandSpear),
        ("2hSpear", WeaponType::TwoHandSpear),
        ("1hAxe", WeaponType::OneHandAxe),
        ("2hAxe", WeaponType::TwoHandAxe),
        ("Mace", WeaponType::Mace),
        ("2hMace", WeaponType::TwoHandMace),
        ("Staff", WeaponType::Staff),
        ("Bow", WeaponType::Bow),
        ("Knuckle", WeaponType::Knuckle),
        ("Musical", WeaponType::Musical),
        ("Whip", WeaponType::Whip),
        ("Book", WeaponType::Book),
        ("Katar", WeaponType::Katar),
        ("Revolver", WeaponType::Revolver),
        ("Rifle", WeaponType::Rifle),
        ("Gatling", WeaponType::Gatling),
        ("Shotgun", WeaponType::Shotgun),
        ("Grenade", WeaponType::Grenade),
        ("Huuma", WeaponType::Huuma),
        ("2hStaff", WeaponType::TwoHandStaff),
    ];
    if map.get("All").copied().unwrap_or(false) {
        return Vec::new();
    }
    let mut set = BTreeSet::new();
    for (name, &on) in map {
        if name == "All" {
            continue;
        }
        let Some(&(_, weapon)) = NAMES.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) else {
            warnings.push(format!("{label}: Weapon '{name}' not supported, ignored"));
            continue;
        };
        if on {
            set.insert(weapon);
        } else {
            set.remove(&weapon);
        }
    }
    set.into_iter().collect()
}

pub(crate) fn item_cost(
    raw: RawItemCost,
    max_level: u8,
    label: &str,
    warnings: &mut Vec<String>,
) -> Option<SkillItemCost> {
    let Ok(amount) = u32::try_from(raw.amount) else {
        warnings.push(format!(
            "{label}: ItemCost {} amount {}, dropped",
            raw.item, raw.amount
        ));
        return None;
    };
    let level = match raw.level {
        None => None,
        Some(level) => match u8::try_from(level) {
            Ok(level) if (1..=max_level).contains(&level) => Some(level),
            _ => {
                warnings.push(format!(
                    "{label}: ItemCost {} level {level}, dropped",
                    raw.item
                ));
                return None;
            }
        },
    };
    Some(SkillItemCost {
        item: raw.item,
        amount,
        level,
    })
}

/// The first `levels` slots in the target type; empty when they are all 0.
fn narrow<T: TryFrom<i32> + Copy + Default + PartialEq>(
    slots: Option<[i32; SLOTS]>,
    levels: usize,
    label: &str,
    key: &str,
    warnings: &mut Vec<String>,
) -> Vec<T> {
    let Some(slots) = slots else {
        return Vec::new();
    };
    let values: Vec<T> = slots[..levels]
        .iter()
        .map(|&value| {
            T::try_from(value).unwrap_or_else(|_| {
                warnings.push(format!("{label}: {key} {value} out of range, using 0"));
                T::default()
            })
        })
        .collect();
    if values.iter().all(|value| *value == T::default()) {
        Vec::new()
    } else {
        values
    }
}
