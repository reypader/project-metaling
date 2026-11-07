//! Job identity: the client job id → rAthena mapid bits, and the item `Jobs` / `Classes` tests
//! (spec `specs/equipment/spec.md` §2.1; catalog `mechanics/equipment.md` §3a). The table is
//! data from the catalog. The status formulas read the class flags from it.

use ro_core::Sex;
use ro_mechanics::ClassFlags;

mod table;

use table::JOB_MAPIDS;

/// Base-class number (`mapid & 0xff`).
pub(crate) const BASE_MASK: u32 = 0xff;
/// 2-1 and 2-2 branch bits.
const JOBL_2_1: u32 = 0x100;
const JOBL_2_2: u32 = 0x200;
/// Either branch bit: a 2nd class (or a Super Novice).
pub(crate) const JOBL_2: u32 = JOBL_2_1 | JOBL_2_2;
/// Third class (also set on fourth classes).
pub(crate) const JOBL_THIRD: u32 = 0x1000;
pub(crate) const JOBL_FOURTH: u32 = 0x1_0000;
pub(crate) const JOBL_UPPER: u32 = 0x10_0000;
pub(crate) const JOBL_BABY: u32 = 0x20_0000;
/// Base class plus branch (rAthena `MAPID_SECONDMASK`).
pub(crate) const SECOND_MASK: u32 = 0xfff;
/// Base class, branch and third bit (rAthena `MAPID_THIRDMASK`).
pub(crate) const THIRD_MASK: u32 = 0xffff;

/// Second-class mapids of the dual-wield lines.
const MAPID_ASSASSIN: u32 = 0x106;
const MAPID_KAGEROU_OBORO: u32 = 0x109;
/// The first-class numbers of the six original lines (Swordman 1 .. Thief 6).
const MAPID_SWORDMAN: u32 = 0x1;
const MAPID_THIEF: u32 = 0x6;
/// Mapids of the extended lines (Rebellion, Kagerou/Oboro), compared under `SECOND_MASK`, so
/// Night Watch and Shinkiro/Shiranui count (catalog `mechanics/battle_config.md` §4, §8).
const MAPID_REBELLION: u32 = 0x108;
/// The Summoner line, under `BASE_MASK` (rAthena `MAPID_FIRSTMASK`; Spirit Handler `0x10a` is in it).
pub(crate) const MAPID_SUMMONER: u32 = 0xa;
/// Expanded jobs with trait stats but without the `JOBL_FOURTH` bit: Night Watch and
/// Shinkiro/Shiranui (under `THIRD_MASK`), Spirit Handler (under `SECOND_MASK`).
const MAPID_NIGHT_WATCH: u32 = 0x1108;
const MAPID_SHINKIRO_SHIRANUI: u32 = 0x1109;
const MAPID_SPIRIT_HANDLER: u32 = 0x10a;
/// The Super Novice line under `SECOND_MASK` (Super Novice, Super Baby, their expanded forms,
/// Hyper Novice).
pub(crate) const MAPID_SUPER_NOVICE: u32 = 0x100;
/// From this job level a Super Novice gets the +10 stats.
const SUPER_NOVICE_BONUS_JOB_LEVEL: u16 = 70;
/// Third-class mapids that need a Mado Gear or a cart for cannonballs.
const MAPID_MECHANIC: u32 = 0x1105;
const MAPID_GENETIC: u32 = 0x1205;

/// `ITEMJ_*` bits of an item's `Classes`.
const ITEMJ_NORMAL: u8 = 0x01;
const ITEMJ_UPPER: u8 = 0x02;
const ITEMJ_BABY: u8 = 0x04;
const ITEMJ_THIRD: u8 = 0x08;
const ITEMJ_THIRD_UPPER: u8 = 0x10;
const ITEMJ_THIRD_BABY: u8 = 0x20;
const ITEMJ_FOURTH: u8 = 0x40;

/// The mapid of a job id; Novice (0) for ids without one.
pub(crate) fn job_mapid(job: u16) -> u32 {
    JOB_MAPIDS
        .binary_search_by_key(&job, |&(id, _)| id)
        .map_or(0, |i| JOB_MAPIDS[i].1)
}

/// rAthena `pc_mapid2jobid`: the job with `mapid`, the female one of a male/female pair for a
/// female character. `None` when no job has that mapid.
pub(crate) fn mapid_job(mapid: u32, sex: Sex) -> Option<u16> {
    let (job, _) = JOB_MAPIDS.iter().find(|&&(_, m)| m == mapid)?;
    Some(job_for_sex(*job, sex))
}

