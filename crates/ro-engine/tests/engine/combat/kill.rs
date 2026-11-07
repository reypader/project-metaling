//! Kills, exp shares, level-ups and the exp tables (ENG-CMB-07..10).

use super::*;

/// ENG-CMB-07: the killing hit sends `0x0977` hp 0, then the exp packets, in the hit's tick and to
/// the attacker only. The mob leaves the grid at once (new attacks on it are silent); `0x0080`
/// type 1 goes to the death cell's area at +250 ms, and the mob is gone after.
#[test]
fn eng_cmb_07_kill_and_vanish() {
    // Job 9 with an empty bar (340) and base 1 (548): no level-up.
    let status = CharacterStatus {
        job_level: 9,
        ..CharacterStatus::default()
    };
    let (a, b, mut engine) = one_hit_kill(1, status);
    let mob = mob_gid(&engine);
    assert_eq!(
        a,
        vec![
            dead_bar(mob, 1),
            long(SP_BASEEXP, 150),
            long(SP_JOBEXP, 40),
            gained(A, SP_BASEEXP, 150),
            gained(A, SP_JOBEXP, 40),
        ]
    );
    assert!(b.is_empty(), "the observer isn't in the log: {b:?}");

    let (_, amotion) = bare_hit(1);
    let dies = 1000 + amotion;
    let mut quiet = Vec::new();
    for now in dies + 1..dies + 250 {
        // B never attacked (no canact); A is past its canact by +150.
        if now == dies + 1 || now == dies + 150 {
            let n = if now == dies + 1 { B } else { A };
            engine.push_intent(intent(
                n,
                ClientIntent::RequestAction {
                    target_id: mob,
                    action_type: ACT_ATTACK_REPEAT,
                },
            ));
        }
        quiet.extend(tick(&mut engine, now));
    }
    assert!(quiet.is_empty(), "a dying mob can't be attacked: {quiet:?}");
    let vanish = tick(&mut engine, dies + 250);
    let died = ServerEvent::UnitVanished {
        gid: mob,
        vanish_type: VANISH_DIED,
    };
    assert_eq!(seen_by(&vanish, A), vec![died.clone()]);
    assert_eq!(seen_by(&vanish, B), vec![died]);
    assert!(engine.mobs().next().is_none(), "despawned, gid free");
}

/// ENG-CMB-08: exp by damage share. Solo at a level difference of -31 pays the full db exp (no
/// penalty). Two attackers share by logged damage (overkill included) with the 25 % bonus. An
/// attacker offline at death gets nothing, and the others don't get its share.
#[test]
fn eng_cmb_08_exp_shares() {
    let status = CharacterStatus {
        job_level: 9,
        ..CharacterStatus::default()
    };
    let (a, _, _) = one_hit_kill(32, status);
    assert_eq!(
        a[a.len() - 2..],
        [gained(A, SP_BASEEXP, 150), gained(A, SP_JOBEXP, 40)],
        "full exp at 32 vs 1: {a:?}"
    );

    // HP 2d + 1: A (continuous, from 1000) hits at +am and +3am; B's single swing at 3am + 10
    // lands at 4am + 10 and kills with d - 1 overkill; A's third hit (+5am) is dropped.
    let (d, am) = bare_hit(32);
    let mut engine = kill_world(
        2 * d + 1,
        vec![
            fighter(A, (51, 50), 32, status),
            fighter(B, (49, 50), 32, status),
        ],
    );
    let mob = mob_gid(&engine);
    let mut seen = seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000), A);
    seen.extend(run(&mut engine, A, 1000, 1000 + 3 * am + 9, 1));
    seen.extend(seen_by(
        &attack(&mut engine, B, ACT_ATTACK, 1000 + 3 * am + 10),
        A,
    ));
    seen.extend(run(
        &mut engine,
        A,
        1000 + 3 * am + 10,
        1000 + 4 * am + 9,
        1,
    ));
    let kill = tick(&mut engine, 1000 + 4 * am + 10);
    for n in [A, B] {
        assert_eq!(
            seen_by(&kill, n)[0],
            dead_bar(mob, 2 * d + 1),
            "{n}: {kill:?}"
        );
    }
    // A swung three times, but its third hit (due at +5am) came after the kill.
    assert_eq!(swings(&seen).len(), 4, "{seen:?}");
    assert_eq!(dealt_by(&seen, B), u64::from(d), "{seen:?}");
    let (a_dealt, b_dealt) = (2 * u64::from(d), u64::from(d));
    let total = a_dealt + b_dealt;
    for (n, dealt) in [(A, a_dealt), (B, b_dealt)] {
        assert_eq!(
            exp_of(&kill, n),
            (
                expected_exp(PORING_EXP.0, dealt, total, 2),
                expected_exp(PORING_EXP.1, dealt, total, 2)
            ),
            "{n}"
        );
    }

    // B hits once (at +am, before A's first hit) and logs out; A's second hit kills. The log
    // holds A 2d (overkill included) and B d: A gets 2/3 with no attacker bonus, and B's third
    // isn't handed on.
    let mut engine = kill_world(
        2 * d + 1,
        vec![
            fighter(A, (51, 50), 32, status),
            fighter(B, (49, 50), 32, status),
        ],
    );
    attack(&mut engine, B, ACT_ATTACK, 1000);
    attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000);
    run(&mut engine, A, 1000, 1000 + am + 5, 1);
    engine.remove_character(char_id(B)).expect("logout");
    run(&mut engine, A, 1000 + am + 5, 1000 + 3 * am - 1, 1);
    let kill = tick(&mut engine, 1000 + 3 * am);
    assert_eq!(seen_by(&kill, A)[0], dead_bar(mob_gid(&engine), 2 * d + 1));
    let (a_dealt, total) = (2 * u64::from(d), 3 * u64::from(d));
    assert_eq!(
        exp_of(&kill, A),
        (
            expected_exp(PORING_EXP.0, a_dealt, total, 1),
            expected_exp(PORING_EXP.1, a_dealt, total, 1)
        )
    );
}

