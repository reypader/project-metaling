//! Job changes, job-level drops and skill resets (ENG-SKILL-07, 08, 10).

use super::*;

/// ENG-SKILL-10: `@job` sends one list at the step-8 position, no `SP_SKILLPOINT`, no
/// `0x0441`; records the class-change level; hidden levels stay in memory.
#[test]
fn eng_skill_10_job_change() {
    // Novice (NV_BASIC 9) -> Swordman (captured 536).
    let mut world = World::new(NOVICE, (1, 10), &[(NV_BASIC, 9)], 0);
    let (a, b) = world.command(&format!("job {SWORDMAN}"));
    let list = only_list(&a);
    assert_eq!(levels(&list), [(1, 9), (2, 0), (4, 0), (5, 0), (6, 0)]);
    // After the looks (step 7), before the recalc's diff.
    let looks = position(&a, |e| matches!(e, ServerEvent::SpriteChanged { .. }));
    let at = position(&a, |e| matches!(e, ServerEvent::SkillList { .. }));
    assert!(looks < at, "{a:?}");
    assert!(!a
        .iter()
        .any(|e| matches!(e, ServerEvent::SkillDelete { .. })));
    assert!(!a.iter().any(
        |e| matches!(e, ServerEvent::ParameterChanged { var_id, .. } if *var_id == SP_SKILLPOINT)
    ));
    assert!(!b.iter().any(is_skill_event), "the observer gets nothing");
    assert_eq!(world.change_level(), [0; 3], "a 1st class sets nothing");

    // Swordman at job 10 -> Knight: the 2nd-class level is the job level before the change.
    let mut world = World::new(
        SWORDMAN,
        (1, 10),
        &[(NV_BASIC, 9), (SM_SWORD, 1), (SM_BASH, 5)],
        3,
    );
    let (a, _) = world.command(&format!("job {KNIGHT}"));
    assert_eq!(world.change_level(), [10, 0, 0]);
    // 6 Swordman points < 9: still the Swordman tree's skills.
    let list = only_list(&a);
    assert!(list.iter().all(|s| s.id < KN_SPEARMASTERY), "{list:?}");
    assert_eq!(entry(&list, SM_BASH).level, 5);

    // Knight -> Mage: the Mage list; Sword and Bash hidden.
    let (a, _) = world.command(&format!("job {MAGE}"));
    let list = only_list(&a);
    assert!(list.iter().all(|s| s.id == NV_BASIC || s.id > SM_ENDURE));
    assert!(!a
        .iter()
        .any(|e| matches!(e, ServerEvent::SkillDelete { .. })));
    // The next tick's pass finds nothing to send.
    let out = world.tick(&[]);
    assert!(!seen_by(&out, A).iter().any(is_skill_event));
    // Back to Knight before a save: the levels are still there. The change from Mage (job
    // level 1) sets the 2nd-class level to 1, so the Knight tree shows at once.
    let (a, _) = world.command(&format!("job {KNIGHT}"));
    assert_eq!(world.change_level(), [1, 0, 0]);
    let list = only_list(&a);
    assert_eq!(entry(&list, SM_BASH).level, 5);
    assert_eq!(entry(&list, SM_SWORD).level, 1);
    assert!(list.iter().any(|s| s.id == KN_SPEARMASTERY));
}

/// ENG-SKILL-07: `@jlvl -5` with 0 free points resets the skills silently; the tick's
/// re-check sends the deletes and the list before the diff (captured 650-663).
#[test]
fn eng_skill_07_job_level_down_resets() {
    let mut world = World::new(
        SWORDMAN,
        (1, 10),
        &[(NV_BASIC, 9), (SM_SWORD, 1), (SM_BASH, 5), (SM_PROVOKE, 3)],
        0,
    );
    let (a, b) = world.command("jlvl -5");
    assert_eq!(
        skill_trace(&a),
        [
            "P4",
            "D3",
            "D7",
            "L[(1, 9), (2, 0), (4, 0), (5, 0), (6, 0)]"
        ]
    );
    let message = position(&a, |e| matches!(e, ServerEvent::SelfChat { .. }));
    let job_level = position(&a, |e| is_var(e, SP_JOBLEVEL));
    let points = position(&a, |e| is_var(e, SP_SKILLPOINT));
    let delete = position(&a, |e| matches!(e, ServerEvent::SkillDelete { .. }));
    assert!(
        message < job_level && job_level < points && points < delete,
        "{a:?}"
    );
    // The stat diff (job bonuses lost) comes after the list.
    let list = position(&a, |e| matches!(e, ServerEvent::SkillList { .. }));
    let diff = position(&a, |e| matches!(e, ServerEvent::CoupleStatus { .. }));
    assert!(list < diff, "{a:?}");
    assert!(!b.iter().any(is_skill_event));

    // Enough free points: no reset, no list.
    let mut world = World::new(SWORDMAN, (1, 10), &[(NV_BASIC, 9), (SM_BASH, 5)], 5);
    let (a, _) = world.command("jlvl -5");
    assert_eq!(skill_trace(&a), ["P0"]);
}

