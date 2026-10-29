//! Job identity: the client job id → rAthena mapid bits, and the item `Jobs` / `Classes` tests
//! (spec `specs/equipment/spec.md` §2.1; catalog `mechanics/equipment.md` §3a). The table is
//! data from the catalog. The status formulas read the class flags from it.

use ro_core::Sex;
use ro_mechanics::ClassFlags;

/// Base-class number (`mapid & 0xff`).
const BASE_MASK: u32 = 0xff;
/// 2-1 and 2-2 branch bits.
const JOBL_2_1: u32 = 0x100;
const JOBL_2_2: u32 = 0x200;
/// Third class (also set on fourth classes).
const JOBL_THIRD: u32 = 0x1000;
const JOBL_FOURTH: u32 = 0x1_0000;
const JOBL_UPPER: u32 = 0x10_0000;
const JOBL_BABY: u32 = 0x20_0000;
/// Base class plus branch (rAthena `MAPID_SECONDMASK`).
const SECOND_MASK: u32 = 0xfff;
/// Base class, branch and third bit (rAthena `MAPID_THIRDMASK`).
const THIRD_MASK: u32 = 0xffff;

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
const MAPID_SUMMONER: u32 = 0xa;
/// Expanded jobs with trait stats but without the `JOBL_FOURTH` bit: Night Watch and
/// Shinkiro/Shiranui (under `THIRD_MASK`), Spirit Handler (under `SECOND_MASK`).
const MAPID_NIGHT_WATCH: u32 = 0x1108;
const MAPID_SHINKIRO_SHIRANUI: u32 = 0x1109;
const MAPID_SPIRIT_HANDLER: u32 = 0x10a;
/// The Super Novice line under `SECOND_MASK` (Super Novice, Super Baby, their expanded forms,
/// Hyper Novice).
const MAPID_SUPER_NOVICE: u32 = 0x100;
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

