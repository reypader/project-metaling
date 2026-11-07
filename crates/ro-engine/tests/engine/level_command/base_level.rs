//! `@blvl`: base level up and down (ENG-STAT-06).

use super::*;

/// ENG-STAT-06: `@blvl 10` on a level-1 Novice: the diff, the heal to the new max, `0x019b`
/// effect 0 (the observer gets only this), the message, then the tail (capture 1 lines
/// 401-423).
#[test]
fn eng_stat_06_base_level_up_order() {
    let mut w = World::new(NOVICE, 1, full(CharacterStatus::default()), Vec::new());
    let (a, b) = w.command("blvl 10");
    let points = jobs().status_points(11);
    assert_eq!(a[a.len() - 5..], base_tail(NOVICE, points, 11)[..], "{a:?}");
    let max_hp = index_of(&a, &p(SP_MAXHP, 90));
    index_of(&a, &p(SP_MAXSP, 21));
    let heal = index_of(&a, &p(SP_HP, 90));
    assert!(max_hp < heal, "diff before the heal: {a:?}");
    assert_eq!(
        a[heal..a.len() - 5]
            .iter()
            .filter(|e| !is_message(e))
            .cloned()
            .collect::<Vec<_>>(),
        vec![p(SP_HP, 90), p(SP_SP, 21), effect(0)],
        "{a:?}"
    );
    assert!(is_message(&a[a.len() - 6]), "the message before the tail");
    assert_eq!(b, vec![effect(0)], "the observer sees the effect only");
    let progress = w.progress();
    assert_eq!(progress.base_level, 11);
    assert_eq!(u32::from(progress.status.status_point), points);
    assert_eq!((progress.status.hp, progress.status.sp), (90, 21));
}

/// ENG-STAT-06: an `elvmax` item comes off after the tail (capture 3): head top `0x099a` then
/// the look; shoes the look then `0x099a`. The observer gets the effect and the look.
#[test]
fn eng_stat_06_elvmax_unequip_comes_last() {
    let slots = vec![worn(MARK, EQP_HEAD_TOP), worn(BOOTS, EQP_SHOES)];
    let mut w = World::new(HYPER_NOVICE, 99, full(CharacterStatus::default()), slots);
    let (a, b) = w.command("blvl 1");
    assert_eq!(
        a[a.len() - 2..],
        [
            ServerEvent::UnequipAck {
                index: 2,
                location: EQP_HEAD_TOP,
                failed: false
            },
            look(LOOK_HEAD_TOP, 0, 0),
        ],
        "{a:?}"
    );
    assert_eq!(
        a[a.len() - 3],
        long(SP_NEXTBASEEXP, next_base(HYPER_NOVICE, 100))
    );
    assert_eq!(b, vec![effect(0), look(LOOK_HEAD_TOP, 0, 0)]);

    let (a, b) = w.command("blvl 1");
    assert_eq!(
        a[a.len() - 2..],
        [
            look(LOOK_SHOES, 0, 0),
            ServerEvent::UnequipAck {
                index: 3,
                location: EQP_SHOES,
                failed: false
            },
        ],
        "{a:?}"
    );
    assert_eq!(b, vec![effect(0), look(LOOK_SHOES, 0, 0)]);
    assert_eq!(w.progress().base_level, 101);
    let (a, _) = w.command("blvl 1");
    assert!(
        !a.iter()
            .any(|e| matches!(e, ServerEvent::UnequipAck { .. })),
        "nothing left to take off: {a:?}"
    );
}

/// ENG-STAT-06: `@blvl -5` with enough unspent points: the message, the clamp pushes, the diff,
/// the tail; no heal, no effect (capture 1 lines 679-703).
#[test]
fn eng_stat_06_base_level_down_order() {
    let status = CharacterStatus {
        status_point: u16::try_from(jobs().status_points(11)).expect("small"),
        ..full(CharacterStatus::default())
    };
    let mut w = World::new(NOVICE, 11, status, Vec::new());
    let (a, b) = w.command("blvl -5");
    assert!(is_message(&a[0]), "{a:?}");
    let max_hp = jobs().get(NOVICE).expect("novice").base_hp(6);
    let hp = i32::try_from(max_hp).expect("small") * 101 / 100;
    assert_eq!(a[1], p(SP_HP, hp), "clamp push first: {a:?}");
    let remove = jobs().status_points(11) - jobs().status_points(6);
    let tail = base_tail(NOVICE, jobs().status_points(11) - remove, 6);
    assert_eq!(a[a.len() - 5..], tail[..], "{a:?}");
    assert!(!a.contains(&effect(0)));
    assert!(b.is_empty(), "nothing for the observer: {b:?}");
}

/// ENG-STAT-06: `@blvl -n` with too few unspent points resets the stats first: the costs, the
/// points (table at the old level), then the message; the `0x0141`s come from the diff.
#[test]
fn eng_stat_06_base_level_down_resets_stats() {
    let status = CharacterStatus {
        str: 10,
        status_point: 0,
        ..full(CharacterStatus::default())
    };
    let mut w = World::new(NOVICE, 11, status, Vec::new());
    let (a, _) = w.command("blvl -5");
    let old_points = jobs().status_points(11);
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
    reset.push(p(SP_STATUSPOINT, old_points as i32));
    reset.push(p(SP_TRAITPOINT, 0));
    assert_eq!(a[..reset.len()], reset[..], "{a:?}");
    assert!(is_message(&a[reset.len()]), "{a:?}");
    assert!(
        a.iter().any(|e| matches!(
            e,
            ServerEvent::CoupleStatus {
                var_id: SP_STR,
                base: 1,
                ..
            }
        )),
        "STR's new base from the diff: {a:?}"
    );
    let left = old_points - (old_points - jobs().status_points(6));
    let progress = w.progress();
    assert_eq!((progress.status.str, progress.base_level), (1, 6));
    assert_eq!(u32::from(progress.status.status_point), left);
}

/// Spec §2.5: a bad argument gets the usage line; the level limits get their own line; nothing
/// changes.
#[test]
fn eng_stat_06_refusals() {
    let mut w = World::new(NOVICE, 1, CharacterStatus::default(), Vec::new());
    for line in ["blvl", "blvl abc", "blvl 0", "jlvl", "jlvl x"] {
        let (a, _) = w.command(line);
        assert_eq!(a.len(), 1, "{line}: {a:?}");
        assert!(message_text(&a[0]).starts_with("Usage"), "{line}: {a:?}");
    }
    for line in ["blvl -1", "jlvl -3"] {
        let (a, _) = w.command(line);
        assert_eq!(a.len(), 1, "{line}: {a:?}");
        assert!(!message_text(&a[0]).starts_with("Usage"), "{line}");
    }
    let (a, _) = w.command("blevel 500");
    assert_eq!(
        a[a.len() - 5..],
        base_tail(NOVICE, jobs().status_points(99), 99)[..]
    );
    let (a, _) = w.command("lvup 1");
    assert_eq!(a.len(), 1, "already at the Novice max: {a:?}");
    assert_eq!(w.progress().base_level, 99);
}
