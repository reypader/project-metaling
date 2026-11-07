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
