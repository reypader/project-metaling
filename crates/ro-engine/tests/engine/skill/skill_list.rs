//! The skill list: values, the login list and saved rows (ENG-SKILL-03, 04).

use super::*;

/// ENG-SKILL-03: the captured `sp`/`range2` values (catalog §10.2) with the shipped data.
#[test]
fn eng_skill_03_captured_list_values() {
    let world = World::new(
        SWORDMAN,
        (1, 45),
        &[
            (NV_BASIC, 9),
            (SM_SWORD, 1),
            (SM_BASH, 5),
            (SM_PROVOKE, 5),
            (SM_MAGNUM, 1),
            (SM_ENDURE, 1),
        ],
        0,
    );
    let list = only_list(&world.login);
    assert_eq!(
        list.iter().map(|s| s.id).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5, 6, 7, 8],
        "ascending ids, quest and wedding skills hidden"
    );
    let values = |id| {
        let e = entry(&list, id);
        (e.inf, e.level, e.sp, e.range, e.upgradable)
    };
    assert_eq!(values(SM_BASH), (1, 5, 8, 1, true));
    assert_eq!(values(SM_PROVOKE), (1, 5, 8, 9, true));
    assert_eq!(values(SM_MAGNUM), (4, 1, 30, 0, true));
    assert_eq!(values(SM_ENDURE), (4, 1, 10, 0, true));
    assert_eq!(values(SM_SWORD), (0, 1, 0, 0, true));
    // Level 0: the real inf, sp and range 0.
    assert_eq!(values(SM_TWOHAND), (0, 0, 0, 0, true));
    assert_eq!(values(NV_BASIC), (0, 9, 0, 0, false));
    assert_eq!(&*entry(&list, SM_BASH).name, "SM_BASH");

    let world = World::new(
        SWORDMAN,
        (1, 45),
        &[(NV_BASIC, 9), (SM_BASH, 10), (SM_PROVOKE, 10)],
        0,
    );
    let list = only_list(&world.login);
    assert_eq!(
        (entry(&list, SM_BASH).sp, entry(&list, SM_BASH).range),
        (15, 1)
    );
    assert_eq!(entry(&list, SM_PROVOKE).sp, 13);
    assert!(!entry(&list, SM_BASH).upgradable, "the tree max");
}

/// ENG-SKILL-04: a fresh Novice's login list is NV_BASIC at level 0 (41 bytes on the wire);
/// a Swordman's is the real one.
#[test]
fn eng_skill_04_login_list() {
    let world = World::new(NOVICE, (1, 1), &[], 0);
    let list = only_list(&world.login);
    assert_eq!(
        list,
        [SkillInfo {
            id: NV_BASIC,
            inf: 0,
            level: 0,
            sp: 0,
            range: 0,
            name: Arc::from("NV_BASIC"),
            upgradable: true,
        }]
    );
    let world = World::new(
        SWORDMAN,
        (1, 10),
        &[(NV_BASIC, 9), (SM_SWORD, 1), (SM_BASH, 5)],
        0,
    );
    assert_eq!(
        levels(&only_list(&world.login)),
        [(1, 9), (2, 1), (3, 0), (4, 0), (5, 5), (6, 0), (7, 0)]
    );
    // NV_BASIC below 9: only the Novice tree (captured 1341).
    let world = World::new(SWORDMAN, (1, 10), &[(NV_BASIC, 3)], 6);
    assert_eq!(levels(&only_list(&world.login)), [(1, 3)]);
}

/// The skills revision (what autosave compares; spec §2.9) moves with a skill-up, a reset and
/// a job change that hides learned levels; not with a refused request.
#[test]
fn skills_revision_tracks_saved_rows() {
    let mut world = World::new(SWORDMAN, (1, 10), &[(NV_BASIC, 9), (SM_BASH, 1)], 2);
    let revision = |world: &World| world.engine.skills_revision(char_id(A)).expect("A");
    let start = revision(&world);
    world.upgrade(&[60_000]);
    assert_eq!(revision(&world), start, "refused");
    world.upgrade(&[SM_BASH]);
    let after_up = revision(&world);
    assert_ne!(after_up, start);
    let snapshot = world.engine.play_snapshot(char_id(A)).expect("A");
    assert_eq!(snapshot.skills, learned(&[(NV_BASIC, 9), (SM_BASH, 2)]));
    world.command(&format!("job {MAGE}"));
    let after_job = revision(&world);
    assert_ne!(after_job, after_up, "Bash hidden");
    let snapshot = world.engine.play_snapshot(char_id(A)).expect("A");
    assert_eq!(
        snapshot.skills,
        learned(&[(NV_BASIC, 9)]),
        "hidden levels aren't saved"
    );
    // The reset refunds the hidden Bash too (rAthena resets every slot with a level): 1 + 2.
    let (a, _) = world.command("skreset");
    assert_ne!(revision(&world), after_job);
    assert_eq!(skill_trace(&a)[0], "P3");
}
