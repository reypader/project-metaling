//! `@jlvl` and `@job` (ENG-STAT-07, 08).

use super::*;

/// ENG-STAT-07: `@jlvl` up: effect 1 (the observer gets only this), the message, the job-level
/// pushes, `SP_SKILLPOINT`, then the diff (capture 1 lines 428-450; capture 3 line 931).
#[test]
fn eng_stat_07_job_level_up_and_down() {
    let mut w = World::new(NOVICE, 1, full(CharacterStatus::default()), Vec::new());
    let (a, b) = w.command("jlvl 9");
    assert_eq!(a[0], effect(1), "{a:?}");
    assert!(is_message(&a[1]));
    assert_eq!(
        a[2..6],
        [
            p(SP_JOBLEVEL, 10),
            long(SP_JOBEXP, 0),
            long(SP_NEXTJOBEXP, next_job(NOVICE, 10)),
            p(SP_SKILLPOINT, 9),
        ],
        "{a:?}"
    );
    assert!(
        matches!(
            a[6],
            ServerEvent::CoupleStatus {
                var_id: SP_STR,
                base: 1,
                bonus: 1
            }
        ),
        "the diff follows: {a:?}"
    );
    assert_eq!(b, vec![effect(1)]);
    assert_eq!(w.progress().status.job_level, 10);

    let (a, b) = w.command("joblevel -2");
    assert!(is_message(&a[0]));
    assert_eq!(
        a[1..5],
        [
            p(SP_JOBLEVEL, 8),
            long(SP_JOBEXP, 0),
            long(SP_NEXTJOBEXP, next_job(NOVICE, 8)),
            p(SP_SKILLPOINT, 7),
        ],
        "{a:?}"
    );
    assert!(!a.contains(&effect(1)));
    assert!(b.is_empty(), "{b:?}");
}

/// ENG-STAT-08: `@job swordman` wearing a Novice-only knife: the costs, the job-level pushes,
/// the knife's look and `0x099a`, the class looks (also to the observer), `0x010f`, then
/// `SP_MAXWEIGHT` and the diff, the equip looks, the message; a save is requested (capture 1
/// lines 1714-1764).
#[test]
fn eng_stat_08_job_change_order() {
    let slots = vec![
        worn(NOVICE_KNIFE, EQP_HAND_R),
        worn(COTTON_SHIRT, EQP_ARMOR),
    ];
    let mut w = World::new(NOVICE, 10, full(CharacterStatus::default()), slots);
    let (a, b) = w.command("job swordman");
    let costs: Vec<ServerEvent> = (0..6)
        .map(|i| ServerEvent::StatusCost {
            var_id: SP_USTR + i,
            value: 2,
        })
        .collect();
    assert_eq!(a[..6], costs[..], "{a:?}");
    assert_eq!(
        a[6..9],
        [
            p(SP_JOBLEVEL, 1),
            long(SP_JOBEXP, 0),
            long(SP_NEXTJOBEXP, next_job(SWORDMAN, 1)),
        ]
    );
    let class_looks = vec![
        look(LOOK_BASE, SWORDMAN.into(), 0),
        look(LOOK_HAIR, 3, 0),
        look(LOOK_CLOTHES_COLOR, 0, 0),
        look(LOOK_BODY2, SWORDMAN.into(), 0),
    ];
    assert_eq!(
        a[9..15],
        [
            look(LOOK_WEAPON, 0, 0),
            ServerEvent::UnequipAck {
                index: 2,
                location: EQP_HAND_R,
                failed: false
            },
            class_looks[0].clone(),
            class_looks[1].clone(),
            class_looks[2].clone(),
            class_looks[3].clone(),
        ],
        "{a:?}"
    );
    assert!(matches!(a[15], ServerEvent::SkillList { .. }), "{a:?}");
    assert_eq!(a[16], p(SP_MAXWEIGHT, 28_000 + 300), "{a:?}");
    let equip_looks = vec![
        look(LOOK_WEAPON, 0, 0),
        look(LOOK_SHOES, 0, 0),
        look(LOOK_HEAD_BOTTOM, 0, 0),
        look(LOOK_HEAD_TOP, 0, 0),
        look(LOOK_HEAD_MID, 0, 0),
        look(LOOK_ROBE, 0, 0),
    ];
    let n = a.len();
    assert_eq!(a[n - 7..n - 1], equip_looks[..], "{a:?}");
    assert!(is_message(&a[n - 1]));
    let mut seen = vec![look(LOOK_WEAPON, 0, 0)];
    seen.extend(class_looks);
    seen.extend(equip_looks);
    assert_eq!(b, seen, "the observer gets the looks only");
    assert_eq!(w.saves(), vec![char_id(A)]);
    let progress = w.progress();
    assert_eq!(
        (
            progress.job_class,
            progress.status.job_level,
            progress.base_level
        ),
        (SWORDMAN, 1, 10)
    );
}

