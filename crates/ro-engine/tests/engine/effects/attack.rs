//! Attack skills: Bash and Mammonite (ENG-EFF-05..07, 09, 17).

use super::*;

/// ENG-EFF-05: a Bash hit: `0x0b1a` (element 0), SP, `0x01de` (sdelay amotion, ddelay dmotion,
/// type 8); the HP, `0x0977` and the kill land at `+amotion`.
#[test]
fn eng_eff_05_bash_hit_and_kill() {
    let mut w = bash_world(100_000, 1);
    let (mob, amotion, sp) = (w.mob_gid(), w.amotion(A), w.status(A).sp);
    let t = w.now;
    let to_a = seen_by(&w.cast(A, SM_BASH, 10, mob), A);
    assert_eq!(to_a.len(), 3, "{to_a:?}");
    assert_eq!(to_a[0], casting(gid(A), mob, SM_BASH, 0, 0));
    assert_eq!(to_a[1], param(SP_SP, sp as i32 - 15));
    let ServerEvent::SkillDamage {
        skill_id,
        src,
        dst,
        src_speed,
        dmg_speed,
        damage,
        level,
        div,
        damage_type,
        ..
    } = to_a[2].clone()
    else {
        panic!("third is 0x01de: {to_a:?}");
    };
    assert_eq!(
        (
            skill_id,
            src,
            dst,
            src_speed as u64,
            dmg_speed,
            level,
            div,
            damage_type
        ),
        (SM_BASH, gid(A), mob, amotion, MOB_DMOTION, 10, 1, 8)
    );
    assert!(damage > 0);
    assert_eq!(w.mob_hp(), 100_000, "the HP lands later");
    let later = w.run_to(A, t + amotion);
    assert_eq!(
        later,
        [ServerEvent::MobHpBar {
            gid: mob,
            hp: 100_000 - damage as u32,
            max_hp: 100_000,
        }]
    );

    // A kill: exp at +amotion, the death vanish 250 ms later.
    let mut w = bash_world(10, 1);
    let (mob, t) = (w.mob_gid(), w.now);
    drop(w.cast(A, SM_BASH, 10, mob));
    let later = w.run_to(A, t + amotion + 250);
    assert!(
        later
            .iter()
            .any(|e| matches!(e, ServerEvent::ExpGained { .. })),
        "{later:?}"
    );
    assert!(later.contains(&ServerEvent::UnitVanished {
        gid: mob,
        vanish_type: 1
    }));
}

/// ENG-EFF-06: a miss is `0x01de` damage 0 and ddelay 0, the SP is still spent, and no
/// `0x0977` follows. A Poring with very high FLEE makes misses common; the seeded roll decides.
#[test]
fn eng_eff_06_bash_miss() {
    let mut w = bash_world(100_000, 999);
    let (mob, amotion) = (w.mob_gid(), w.amotion(A));
    for _ in 0..20 {
        let sp = w.status(A).sp;
        let t = w.now;
        let to_a = seen_by(&w.cast(A, SM_BASH, 1, mob), A);
        let Some(ServerEvent::SkillDamage {
            damage, dmg_speed, ..
        }) = skill_damage(&to_a)
        else {
            panic!("no 0x01de: {to_a:?}");
        };
        let later = w.run_to(A, t + amotion + 200);
        if damage == 0 {
            assert_eq!(dmg_speed, 0);
            assert_eq!(w.status(A).sp, sp - 8);
            assert!(
                !later
                    .iter()
                    .any(|e| matches!(e, ServerEvent::MobHpBar { .. })),
                "{later:?}"
            );
            return;
        }
    }
    panic!("no miss in 20 casts at 5 % hit");
}