/// ENG-CMB-09: natural level-ups in the captured order (`experience.md` "Capture"): job only;
/// base and job in one kill; base only; and at the max job level.
#[test]
fn eng_cmb_09_level_ups() {
    // Job 1 -> 2 (file 686-698): the job tail inline, the diff, effect 1, then both 0x0acc. The
    // leftover 30 is clamped to the consumed bar - 1.
    let (a, b, engine) = one_hit_kill(1, CharacterStatus::default());
    let mob = mob_gid(&engine);
    assert_eq!(
        a[..6],
        [
            dead_bar(mob, 1),
            long(SP_BASEEXP, 150),
            p(SP_JOBLEVEL, 2),
            long(SP_JOBEXP, 9),
            long(SP_NEXTJOBEXP, 18),
            p(SP_SKILLPOINT, 1),
        ],
        "{a:?}"
    );
    assert_eq!(
        a[a.len() - 3..],
        [
            effect(A, 1),
            gained(A, SP_BASEEXP, 150),
            gained(A, SP_JOBEXP, 40)
        ],
        "{a:?}"
    );
    assert!(
        a[6..a.len() - 3]
            .iter()
            .any(|e| matches!(e, ServerEvent::CoupleStatus { .. })),
        "the job-bonus LUK in the diff: {a:?}"
    );
    assert!(!a.contains(&p(SP_HP, 40)), "no heal on a job level-up");
    assert_eq!(b, vec![effect(A, 1)]);

    // Base 1 -> 2 and job 4 -> 5 (file 1179-1203): the base tail inline, the one diff, the heal,
    // effect 0, then the job tail, effect 1 and both 0x0acc.
    let double = CharacterStatus {
        base_exp: 450,
        job_level: 4,
        job_exp: 27,
        skill_point: 3,
        ..CharacterStatus::default()
    };
    let (a, b, engine) = one_hit_kill(1, double);
    let base_burst = [
        dead_bar(mob, 1),
        p(SP_STATUSPOINT, 51),
        p(SP_TRAITPOINT, 0),
        p(SP_BASELEVEL, 2),
        long(SP_BASEEXP, 52),
        long(SP_NEXTBASEEXP, 894),
    ];
    assert_eq!(a[..6], base_burst, "{a:?}");
    let tail = [
        p(SP_HP, 45),
        p(SP_SP, 12),
        effect(A, 0),
        p(SP_JOBLEVEL, 5),
        long(SP_JOBEXP, 27),
        long(SP_NEXTJOBEXP, 91),
        p(SP_SKILLPOINT, 4),
        effect(A, 1),
        gained(A, SP_BASEEXP, 150),
        gained(A, SP_JOBEXP, 40),
    ];
    assert_eq!(a[a.len() - tail.len()..], tail, "{a:?}");
    assert!(a.contains(&p(6, 45)), "the diff has MAXHP 45: {a:?}");
    assert_eq!(b, vec![effect(A, 0), effect(A, 1)]);
    let progress = engine.progress_snapshot(char_id(A)).expect("A");
    assert_eq!(
        (
            progress.base_level,
            progress.status.job_level,
            progress.status.status_point,
            progress.status.skill_point,
            progress.status.base_exp,
            progress.status.job_exp,
            progress.status.hp,
        ),
        (2, 5, 51, 4, 52, 27, 45)
    );

    // Base only: the base burst, then SP_JOBEXP, then both 0x0acc (source order).
    let base_only = CharacterStatus {
        base_exp: 450,
        job_level: 9,
        ..CharacterStatus::default()
    };
    let (a, _, _) = one_hit_kill(1, base_only);
    assert_eq!(a[..6], base_burst, "{a:?}");
    assert_eq!(
        a[a.len() - 6..],
        [
            p(SP_HP, 45),
            p(SP_SP, 12),
            effect(A, 0),
            long(SP_JOBEXP, 40),
            gained(A, SP_BASEEXP, 150),
            gained(A, SP_JOBEXP, 40),
        ],
        "{a:?}"
    );

    // Novice job 10 is the max: the exp still accrues, the job 0x0acc shows 0.
    let at_max = CharacterStatus {
        job_level: 10,
        ..CharacterStatus::default()
    };
    let (a, b, engine) = one_hit_kill(1, at_max);
    assert_eq!(
        a,
        vec![
            dead_bar(mob, 1),
            long(SP_BASEEXP, 150),
            long(SP_JOBEXP, 40),
            gained(A, SP_BASEEXP, 150),
            gained(A, SP_JOBEXP, 0),
        ]
    );
    assert!(b.is_empty());
    let progress = engine.progress_snapshot(char_id(A)).expect("A");
    assert_eq!(
        (progress.status.job_level, progress.status.job_exp),
        (10, 40)
    );
}

