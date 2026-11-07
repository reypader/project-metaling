//! Casting rules: First Aid, cast delay, costs, range and cast time (ENG-EFF-01..04, 08, 12).

use super::*;

/// ENG-EFF-01: First Aid below max HP: `0x0b1a`, SP, `0x09cb` 5, `SP_HP`; the observer gets
/// `0x0b1a` and `0x09cb`. The client's target id is ignored for a self skill.
#[test]
fn eng_eff_01_first_aid_heals_five() {
    let mut w = first_aid_world(10);
    let sp = w.status(A).sp;
    let out = w.cast(A, NV_FIRSTAID, 1, 12345);
    assert_eq!(
        seen_by(&out, A),
        [
            casting(gid(A), gid(A), NV_FIRSTAID, 0, 0),
            param(SP_SP, sp as i32 - 3),
            no_damage(NV_FIRSTAID, 5, gid(A), gid(A)),
            param(SP_HP, 15),
        ]
    );
    assert_eq!(
        seen_by(&out, B),
        [
            casting(gid(A), gid(A), NV_FIRSTAID, 0, 0),
            no_damage(NV_FIRSTAID, 5, gid(A), gid(A)),
        ]
    );
    assert_eq!((w.status(A).hp, w.status(A).sp), (15, sp - 3));
}

/// ENG-EFF-02: First Aid at full HP: `0x09cb` still says 5, and no HP push follows.
#[test]
fn eng_eff_02_first_aid_at_full_hp() {
    let mut w = first_aid_world(9999);
    let sp = w.status(A).sp;
    let out = w.cast(A, NV_FIRSTAID, 1, gid(A));
    assert_eq!(
        seen_by(&out, A),
        [
            casting(gid(A), gid(A), NV_FIRSTAID, 0, 0),
            param(SP_SP, sp as i32 - 3),
            no_damage(NV_FIRSTAID, 5, gid(A), gid(A)),
        ]
    );
}

/// ENG-EFF-03: a cast before `canact` is dropped with no packet; one at `T + amotion + 100` is
/// accepted.
#[test]
fn eng_eff_03_cast_gate() {
    let mut w = first_aid_world(10);
    let amotion = w.amotion(A);
    let t = w.now;
    assert!(!seen_by(&w.cast(A, NV_FIRSTAID, 1, gid(A)), A).is_empty());
    let early = t + amotion + 90;
    let out = w.at(
        early,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: NV_FIRSTAID,
                target: gid(A),
            },
        )],
    );
    assert_eq!(seen_by(&out, A), [], "too early is silent");
    let out = w.at(
        t + amotion + 100,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: NV_FIRSTAID,
                target: gid(A),
            },
        )],
    );
    assert_eq!(
        seen_by(&out, A)[0],
        casting(gid(A), gid(A), NV_FIRSTAID, 0, 0)
    );
}