/// ENG-EFF-07: a Bash stops the auto-attack loop, which doesn't resume; a cast in the tick of a
/// due swing finds the swing's gate and is dropped.
#[test]
fn eng_eff_07_bash_during_attack_loop() {
    let mut w = bash_world(1_000_000, 1);
    let mob = w.mob_gid();
    let t0 = w.now;
    let out = w.at(
        t0,
        &[(
            A,
            ClientIntent::RequestAction {
                target_id: mob,
                action_type: 7,
            },
        )],
    );
    assert!(seen_by(&out, A)
        .iter()
        .any(|e| matches!(e, ServerEvent::Damage { .. })));
    let amotion = w.amotion(A);
    let adelay = 2 * amotion;
    // The second swing is due at t0 + adelay; a cast in that tick is dropped.
    let out = w.at(
        t0 + adelay,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: SM_BASH,
                target: mob,
            },
        )],
    );
    let to_a = seen_by(&out, A);
    assert!(
        to_a.iter().any(|e| matches!(e, ServerEvent::Damage { .. })),
        "the swing ran: {to_a:?}"
    );
    assert!(
        !to_a
            .iter()
            .any(|e| matches!(e, ServerEvent::SkillCasting { .. })),
        "the cast was dropped: {to_a:?}"
    );
    // Once the gate passes, the cast goes through and the loop stops for good.
    let t1 = t0 + adelay + amotion + 100;
    w.run_to(A, t1 - 10);
    let out = w.at(
        t1,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: SM_BASH,
                target: mob,
            },
        )],
    );
    assert!(skill_damage(&seen_by(&out, A)).is_some());
    let later = w.run_to(A, t1 + 5 * adelay);
    assert!(
        !later
            .iter()
            .any(|e| matches!(e, ServerEvent::Damage { .. })),
        "no swing after the cast: {later:?}"
    );
}

/// ENG-EFF-09: Mammonite pays SP then zeny (`0x00b1` with the new total) before `0x01de`.
#[test]
fn eng_eff_09_mammonite_pays_zeny() {
    let mut w = World::new(
        vec![Player::new(A, MERCHANT, NEXT_TO_MOB, &[(MC_MAMMONITE, 10)]).status(|s| s.zeny = 5000)],
        PORING,
        mobs(1_000_000, 1),
    );
    let (mob, sp) = (w.mob_gid(), w.status(A).sp);
    let to_a = seen_by(&w.cast(A, MC_MAMMONITE, 10, mob), A);
    assert_eq!(to_a[0], casting(gid(A), mob, MC_MAMMONITE, 0, 0));
    assert_eq!(to_a[1], param(SP_SP, sp as i32 - 5));
    assert_eq!(
        to_a[2],
        ServerEvent::LongParameterChanged {
            var_id: SP_ZENY,
            value: 4000
        }
    );
    assert!(matches!(to_a[3], ServerEvent::SkillDamage { .. }));
    assert_eq!(w.status(A).zeny, 4000);
}

/// ENG-EFF-17: Bash on a player begins (`0x0b1a` to the area) and fails at castend with
/// `0x0110` cause 0; no SP, no damage; `canact` keeps cast begin's `+ amotion + 200`.
#[test]
fn eng_eff_17_attack_skill_on_player() {
    let mut w = World::new(
        vec![
            Player::new(A, SWORDMAN, (40, 40), &[(SM_BASH, 10)]),
            Player::new(B, NOVICE, (41, 40), &[]),
        ],
        PORING,
        mobs(1000, 1),
    );
    let (sp, amotion, t) = (w.status(A).sp, w.amotion(A), w.now);
    let out = w.cast(A, SM_BASH, 10, gid(B));
    assert_eq!(
        seen_by(&out, A),
        [casting(gid(A), gid(B), SM_BASH, 0, 0), fail(SM_BASH, 0)]
    );
    assert_eq!(seen_by(&out, B), [casting(gid(A), gid(B), SM_BASH, 0, 0)]);
    assert_eq!(w.status(A).sp, sp);
    let retry = |w: &mut World, now: u64| {
        seen_by(
            &w.at(
                now,
                &[(
                    A,
                    ClientIntent::UseSkill {
                        level: 10,
                        skill_id: SM_BASH,
                        target: gid(B),
                    },
                )],
            ),
            A,
        )
    };
    assert_eq!(retry(&mut w, t + amotion + 190), []);
    assert_eq!(retry(&mut w, t + amotion + 200)[1], fail(SM_BASH, 0));
}
