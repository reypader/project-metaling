//! Building one `MobData` from a merged rAthena entry.

use std::collections::BTreeMap;

use ro_db::{
    Element, ItemDb, MobClass, MobData, MobDrop, MobId, MobMode, MobModes, MobRace, MobSize,
    DEFAULT_MOB_MOTION, DEFAULT_MOB_WALK_SPEED, MAX_DROP_RATE, MAX_ELEMENT_LEVEL, MAX_MOB_DROPS,
    MAX_MVP_DROPS,
};

use crate::raw_dto::{RawDrop, RawMob};

/// rAthena `MIN_WALK_SPEED`, `MAX_WALK_SPEED` (`DEFAULT_MOB_WALK_SPEED` is the default).
const MIN_WALK_SPEED: u16 = 20;
const MAX_WALK_SPEED: u16 = 1000;
/// rAthena `MIN_ASPD`: the cap of `AttackDelay` and `AttackMotion` for mobs. The floor of
/// `AttackDelay` and both defaults are `DEFAULT_MOB_MOTION` (`MAX_ASPD_NOPC`).
const MAX_MOB_MOTION: u16 = 8000;

/// rAthena's `MONSTER_TYPE_<Ai>` mode masks (catalog `db/mob_db.md` "Mode bits and Ai table").
/// An unknown value is `06` (no bits).
const AI_MODES: [(&str, u32); 23] = [
    ("01", 0x81),
    ("02", 0x83),
    ("03", 0x1089),
    ("04", 0x3885),
    ("05", 0x2085),
    ("06", 0),
    ("07", 0x108B),
    ("08", 0x7085),
    ("09", 0x3095),
    ("10", 0x84),
    ("11", 0x84),
    ("12", 0x2085),
    ("13", 0x308D),
    ("17", 0x91),
    ("19", 0x3095),
    ("20", 0x3295),
    ("21", 0x3695),
    ("24", 0xA1),
    ("25", 0x1),
    ("26", 0xB695),
    ("27", 0x8084),
    ("ABR_PASSIVE", 0x21),
    ("ABR_OFFENSIVE", 0xA5),
];

const SIZES: [MobSize; 3] = [MobSize::Small, MobSize::Medium, MobSize::Large];
const RACES: [MobRace; 12] = [
    MobRace::Formless,
    MobRace::Undead,
    MobRace::Brute,
    MobRace::Plant,
    MobRace::Insect,
    MobRace::Fish,
    MobRace::Demon,
    MobRace::Demihuman,
    MobRace::Angel,
    MobRace::Dragon,
    MobRace::PlayerHuman,
    MobRace::PlayerDoram,
];
const ELEMENTS: [Element; 10] = [
    Element::Neutral,
    Element::Water,
    Element::Earth,
    Element::Fire,
    Element::Wind,
    Element::Poison,
    Element::Holy,
    Element::Dark,
    Element::Ghost,
    Element::Undead,
];

/// Case-insensitive, ignoring `_` (`Player_Human` is `PlayerHuman`).
fn same_name(a: &str, b: &str) -> bool {
    let fold = |s: &str| {
        s.chars()
            .filter(|c| *c != '_')
            .map(|c| c.to_ascii_lowercase())
            .collect::<String>()
    };
    fold(a) == fold(b)
}

/// Looks `name` up among `all` by their `Debug` names; unknown names fall back to the default
/// with a warning, as rAthena does.
fn named<T: Copy + Default + std::fmt::Debug>(
    id: u32,
    field: &str,
    name: Option<&str>,
    all: &[T],
    warnings: &mut Vec<String>,
) -> T {
    let Some(name) = name else {
        return T::default();
    };
    all.iter()
        .copied()
        .find(|value| same_name(&format!("{value:?}"), name))
        .unwrap_or_else(|| {
            warnings.push(format!(
                "mob {id}: unknown {field} '{name}', using the default"
            ));
            T::default()
        })
}

/// The modes `Class` adds after everything else.
fn class_modes(class: MobClass) -> &'static [MobMode] {
    match class {
        MobClass::Boss => &[
            MobMode::Detector,
            MobMode::StatusImmune,
            MobMode::KnockbackImmune,
        ],
        MobClass::Guardian => &[MobMode::StatusImmune],
        MobClass::Battlefield => &[MobMode::StatusImmune, MobMode::SkillImmune],
        MobClass::Event => &[MobMode::FixedItemDrop],
        MobClass::Normal => &[],
    }
}

/// `Ai` mask, then the `Modes` toggles, then the `Class` additions. No `Ai` is mask 0.
fn build_modes(
    id: u32,
    ai: Option<&str>,
    toggles: Option<&BTreeMap<String, bool>>,
    class: MobClass,
    warnings: &mut Vec<String>,
) -> MobModes {
    let mask = ai.map_or(0, |key| {
        AI_MODES
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map_or_else(
                || {
                    warnings.push(format!("mob {id}: unknown Ai '{key}', using 06"));
                    0
                },
                |&(_, mask)| mask,
            )
    });
    let mut modes = MobModes::from_bits(mask);
    for (name, &on) in toggles.into_iter().flatten() {
        match MobMode::from_name(name) {
            Some(mode) => modes.set(mode, on),
            None => warnings.push(format!("mob {id}: unknown mode '{name}', skipped")),
        }
    }
    for &mode in class_modes(class) {
        modes.set(mode, true);
    }
    modes
}