/// `pc_mapid2jobid`'s sex pick (spec §2.6): the jobs that share a mapid are male/female pairs
/// (Bard/Dancer, Kagerou/Oboro, ...), the male one first. Other jobs are returned unchanged.
pub(crate) fn job_for_sex(job: u16, sex: Sex) -> u16 {
    let Ok(i) = JOB_MAPIDS.binary_search_by_key(&job, |&(id, _)| id) else {
        return job;
    };
    let mapid = JOB_MAPIDS[i].1;
    let mut pair = JOB_MAPIDS
        .iter()
        .filter(|&&(_, m)| m == mapid)
        .map(|&(id, _)| id);
    let (Some(male), Some(female)) = (pair.next(), pair.next()) else {
        return job;
    };
    match sex {
        Sex::Female => female,
        _ => male,
    }
}

/// `pc_is_primary_fourth`:the twelve 4th classes of the 2-1 and 2-2 lines (Dragon Knight ..
/// Shadow Cross, Imperial Guard .. Abyss Chaser). Hyper Novice, Sky Emperor and Soul Ascetic
/// carry `JOBL_FOURTH` too but aren't primary (capture 2: Hyper Novice has no ×1.25 MaxHP).
pub(crate) fn is_primary_fourth(mapid: u32) -> bool {
    let base = mapid & BASE_MASK;
    mapid & JOBL_FOURTH != 0
        && mapid & (JOBL_2_1 | JOBL_2_2) != 0
        && (MAPID_SWORDMAN..=MAPID_THIEF).contains(&base)
}

/// What the status formulas need to know about a job's class (catalog
/// `mechanics/battle_config.md` "pc_is_trait_job" and the max-parameter order).
pub(crate) fn class_flags(mapid: u32) -> ClassFlags {
    let third_line = mapid & THIRD_MASK;
    ClassFlags {
        upper: mapid & JOBL_UPPER != 0,
        baby: mapid & JOBL_BABY != 0,
        third: mapid & JOBL_THIRD != 0,
        fourth: is_primary_fourth(mapid),
        trait_job: mapid & JOBL_FOURTH != 0
            || matches!(third_line, MAPID_NIGHT_WATCH | MAPID_SHINKIRO_SHIRANUI)
            || mapid & SECOND_MASK == MAPID_SPIRIT_HANDLER,
        summoner: mapid & BASE_MASK == MAPID_SUMMONER,
        extended: matches!(mapid & SECOND_MASK, MAPID_REBELLION | MAPID_KAGEROU_OBORO),
    }
}

/// rAthena's Super Novice rule: all six stats +10 for the Super Novice line at job level 70 or
/// in a third/fourth form, while the character has never died (`die_counter == 0`; catalog
/// `mechanics/stats.md` "Hyper Novice"). Nobody dies yet, so the counter is always 0.
pub(crate) fn super_novice_bonus(mapid: u32, job_level: u16) -> bool {
    mapid & SECOND_MASK == MAPID_SUPER_NOVICE
        && (job_level >= SUPER_NOVICE_BONUS_JOB_LEVEL || mapid & JOBL_THIRD != 0)
}

/// The item `Jobs` test: the bit of the base class in the set of the job's branch.
pub(crate) fn job_can_use(jobs: &[u64; 3], mapid: u32) -> bool {
    let index = if mapid & JOBL_2_1 != 0 {
        1
    } else if mapid & JOBL_2_2 != 0 {
        2
    } else {
        0
    };
    let bit = 1u64 << (mapid & BASE_MASK).min(63);
    jobs[index] & bit != 0
}

/// The item `Classes` test (renewal): the one `ITEMJ_*` bit of the job's class must be set.
pub(crate) fn class_can_use(classes: u8, mapid: u32) -> bool {
    let third = mapid & JOBL_THIRD != 0;
    let needed = if mapid & JOBL_FOURTH != 0 {
        ITEMJ_FOURTH
    } else if third && mapid & JOBL_UPPER != 0 {
        ITEMJ_THIRD_UPPER
    } else if third && mapid & JOBL_BABY != 0 {
        ITEMJ_THIRD_BABY
    } else if third {
        ITEMJ_THIRD
    } else if mapid & JOBL_UPPER != 0 {
        ITEMJ_UPPER
    } else if mapid & JOBL_BABY != 0 {
        ITEMJ_BABY
    } else {
        ITEMJ_NORMAL
    };
    classes & needed != 0
}

/// The Assassin and Kagerou/Oboro lines (with their transcendent, baby, third and fourth forms)
/// may wear a dagger, one-hand sword or one-hand axe in either hand.
pub(crate) fn dual_wields(mapid: u32) -> bool {
    matches!(mapid & SECOND_MASK, MAPID_ASSASSIN | MAPID_KAGEROU_OBORO)
}

/// Mechanic forms: cannonballs need a Mado Gear, which doesn't exist here.
pub(crate) fn is_mechanic(mapid: u32) -> bool {
    mapid & THIRD_MASK == MAPID_MECHANIC
}

/// Genetic forms: cannonballs need a cart, which doesn't exist here.
pub(crate) fn is_genetic(mapid: u32) -> bool {
    mapid & THIRD_MASK == MAPID_GENETIC
}

#[cfg(test)]
mod tests;