/// ENG-EFF-04: not enough SP (cause 1), zeny (5), wrong weapon (6): `0x0110` alone, nothing
/// spent; SP is checked before zeny.
#[test]
fn eng_eff_04_requirement_failures() {
    let mut w = World::new(
        vec![Player::new(A, NOVICE, (40, 40), &[(NV_FIRSTAID, 1)]).status(|s| s.sp = 2)],
        PORING,
        mobs(1000, 1),
    );
    assert_eq!(
        seen_by(&w.cast(A, NV_FIRSTAID, 1, gid(A)), A),
        [fail(NV_FIRSTAID, 1)]
    );
    assert_eq!(w.status(A).sp, 2);

    // Merchant: no zeny, then no SP and no zeny.
    let mut w = World::new(
        vec![Player::new(A, MERCHANT, NEXT_TO_MOB, &[(MC_MAMMONITE, 10)])],
        PORING,
        mobs(100_000, 1),
    );
    let mob = w.mob_gid();
    let sp = w.status(A).sp;
    assert_eq!(
        seen_by(&w.cast(A, MC_MAMMONITE, 10, mob), A),
        [fail(MC_MAMMONITE, 5)]
    );
    assert_eq!((w.status(A).sp, w.status(A).zeny), (sp, 0));
    let mut w = World::new(
        vec![Player::new(A, MERCHANT, NEXT_TO_MOB, &[(MC_MAMMONITE, 10)]).status(|s| s.sp = 4)],
        PORING,
        mobs(100_000, 1),
    );
    let mob = w.mob_gid();
    assert_eq!(
        seen_by(&w.cast(A, MC_MAMMONITE, 10, mob), A),
        [fail(MC_MAMMONITE, 1)]
    );

    // Swordman with a bow: Bash refuses it.
    let mut bow = InventoryItem::new(ItemId(BOW), 1);
    bow.equip = EQP_ARMS;
    let mut swordman = Player::new(A, SWORDMAN, NEXT_TO_MOB, &[(SM_BASH, 10)]);
    swordman.inventory = vec![Some(bow)];
    let mut w = World::new(vec![swordman], PORING, mobs(100_000, 1));
    let mob = w.mob_gid();
    let sp = w.status(A).sp;
    assert_eq!(seen_by(&w.cast(A, SM_BASH, 10, mob), A), [fail(SM_BASH, 6)]);
    assert_eq!(w.status(A).sp, sp);

    // The requirements come before the range: out of range with 4 SP still gets cause 1.
    let mut w = World::new(
        vec![Player::new(A, SWORDMAN, (55, 50), &[(SM_BASH, 10)]).status(|s| s.sp = 4)],
        PORING,
        mobs(100_000, 1),
    );
    let mob = w.mob_gid();
    assert_eq!(seen_by(&w.cast(A, SM_BASH, 1, mob), A), [fail(SM_BASH, 1)]);

    // At 90 % weight or more: cause 9, before anything else.
    let mut novice = Player::new(A, NOVICE, (40, 40), &[(NV_FIRSTAID, 1)]);
    novice.inventory = vec![Some(InventoryItem::new(ItemId(ANVIL), 1))];
    let mut w = World::new(vec![novice], PORING, mobs(1000, 1));
    assert_eq!(
        seen_by(&w.cast(A, NV_FIRSTAID, 1, gid(A)), A),
        [fail(NV_FIRSTAID, 9)]
    );
}