/// ENG-STAT-08: a base level above the new job's max is capped with a stat reset (its pushes,
/// for the new class: trait costs 0) and the base tail, sent before the costs (traits spec
/// §2.7; leaving Hyper Novice then resets again, ENG-TRAIT-06).
#[test]
fn eng_stat_08_job_change_caps_the_base_level() {
    let status = CharacterStatus {
        str: 50,
        status_point: 0,
        ..full(CharacterStatus::default())
    };
    let mut w = World::new(HYPER_NOVICE, 150, status, Vec::new());
    let (a, _) = w.command("job 1");
    let points = jobs().status_points(99);
    let mut reset: Vec<ServerEvent> = (0..6)
        .map(|i| ServerEvent::StatusCost {
            var_id: SP_USTR + i,
            value: 2,
        })
        .collect();
    reset.extend((0..6).map(|i| ServerEvent::StatusCost {
        var_id: SP_UPOW + i,
        value: 0,
    }));
    reset.push(p(SP_STATUSPOINT, points as i32));
    reset.push(p(SP_TRAITPOINT, 0));
    assert_eq!(a[..reset.len()], reset[..], "{a:?}");
    assert_eq!(
        a[reset.len()..reset.len() + 5],
        base_tail(SWORDMAN, points, 99)[..],
        "{a:?}"
    );
    let progress = w.progress();
    assert_eq!((progress.base_level, progress.status.str), (99, 1));
    assert_eq!(
        u32::from(progress.status.status_point),
        jobs().status_points(99)
    );
}

/// Spec §2.6: the current job or an unknown name gets our message and changes nothing;
/// Bard and Dancer follow the character's sex.
#[test]
fn eng_stat_08_job_refusals_and_sex() {
    let mut w = World::new(NOVICE, 1, CharacterStatus::default(), Vec::new());
    for line in ["job 0", "job novice", "job not_a_job", "job 99999", "job"] {
        let (a, b) = w.command(line);
        assert!(a.iter().all(is_message), "{line}: {a:?}");
        assert!(!a.is_empty(), "{line}");
        assert!(b.is_empty(), "{line}: {b:?}");
    }
    assert!(w.saves().is_empty());
    assert_eq!(w.progress().job_class, NOVICE);

    w.command("job bard");
    assert_eq!(w.progress().job_class, BARD, "a male stays a Bard");
    let mut w = World::with_sex(
        NOVICE,
        1,
        CharacterStatus::default(),
        Vec::new(),
        Sex::Female,
    );
    w.command("jobchange bard");
    assert_eq!(w.progress().job_class, DANCER);
    let (a, _) = w.command("job 20");
    assert!(a.iter().all(is_message), "already a Dancer: {a:?}");
}

/// Spec §2.6 step 10: `@job`'s equip looks carry each slot's view when they are sent, so an
/// equip later in the same tick isn't undone on screen (Finish review m-1).
#[test]
fn eng_stat_08_equip_looks_are_read_when_sent() {
    let slots = vec![Some(InventoryItem::new(ItemId(KNIFE), 1))];
    let mut w = World::new(NOVICE, 10, CharacterStatus::default(), slots);
    let text = format!("{} : @job swordman", name(A)).into_bytes();
    let out = w.tick(&[
        (A, ClientIntent::Chat { text }),
        (
            A,
            ClientIntent::EquipItem {
                index: 2,
                position: EQP_HAND_R,
            },
        ),
    ]);
    for n in [A, B] {
        let seen = seen_by(&out, n);
        let last_weapon = seen
            .iter()
            .rev()
            .find(|e| matches!(e, ServerEvent::SpriteChanged { look_type, .. } if *look_type == LOOK_WEAPON));
        assert_eq!(
            last_weapon,
            Some(&look(LOOK_WEAPON, KNIFE, 0)),
            "{n}: {seen:?}"
        );
    }
}

/// Spec §2.6: `@job` keeps usable gear on, and sends no heal.
#[test]
fn eng_stat_08_usable_gear_stays() {
    let slots = vec![worn(KNIFE, EQP_HAND_R), worn(MARK, EQP_HEAD_TOP)];
    let mut w = World::new(NOVICE, 10, CharacterStatus::default(), slots);
    let (a, _) = w.command("job swordman");
    assert!(
        !a.iter()
            .any(|e| matches!(e, ServerEvent::UnequipAck { .. })),
        "{a:?}"
    );
    let n = a.len();
    assert_eq!(a[n - 7], look(LOOK_WEAPON, KNIFE, 0), "{a:?}");
    assert_eq!(a[n - 4], look(LOOK_HEAD_TOP, 1348, 0), "{a:?}");
    assert!(
        !a.iter()
            .any(|e| matches!(e, ServerEvent::ParameterChanged { var_id: SP_HP, .. })),
        "no heal: {a:?}"
    );
}