/// ENG-CMB-10: the login burst sends the real NEXT values from the exp tables (captured fresh
/// Novice: 548 and 10).
#[test]
fn eng_cmb_10_login_sends_real_next_exp() {
    let mut engine = kill_world(1, Vec::new());
    let c = 3;
    let status = CharacterStatus {
        job_level: 9,
        ..CharacterStatus::default()
    };
    engine
        .spawn_character(fighter(c, (60, 60), 32, status), 0)
        .expect("spawn");
    let seen = seen_by(&send(&mut engine, c, ClientIntent::NotifyActorInit, 10), c);
    let next = |job_exp: bool, level: u16| {
        let jobs = jobs();
        let v = if job_exp {
            jobs.next_job_exp(NOVICE, level)
        } else {
            jobs.next_base_exp(NOVICE, level)
        };
        i64::try_from(v).expect("fits")
    };
    assert_eq!((next(false, 1), next(true, 1)), (548, 10), "captured bars");
    assert!(
        seen.contains(&long(SP_NEXTBASEEXP, next(false, 32))),
        "{seen:?}"
    );
    assert!(
        seen.contains(&long(SP_NEXTJOBEXP, next(true, 9))),
        "{seen:?}"
    );
    assert_eq!(next(true, 9), 340);
}

/// ENG-CMB-09 (final review finding 2): two mobs die in one tick and both pay A, the first with
/// a base level-up that queues the rest of its pushes. The values A ends the tick with must be
/// the server's, not the first kill's stale queued `SP_JOBEXP`.
#[test]
fn eng_cmb_09_two_kills_in_one_tick_end_on_live_values() {
    // Base 32 always hits a Poring (level 1 can miss).
    let (d, am) = bare_hit(32);
    // A is one exp short of base 33, at job 9 (bar 340: no job level-up).
    let a_status = CharacterStatus {
        base_exp: jobs().next_base_exp(NOVICE, 32) - 1,
        job_level: 9,
        ..CharacterStatus::default()
    };
    let job9 = CharacterStatus {
        job_level: 9,
        ..CharacterStatus::default()
    };
    let (m1, m2) = ((50, 50), (50, 52));
    let mut engine = kill_world_at(
        d + 1,
        vec![
            fighter(A, (51, 51), 32, a_status),
            // The same level and job, so their amotion matches A's.
            fighter(B, (49, 49), 32, job9),
            fighter(3, (49, 53), 32, job9),
        ],
        &[m1, m2],
    );
    let gids: Vec<u32> = engine.mobs().map(|m| m.gid).collect();
    let act = |target_id| ClientIntent::RequestAction {
        target_id,
        action_type: ACT_ATTACK,
    };
    // A leaves each Poring at 1 HP: mob 1 at +am, mob 2 at +3am.
    send(&mut engine, A, act(gids[0]), 1000);
    tick(&mut engine, 1000 + am);
    send(&mut engine, A, act(gids[1]), 1000 + 2 * am);
    // B and C finish both off with hits landing in the same tick, 1000 + 4am.
    engine.push_intent(intent(B, act(gids[0])));
    engine.push_intent(intent(3, act(gids[1])));
    tick(&mut engine, 1000 + 3 * am);
    let kills = seen_by(&tick(&mut engine, 1000 + 4 * am), A);
    let bars = kills
        .iter()
        .filter(|e| matches!(e, ServerEvent::MobHpBar { hp: 0, .. }))
        .count();
    assert_eq!(bars, 2, "both died in this tick: {kills:?}");
    let progress = engine.progress_snapshot(char_id(A)).expect("A");
    assert_eq!(progress.base_level, 33, "the first kill levelled base");
    let last = |var: u16| {
        kills.iter().rev().find_map(|e| match e {
            ServerEvent::LongLongParameterChanged { var_id, value } if *var_id == var => {
                Some(*value)
            }
            _ => None,
        })
    };
    let wire = |v: u64| Some(i64::try_from(v).expect("fits"));
    assert_eq!(
        last(SP_BASEEXP),
        wire(progress.status.base_exp),
        "{kills:?}"
    );
    assert_eq!(last(SP_JOBEXP), wire(progress.status.job_exp), "{kills:?}");
}