/// Every job id with a mapid, sorted by id. Ids not listed (mounted variants like 13) act as
/// Novice, as rAthena resets them.
const JOB_MAPIDS: [(u16, u32); 154] = [
    (0, 0x0),          // NOVICE
    (1, 0x1),          // SWORDMAN
    (2, 0x2),          // MAGE
    (3, 0x3),          // ARCHER
    (4, 0x4),          // ACOLYTE
    (5, 0x5),          // MERCHANT
    (6, 0x6),          // THIEF
    (7, 0x101),        // KNIGHT
    (8, 0x104),        // PRIEST
    (9, 0x102),        // WIZARD
    (10, 0x105),       // BLACKSMITH
    (11, 0x103),       // HUNTER
    (12, 0x106),       // ASSASSIN
    (14, 0x201),       // CRUSADER
    (15, 0x204),       // MONK
    (16, 0x202),       // SAGE
    (17, 0x206),       // ROGUE
    (18, 0x205),       // ALCHEMIST
    (19, 0x203),       // BARD
    (20, 0x203),       // DANCER
    (22, 0xc),         // WEDDING
    (23, 0x100),       // SUPER_NOVICE
    (24, 0x8),         // GUNSLINGER
    (25, 0x9),         // NINJA
    (26, 0xd),         // XMAS
    (27, 0xe),         // SUMMER
    (28, 0xf),         // HANBOK
    (29, 0x10),        // OKTOBERFEST
    (30, 0x11),        // SUMMER2
    (4001, 0x10_0000), // NOVICE_HIGH
    (4002, 0x10_0001), // SWORDMAN_HIGH
    (4003, 0x10_0002), // MAGE_HIGH
    (4004, 0x10_0003), // ARCHER_HIGH
    (4005, 0x10_0004), // ACOLYTE_HIGH
    (4006, 0x10_0005), // MERCHANT_HIGH
    (4007, 0x10_0006), // THIEF_HIGH
    (4008, 0x10_0101), // LORD_KNIGHT
    (4009, 0x10_0104), // HIGH_PRIEST
    (4010, 0x10_0102), // HIGH_WIZARD
    (4011, 0x10_0105), // WHITESMITH
    (4012, 0x10_0103), // SNIPER
    (4013, 0x10_0106), // ASSASSIN_CROSS
    (4015, 0x10_0201), // PALADIN
    (4016, 0x10_0204), // CHAMPION
    (4017, 0x10_0202), // PROFESSOR
    (4018, 0x10_0206), // STALKER
    (4019, 0x10_0205), // CREATOR
    (4020, 0x10_0203), // CLOWN
    (4021, 0x10_0203), // GYPSY
    (4023, 0x20_0000), // BABY
    (4024, 0x20_0001), // BABY_SWORDMAN
    (4025, 0x20_0002), // BABY_MAGE
    (4026, 0x20_0003), // BABY_ARCHER
    (4027, 0x20_0004), // BABY_ACOLYTE
    (4028, 0x20_0005), // BABY_MERCHANT
    (4029, 0x20_0006), // BABY_THIEF
    (4030, 0x20_0101), // BABY_KNIGHT
    (4031, 0x20_0104), // BABY_PRIEST
    (4032, 0x20_0102), // BABY_WIZARD
    (4033, 0x20_0105), // BABY_BLACKSMITH
    (4034, 0x20_0103), // BABY_HUNTER
    (4035, 0x20_0106), // BABY_ASSASSIN
    (4037, 0x20_0201), // BABY_CRUSADER
    (4038, 0x20_0204), // BABY_MONK
    (4039, 0x20_0202), // BABY_SAGE
    (4040, 0x20_0206), // BABY_ROGUE
    (4041, 0x20_0205), // BABY_ALCHEMIST
    (4042, 0x20_0203), // BABY_BARD
    (4043, 0x20_0203), // BABY_DANCER
    (4045, 0x20_0100), // SUPER_BABY
    (4046, 0x7),       // TAEKWON
    (4047, 0x107),     // STAR_GLADIATOR
    (4049, 0x207),     // SOUL_LINKER
    (4050, 0xb),       // GANGSI
    (4051, 0x10b),     // DEATH_KNIGHT
    (4052, 0x20b),     // DARK_COLLECTOR
    (4054, 0x1101),    // RUNE_KNIGHT
    (4055, 0x1102),    // WARLOCK
    (4056, 0x1103),    // RANGER
    (4057, 0x1104),    // ARCH_BISHOP
    (4058, 0x1105),    // MECHANIC
    (4059, 0x1106),    // GUILLOTINE_CROSS
    (4060, 0x10_1101), // RUNE_KNIGHT_T
    (4061, 0x10_1102), // WARLOCK_T
    (4062, 0x10_1103), // RANGER_T
    (4063, 0x10_1104), // ARCH_BISHOP_T
    (4064, 0x10_1105), // MECHANIC_T
    (4065, 0x10_1106), // GUILLOTINE_CROSS_T
    (4066, 0x1201),    // ROYAL_GUARD
    (4067, 0x1202),    // SORCERER
    (4068, 0x1203),    // MINSTREL
    (4069, 0x1203),    // WANDERER
    (4070, 0x1204),    // SURA
    (4071, 0x1205),    // GENETIC
    (4072, 0x1206),    // SHADOW_CHASER
    (4073, 0x10_1201), // ROYAL_GUARD_T
    (4074, 0x10_1202), // SORCERER_T
    (4075, 0x10_1203), // MINSTREL_T
    (4076, 0x10_1203), // WANDERER_T
    (4077, 0x10_1204), // SURA_T
    (4078, 0x10_1205), // GENETIC_T
    (4079, 0x10_1206), // SHADOW_CHASER_T
    (4096, 0x20_1101), // BABY_RUNE_KNIGHT
    (4097, 0x20_1102), // BABY_WARLOCK
    (4098, 0x20_1103), // BABY_RANGER
    (4099, 0x20_1104), // BABY_ARCH_BISHOP
    (4100, 0x20_1105), // BABY_MECHANIC
    (4101, 0x20_1106), // BABY_GUILLOTINE_CROSS
    (4102, 0x20_1201), // BABY_ROYAL_GUARD
    (4103, 0x20_1202), // BABY_SORCERER
    (4104, 0x20_1203), // BABY_MINSTREL
    (4105, 0x20_1203), // BABY_WANDERER
    (4106, 0x20_1204), // BABY_SURA
    (4107, 0x20_1205), // BABY_GENETIC
    (4108, 0x20_1206), // BABY_SHADOW_CHASER
    (4190, 0x1100),    // SUPER_NOVICE_E
    (4191, 0x20_1100), // SUPER_BABY_E
    (4211, 0x109),     // KAGEROU
    (4212, 0x109),     // OBORO
    (4215, 0x108),     // REBELLION
    (4218, 0xa),       // SUMMONER
    (4220, 0x20_000a), // BABY_SUMMONER
    (4222, 0x20_0009), // BABY_NINJA
    (4223, 0x20_0109), // BABY_KAGEROU
    (4224, 0x20_0109), // BABY_OBORO
    (4225, 0x20_0007), // BABY_TAEKWON
    (4226, 0x20_0107), // BABY_STAR_GLADIATOR
    (4227, 0x20_0207), // BABY_SOUL_LINKER
    (4228, 0x20_0008), // BABY_GUNSLINGER
    (4229, 0x20_0108), // BABY_REBELLION
    (4239, 0x1107),    // STAR_EMPEROR
    (4240, 0x1207),    // SOUL_REAPER
    (4241, 0x20_1107), // BABY_STAR_EMPEROR
    (4242, 0x20_1207), // BABY_SOUL_REAPER
    (4252, 0x1_1101),  // DRAGON_KNIGHT
    (4253, 0x1_1105),  // MEISTER
    (4254, 0x1_1106),  // SHADOW_CROSS
    (4255, 0x1_1102),  // ARCH_MAGE
    (4256, 0x1_1104),  // CARDINAL
    (4257, 0x1_1103),  // WINDHAWK
    (4258, 0x1_1201),  // IMPERIAL_GUARD
    (4259, 0x1_1205),  // BIOLO
    (4260, 0x1_1206),  // ABYSS_CHASER
    (4261, 0x1_1202),  // ELEMENTAL_MASTER
    (4262, 0x1_1204),  // INQUISITOR
    (4263, 0x1_1203),  // TROUBADOUR
    (4264, 0x1_1203),  // TROUVERE
    (4302, 0x1_1107),  // SKY_EMPEROR
    (4303, 0x1_1207),  // SOUL_ASCETIC
    (4304, 0x1109),    // SHINKIRO
    (4305, 0x1109),    // SHIRANUI
    (4306, 0x1108),    // NIGHT_WATCH
    (4307, 0x1_1100),  // HYPER_NOVICE
    (4308, 0x10a),     // SPIRIT_HANDLER
];