/// ENG-EFF-08: out of range standing is a silent drop; sent mid-walk out of `range - 1` it
/// fires at the step end; a self cast while walking stops the walk (`0x0088`) first.
#[test]
fn eng_eff_08_range_and_walking() {
    let mut w = World::new(
        vec![Player::new(A, SWORDMAN, (55, 50), &[(SM_BASH, 10)])],
        PORING,
        mobs(1_000_000, 1),
    );
    let mob = w.mob_gid();
    assert_eq!(seen_by(&w.cast(A, SM_BASH, 1, mob), A), [], "out of range");
    // Walk toward the mob and ask mid-walk: stored, then cast once adjacent.
    let t = w.now;
    drop(w.at(
        t,
        &[(
            A,
            ClientIntent::RequestMove {
                dest_x: 51,
                dest_y: 50,
            },
        )],
    ));
    let out = w.at(
        t + 50,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: SM_BASH,
                target: mob,
            },
        )],
    );
    assert!(skill_damage(&seen_by(&out, A)).is_none(), "not yet");
    let later = w.run_to(A, t + 2000);
    let cast_at = later
        .iter()
        .position(|e| matches!(e, ServerEvent::SkillCasting { .. }))
        .expect("the step action casts");
    assert!(skill_damage(&later[cast_at..]).is_some());

    // Heal on self while walking: `0x0088` then `0x0b1a`. (NV_BASIC 9: a first job with fewer
    // points spent sees only the Novice tree.)
    let mut w = World::new(
        vec![Player::new(
            A,
            ACOLYTE,
            (40, 40),
            &[(NV_BASIC, 9), (AL_HEAL, 1)],
        )],
        PORING,
        mobs(1000, 1),
    );
    let t = w.now;
    drop(w.at(
        t,
        &[(
            A,
            ClientIntent::RequestMove {
                dest_x: 40,
                dest_y: 30,
            },
        )],
    ));
    let out = w.at(
        t + 200,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: AL_HEAL,
                target: gid(A),
            },
        )],
    );
    let to_a = seen_by(&out, A);
    assert!(
        matches!(to_a[0], ServerEvent::UnitStopped { gid: g, .. } if g == gid(A)),
        "{to_a:?}"
    );
    assert_eq!(to_a[1], casting(gid(A), gid(A), AL_HEAL, 6, 0));
    assert!(!w.engine.is_walking(char_id(A)));

    // Heal on a mob within `range - 1` mid-walk: `0x0088`, then the cast at once.
    let mut w = World::new(
        vec![Player::new(
            A,
            ACOLYTE,
            (45, 50),
            &[(NV_BASIC, 9), (AL_HEAL, 1)],
        )],
        PORING,
        mobs(1000, 1),
    );
    let (mob, t) = (w.mob_gid(), w.now);
    let mv = w.at(
        t,
        &[(
            A,
            ClientIntent::RequestMove {
                dest_x: 45,
                dest_y: 40,
            },
        )],
    );
    assert!(!seen_by(&mv, A).is_empty(), "the walk starts");
    let out = w.at(
        t + 200,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: AL_HEAL,
                target: mob,
            },
        )],
    );
    let to_a = seen_by(&out, A);
    assert!(
        matches!(to_a[0], ServerEvent::UnitStopped { gid: g, .. } if g == gid(A)),
        "{to_a:?}"
    );
    assert_eq!(to_a[1], casting(gid(A), mob, AL_HEAL, 6, 0));

    // A cast ends a pending attack step action. Cast just after a step end, while the step
    // action's timer is armed: without the clear it would fire (`0x0139` or a swing).
    let mut w = World::new(
        vec![Player::new(A, NOVICE, (58, 50), &[(NV_FIRSTAID, 1)])],
        PORING,
        mobs(1_000_000, 1),
    );
    let (mob, t) = (w.mob_gid(), w.now);
    drop(w.at(
        t,
        &[(
            A,
            ClientIntent::RequestMove {
                dest_x: 51,
                dest_y: 50,
            },
        )],
    ));
    drop(w.at(
        t + 50,
        &[(
            A,
            ClientIntent::RequestAction {
                target_id: mob,
                action_type: 7,
            },
        )],
    ));
    let out = w.at(
        t + 170,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: NV_FIRSTAID,
                target: gid(A),
            },
        )],
    );
    assert!(seen_by(&out, A).contains(&casting(gid(A), gid(A), NV_FIRSTAID, 0, 0)));
    let later = w.run_to(A, t + 3000);
    assert!(
        !later.iter().any(|e| matches!(
            e,
            ServerEvent::Damage { .. } | ServerEvent::AttackOutOfRange { .. }
        )),
        "{later:?}"
    );
}

/// ENG-EFF-12: a cast with a cast time: `0x0b1a` carries it (1000 ms at DEX 1 / INT 1 = 924),
/// another request meanwhile is dropped, and the effect lands at `CastEnd`.
#[test]
fn eng_eff_12_cast_time() {
    let mut w = World::with_skills(
        vec![Player::new(A, SWORDMAN, NEXT_TO_MOB, &[(SM_BASH, 10)])],
        PORING,
        mobs(1_000_000, 1),
        skills_with_bash_cast_time(),
    );
    let (mob, t) = (w.mob_gid(), w.now);
    let dex = w.battle_stat(A, SP_DEX);
    let int = w.battle_stat(A, 16);
    assert_eq!((dex, int), (1, 1), "the fixture's stats");
    let to_a = seen_by(&w.cast(A, SM_BASH, 1, mob), A);
    assert_eq!(to_a, [casting(gid(A), mob, SM_BASH, 0, 924)]);
    let mid = w.at(
        t + 400,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: SM_BASH,
                target: mob,
            },
        )],
    );
    assert_eq!(seen_by(&mid, A), [], "a cast is in progress");
    let before = w.run_to(A, t + 920);
    assert!(skill_damage(&before).is_none(), "{before:?}");
    let at_end = w.run_to(A, t + 930);
    assert!(
        matches!(at_end[0], ServerEvent::ParameterChanged { var_id, .. } if var_id == SP_SP),
        "{at_end:?}"
    );
    assert!(skill_damage(&at_end).is_some());
}