/// ENG-SKILL-08: `@skreset` and `@reset` (captured 766-773, 859-892).
#[test]
fn eng_skill_08_resets() {
    let spent = [
        (NV_BASIC, 9),
        (SM_SWORD, 1),
        (SM_TWOHAND, 1),
        (SM_BASH, 5),
        (SM_MAGNUM, 1),
        (SM_PROVOKE, 5),
        (SM_ENDURE, 1),
    ];
    let mut world = World::new(SWORDMAN, (1, 45), &spent, 30);
    let (a, b) = world.command("skreset");
    assert_eq!(
        skill_trace(&a),
        [
            "P44",
            "L[(1, 9), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0), (8, 0)]",
            "D3",
            "D7",
            "D8",
            "L[(1, 9), (2, 0), (4, 0), (5, 0), (6, 0)]"
        ]
    );
    let list = a
        .iter()
        .rposition(|e| matches!(e, ServerEvent::SkillList { .. }))
        .expect("list");
    let message = position(&a, |e| matches!(e, ServerEvent::SelfChat { .. }));
    assert!(list < message, "the message last: {a:?}");
    assert!(!b.iter().any(is_skill_event), "the observer gets nothing");
    // Nothing left to refund: nothing at all but the message.
    let (a, _) = world.command("resetskill");
    assert!(skill_trace(&a).is_empty(), "{a:?}");

    // As a Novice, NV_BASIC is reset and refunded.
    let mut world = World::new(NOVICE, (1, 10), &[(NV_BASIC, 5)], 4);
    let (a, _) = world.command("skreset");
    assert_eq!(skill_trace(&a), ["P9", "L[(1, 0)]"]);

    // @reset: the stat reset's pushes, then the skill reset's, deletes, list, diff, message.
    let mut world = World::new(SWORDMAN, (1, 45), &spent, 25);
    let (a, b) = world.command("reset");
    assert_eq!(
        skill_trace(&a),
        [
            "P39",
            "L[(1, 9), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0), (8, 0)]",
            "D3",
            "D7",
            "D8",
            "L[(1, 9), (2, 0), (4, 0), (5, 0), (6, 0)]"
        ]
    );
    let status_point = position(&a, |e| is_var(e, SP_STATUSPOINT));
    let trait_point = position(&a, |e| is_var(e, SP_TRAITPOINT));
    let points = position(&a, |e| is_var(e, SP_SKILLPOINT));
    let message = position(&a, |e| matches!(e, ServerEvent::SelfChat { .. }));
    let last_list = a
        .iter()
        .rposition(|e| matches!(e, ServerEvent::SkillList { .. }))
        .expect("list");
    assert!(status_point < trait_point && trait_point < points, "{a:?}");
    assert!(last_list < message, "{a:?}");
    assert!(!b.iter().any(is_skill_event));
}

/// `@job` (review fix): the list at step 8 is rAthena's first pass, counted on the old class's
/// visible skills; the tick's re-check counts again on the new set and trims it: deletes after
/// `SP_MAXWEIGHT`, then the final list (captured Dancer -> Assassin, `equip_sequence_4`
/// 2608-2612: {1,48,49,50,52,132,134}, `0x0441` 132, 134, {1,48,49,50,52}).
#[test]
fn job_change_sends_the_second_pass() {
    let archer = [
        (NV_BASIC, 9),
        (43, 10),
        (44, 10),
        (45, 10),
        (46, 10),
        (47, 10),
    ];
    let mut world = World::new(BARD, (90, 50), &archer, 0);
    let (a, _) = world.command(&format!("job {ASSASSIN}"));
    let all = lists(&a);
    assert_eq!(all.len(), 2, "{a:?}");
    let ids = |list: &[SkillInfo]| list.iter().map(|s| s.id).collect::<Vec<_>>();
    assert_eq!(ids(&all[0]), [1, 48, 49, 50, 52, 132, 134]);
    assert_eq!(ids(&all[1]), [1, 48, 49, 50, 52]);
    let deletes: Vec<u16> = a
        .iter()
        .filter_map(|e| match e {
            ServerEvent::SkillDelete { id } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(deletes, [132, 134]);
    let first_delete = position(&a, |e| matches!(e, ServerEvent::SkillDelete { .. }));
    if let Some(weight) = a.iter().position(|e| is_var(e, SP_MAXWEIGHT)) {
        assert!(weight < first_delete, "{a:?}");
    }
    // An Assassin skill is now refused.
    let (a, _) = world.upgrade(&[134]);
    assert!(!a.iter().any(is_skill_event));
}

/// An active skill-up whose full re-check hides slots sends their deletes (review fix): a
/// reset and a skill-up in the same tick, before the pass runs.
#[test]
fn skill_up_after_a_reset_in_the_same_tick_sends_the_deletes() {
    let spent = [
        (NV_BASIC, 9),
        (SM_SWORD, 1),
        (SM_TWOHAND, 1),
        (SM_BASH, 5),
        (SM_MAGNUM, 1),
        (SM_PROVOKE, 5),
        (SM_ENDURE, 1),
    ];
    let mut world = World::new(SWORDMAN, (1, 45), &spent, 30);
    let text = format!("{} : @skreset", name(A)).into_bytes();
    let out = world.tick(&[
        (A, ClientIntent::Chat { text }),
        (A, ClientIntent::UpgradeSkill { id: SM_BASH }),
    ]);
    let a = seen_by(&out, A);
    let trace = skill_trace(&a);
    assert_eq!(
        trace,
        [
            "P44",
            "L[(1, 9), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0), (8, 0)]",
            "P43",
            "D3",
            "D7",
            "D8",
            "L[(1, 9), (2, 0), (4, 0), (5, 1), (6, 0)]"
        ],
        "{a:?}"
    );
}