/// The mapid of a job id; Novice (0) for ids without one.
pub(crate) fn job_mapid(job: u16) -> u32 {
    JOB_MAPIDS
        .binary_search_by_key(&job, |&(id, _)| id)
        .map_or(0, |i| JOB_MAPIDS[i].1)
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
fn is_primary_fourth(mapid: u32) -> bool {
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
mod tests {
    use super::*;

    /// `trait_job` matches catalog `mechanics/trait_stats.md` §1 "Trait job set" for every
    /// shipped job: the primary 4th classes, Hyper Novice, Sky Emperor, Soul Ascetic, Night
    /// Watch, Shinkiro/Shiranui and Spirit Handler.
    #[test]
    fn trait_job_matches_the_catalog_set() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ro-db/data/job");
        let jobs = ro_db::JobDb::load_dir(&dir).expect("shipped job data");
        let trait_jobs = [
            "Dragon_Knight",
            "Meister",
            "Shadow_Cross",
            "Arch_Mage",
            "Cardinal",
            "Windhawk",
            "Imperial_Guard",
            "Biolo",
            "Abyss_Chaser",
            "Elemental_Master",
            "Inquisitor",
            "Troubadour",
            "Trouvere",
            "Hyper_Novice",
            "Sky_Emperor",
            "Soul_Ascetic",
            "Night_Watch",
            "Shinkiro",
            "Shiranui",
            "Spirit_Handler",
        ];
        let wrong: Vec<(&str, bool)> = jobs
            .iter()
            .map(|job| (job.name.as_str(), class_flags(job_mapid(job.id)).trait_job))
            .filter(|(name, flag)| *flag != trait_jobs.contains(name))
            .collect();
        assert!(wrong.is_empty(), "(job, trait_job) disagreeing: {wrong:?}");
        for name in trait_jobs {
            assert!(jobs.by_name(name).is_some(), "{name} is shipped");
        }
    }

    #[test]
    fn table_is_sorted_and_unique() {
        assert!(JOB_MAPIDS.windows(2).all(|w| w[0].0 < w[1].0));
    }

    /// Spec §2.6: the sex picks the job of a shared mapid; no mapid is shared by more than two.
    #[test]
    fn job_for_sex_swaps_the_pairs() {
        for (job, male, female) in [
            (19, 19, 20),
            (20, 19, 20),
            (4212, 4211, 4212),
            (4305, 4304, 4305),
        ] {
            assert_eq!(job_for_sex(job, Sex::Male), male, "{job}");
            assert_eq!(job_for_sex(job, Sex::Female), female, "{job}");
        }
        assert_eq!(job_for_sex(1, Sex::Female), 1);
        assert_eq!(job_for_sex(65_000, Sex::Female), 65_000, "no mapid");
        for &(_, mapid) in &JOB_MAPIDS {
            let shared = JOB_MAPIDS.iter().filter(|&&(_, m)| m == mapid).count();
            assert!(shared <= 2, "{mapid:#x}");
        }
    }

    #[test]
    fn mapids_follow_the_catalog() {
        assert_eq!(job_mapid(0), 0);
        assert_eq!(job_mapid(12), 0x106, "Assassin");
        assert_eq!(job_mapid(4008), 0x10_0101, "Lord Knight");
        assert_eq!(job_mapid(4035), 0x20_0106, "Baby Assassin");
        assert_eq!(job_mapid(4059), 0x1106, "Guillotine Cross");
        assert_eq!(job_mapid(4252), 0x1_1101, "Dragon Knight");
        assert_eq!(job_mapid(4211), 0x109, "Kagerou");
        assert_eq!(job_mapid(20), job_mapid(19), "Dancer shares Bard's mapid");
    }

    #[test]
    fn unmapped_ids_act_as_novice() {
        for id in [13, 21, 31, 4014, 4048, 4219, 9999] {
            assert_eq!(job_mapid(id), 0, "job {id}");
        }
    }

    /// Catalog §3a: Knight → index 1 bit 1, Crusader → index 2 bit 1, Novice → index 0 bit 0;
    /// third and fourth forms test their second class's bit.
    #[test]
    fn job_test_uses_branch_and_base_bit() {
        let knight_only = [0, 1 << 1, 0];
        assert!(job_can_use(&knight_only, job_mapid(7)));
        assert!(job_can_use(&knight_only, job_mapid(4054)), "Rune Knight");
        assert!(job_can_use(&knight_only, job_mapid(4252)), "Dragon Knight");
        assert!(!job_can_use(&knight_only, job_mapid(14)), "Crusader");
        assert!(!job_can_use(&knight_only, job_mapid(1)), "Swordman");
        assert!(job_can_use(&[0, 0, 1 << 1], job_mapid(14)));
        assert!(job_can_use(&[1, 0, 0], job_mapid(0)));
        assert!(job_can_use(&[1, 0, 0], job_mapid(4001)), "Novice High");
        assert!(
            !job_can_use(&[1, 0, 0], job_mapid(23)),
            "Super Novice is 2-1"
        );
    }

    #[test]
    fn class_test_needs_the_exact_class_bit() {
        assert!(class_can_use(ITEMJ_NORMAL, job_mapid(0)));
        assert!(
            !class_can_use(ITEMJ_UPPER, job_mapid(0)),
            "transcendent-only item, novice"
        );
        assert!(class_can_use(ITEMJ_UPPER, job_mapid(4008)));
        assert!(
            !class_can_use(ITEMJ_NORMAL, job_mapid(4008)),
            "no fall-through"
        );
        assert!(class_can_use(ITEMJ_BABY, job_mapid(4035)));
        assert!(class_can_use(ITEMJ_THIRD, job_mapid(4054)));
        assert!(class_can_use(ITEMJ_THIRD_UPPER, job_mapid(4060)));
        assert!(class_can_use(ITEMJ_THIRD_BABY, job_mapid(4096)));
        assert!(class_can_use(ITEMJ_FOURTH, job_mapid(4252)));
        assert!(!class_can_use(ITEMJ_THIRD, job_mapid(4252)));
    }

    #[test]
    fn class_flags_follow_the_mapid() {
        let flags = |job| class_flags(job_mapid(job));
        assert_eq!(flags(0), ClassFlags::default(), "Novice");
        assert!(flags(4008).upper && !flags(4008).third, "Lord Knight");
        assert!(flags(4054).third && !flags(4054).upper, "Rune Knight");
        let shadow_cross = flags(4254);
        assert!(shadow_cross.third && shadow_cross.fourth && shadow_cross.trait_job);
        assert!(
            flags(4252).fourth && flags(4263).fourth,
            "Dragon Knight, Troubadour"
        );
        for not_primary in [4302, 4303, 4307] {
            assert!(
                !flags(not_primary).fourth,
                "job {not_primary} is not a primary 4th"
            );
        }
        for expanded in [4304, 4305, 4306, 4307, 4308] {
            assert!(flags(expanded).trait_job, "job {expanded}");
        }
        assert!(
            !flags(4190).trait_job,
            "Super Novice (expanded) has no traits"
        );
        assert!(flags(4023).baby, "Baby Novice");
        assert!(flags(4218).summoner && flags(4220).summoner);
        // The cap selectors use FIRSTMASK for Summoner and SECONDMASK for the extended lines
        // (catalog `mechanics/battle_config.md` §4, §8).
        assert!(
            flags(4308).summoner,
            "Spirit Handler: 0x10a & 0xff = Summoner"
        );
        assert!(flags(4211).extended && flags(4215).extended && flags(4223).extended);
        assert!(
            flags(4304).extended && flags(4306).extended,
            "Shinkiro and Night Watch: 0x1109/0x1108 & 0xfff"
        );
        assert!(flags(4304).third, "the ASPD cap tests third first");
    }

    #[test]
    fn super_novice_bonus_needs_job_70_or_an_expanded_form() {
        let bonus = |job, level| super_novice_bonus(job_mapid(job), level);
        assert!(!bonus(23, 69) && bonus(23, 70), "Super Novice");
        assert!(bonus(4307, 1), "Hyper Novice, captured at job level 1");
        assert!(bonus(4190, 1), "Super Novice (expanded)");
        assert!(bonus(4045, 70), "Super Baby");
        assert!(!bonus(0, 99) && !bonus(4001, 70), "Novice, High Novice");
    }

    #[test]
    fn dual_wield_lines() {
        for id in [
            12, 4013, 4035, 4059, 4065, 4101, 4211, 4212, 4223, 4304, 4254,
        ] {
            assert!(dual_wields(job_mapid(id)), "job {id}");
        }
        for id in [0, 6, 7, 17, 25, 4018] {
            assert!(!dual_wields(job_mapid(id)), "job {id}");
        }
    }

    #[test]
    fn cannonball_jobs() {
        assert!(is_mechanic(job_mapid(4058)) && is_mechanic(job_mapid(4064)));
        assert!(is_mechanic(job_mapid(4100)) && is_mechanic(job_mapid(4253)));
        assert!(is_genetic(job_mapid(4071)) && !is_genetic(job_mapid(18)));
    }
}