fn mob_class(name: &str) -> Option<MobClass> {
    Some(match name.to_ascii_lowercase().as_str() {
        "normal" => MobClass::Normal,
        "boss" => MobClass::Boss,
        "guardian" => MobClass::Guardian,
        "battlefield" => MobClass::Battlefield,
        "event" => MobClass::Event,
        _ => return None,
    })
}

/// Places drops into `max` slots: `Index` overwrites that slot, otherwise the first free slot.
/// Unknown items are skipped; rate 0 entries are removed afterwards; rates are capped at 10000.
fn resolve_drops(
    mob_id: u32,
    list: &[RawDrop],
    max: usize,
    items: &ItemDb,
    warnings: &mut Vec<String>,
) -> Vec<MobDrop> {
    let mut slots: Vec<Option<MobDrop>> = vec![None; max];
    for raw in list {
        let Some(item) = items.by_aegis_name(&raw.item) else {
            warnings.push(format!("mob {mob_id}: unknown drop item '{}'", raw.item));
            continue;
        };
        let rate = u16::try_from(raw.rate.min(u32::from(MAX_DROP_RATE))).unwrap_or(MAX_DROP_RATE);
        let drop = MobDrop {
            item: item.id,
            rate,
        };
        let slot = match raw.index {
            Some(i) if i < max => Some(i),
            Some(i) => {
                warnings.push(format!("mob {mob_id}: drop Index {i} out of range"));
                None
            }
            None => slots.iter().position(Option::is_none).or_else(|| {
                warnings.push(format!("mob {mob_id}: more than {max} drops"));
                None
            }),
        };
        if let Some(slot) = slot.and_then(|i| slots.get_mut(i)) {
            *slot = Some(drop);
        }
    }
    slots.into_iter().flatten().filter(|d| d.rate > 0).collect()
}

pub(crate) fn build(raw: RawMob, items: &ItemDb, warnings: &mut Vec<String>) -> Option<MobData> {
    let id = raw.id;
    if !MobId(id).is_valid() {
        warnings.push(format!("mob {id}: id outside the mob id ranges, skipped"));
        return None;
    }
    let (Some(aegis_name), Some(name)) = (raw.aegis_name, raw.name) else {
        warnings.push(format!("mob {id}: no AegisName or Name, skipped"));
        return None;
    };
    let class = match raw.class.as_deref() {
        None => MobClass::Normal,
        Some(c) => mob_class(c).unwrap_or_else(|| {
            warnings.push(format!("mob {id}: unknown Class '{c}', using Normal"));
            MobClass::Normal
        }),
    };
    let modes = build_modes(id, raw.ai.as_deref(), raw.modes.as_ref(), class, warnings);
    let element_level = match raw.element_level {
        None => 1,
        Some(level) => u8::try_from(level)
            .ok()
            .filter(|l| (1..=MAX_ELEMENT_LEVEL).contains(l))
            .unwrap_or_else(|| {
                warnings.push(format!("mob {id}: ElementLevel {level} invalid, using 1"));
                1
            }),
    };
    let attack_motion = raw
        .attack_motion
        .unwrap_or(DEFAULT_MOB_MOTION)
        .clamp(1, MAX_MOB_MOTION);
    let attack_delay = raw
        .attack_delay
        .unwrap_or(DEFAULT_MOB_MOTION)
        .clamp(DEFAULT_MOB_MOTION, MAX_MOB_MOTION)
        .max(attack_motion);
    let defense = i16::try_from(raw.defense.unwrap_or(0)).unwrap_or(i16::MAX);
    let stat = |value: Option<u16>| value.unwrap_or(1);
    Some(MobData {
        id: MobId(id),
        aegis_name,
        name,
        level: raw.level.unwrap_or(1),
        hp: raw.hp.unwrap_or(1),
        base_exp: raw.base_exp.unwrap_or(0),
        job_exp: raw.job_exp.unwrap_or(0),
        attack: raw.attack.unwrap_or(0),
        defense,
        resistance: raw.resistance.unwrap_or(0),
        str: stat(raw.str),
        agi: stat(raw.agi),
        vit: stat(raw.vit),
        int: stat(raw.int),
        dex: stat(raw.dex),
        luk: stat(raw.luk),
        attack_range: raw.attack_range.unwrap_or(0),
        size: named(id, "Size", raw.size.as_deref(), &SIZES, warnings),
        race: named(id, "Race", raw.race.as_deref(), &RACES, warnings),
        element: named(id, "Element", raw.element.as_deref(), &ELEMENTS, warnings),
        element_level,
        walk_speed: raw
            .walk_speed
            .unwrap_or(DEFAULT_MOB_WALK_SPEED)
            .clamp(MIN_WALK_SPEED, MAX_WALK_SPEED),
        attack_delay,
        attack_motion,
        damage_motion: raw.damage_motion.unwrap_or(0),
        class,
        modes,
        drops: resolve_drops(id, &raw.drops, MAX_MOB_DROPS, items, warnings),
        mvp_drops: resolve_drops(id, &raw.mvp_drops, MAX_MVP_DROPS, items, warnings),
    })
}
