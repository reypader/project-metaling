//! Heal, and the `@zeny`/`@heal` commands (ENG-EFF-10, 11, 16).

use super::*;

/// ENG-EFF-10: Heal on self, on B, on a mob and at full HP: `0x0983` (efst 46, AID = caster,
/// 500) before the unclamped `0x09cb`; the target's own HP push; the mob's bar to its
/// damage-log members; nothing at full HP.
#[test]
fn eng_eff_10_heal_targets() {
    let mut w = heal_world(10, 9999, PORING);
    let amount = heal_amount(&w, A, 10);
    let sp = w.status(A).sp;
    let max_hp = w.status(A).max_hp;
    let to_a = seen_by(&w.cast(A, AL_HEAL, 10, gid(A)), A);
    assert_eq!(
        to_a,
        [
            casting(gid(A), gid(A), AL_HEAL, 6, 0),
            param(SP_SP, sp as i32 - 40),
            postdelay(gid(A), 500),
            no_damage(AL_HEAL, amount, gid(A), gid(A)),
            param(SP_HP, (10 + amount).min(max_hp as i32)),
        ]
    );

    // On B at full HP (B's max is small): the full amount, no push to anyone.
    let amotion = w.amotion(A);
    let t = w.now + 500.max(amotion + 100);
    w.run_to(A, t - 10);
    let out = w.at(
        t,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 10,
                skill_id: AL_HEAL,
                target: gid(B),
            },
        )],
    );
    let (to_a, to_b) = (seen_by(&out, A), seen_by(&out, B));
    assert_eq!(to_a[3], no_damage(AL_HEAL, amount, gid(B), gid(A)));
    assert_eq!(to_a.len(), 4, "{to_a:?}");
    assert_eq!(
        to_b,
        [
            casting(gid(A), gid(B), AL_HEAL, 6, 0),
            postdelay(gid(A), 500),
            no_damage(AL_HEAL, amount, gid(B), gid(A)),
        ]
    );

    // On B below max: B gets its own HP push, A doesn't.
    let mut w = heal_world(9999, 1, PORING);
    let out = w.cast(A, AL_HEAL, 1, gid(B));
    let b_max = w.status(B).max_hp;
    let to_b = seen_by(&out, B);
    assert_eq!(
        to_b.last(),
        Some(&param(SP_HP, (1 + heal_amount(&w, A, 1)).min(b_max as i32)))
    );
    assert!(!seen_by(&out, A)
        .iter()
        .any(|e| matches!(e, ServerEvent::ParameterChanged { var_id, .. } if *var_id == SP_HP)));

    // On a mob A has hit: its HP rises and its bar goes to A.
    let mut w = heal_world(9999, 9999, PORING);
    let mob = w.mob_gid();
    let t = w.now;
    drop(w.at(
        t,
        &[(
            A,
            ClientIntent::RequestAction {
                target_id: mob,
                action_type: 0,
            },
        )],
    ));
    w.run_to(A, t + 2000);
    let hurt = w.mob_hp();
    assert!(hurt < 1000);
    let to_a = seen_by(&w.cast(A, AL_HEAL, 1, mob), A);
    let healed = (hurt + heal_amount(&w, A, 1) as u32).min(1000);
    assert_eq!(
        to_a.last(),
        Some(&ServerEvent::MobHpBar {
            gid: mob,
            hp: healed,
            max_hp: 1000
        })
    );
    assert_eq!(w.mob_hp(), healed);
}

/// ENG-EFF-11: Heal on an Undead mob is dropped silently (deviation: rAthena deals holy
/// damage, captured in `scripting_2.txt`).
#[test]
fn eng_eff_11_heal_on_undead_is_dropped() {
    let mut w = heal_world(10, 9999, ZOMBIE);
    let mob = w.mob_gid();
    let sp = w.status(A).sp;
    assert_eq!(seen_by(&w.cast(A, AL_HEAL, 10, mob), A), []);
    assert_eq!(w.status(A).sp, sp);
}

/// ENG-EFF-16: `@zeny` adds (or removes) and pushes `0x00b1`, refusing a result out of range;
/// `@heal` fills HP/SP or adds the given amounts, keeping HP at 1 or more.
#[test]
fn eng_eff_16_zeny_and_heal_commands() {
    let mut w = first_aid_world(10);
    let say = |w: &mut World, line: &str| {
        let text = format!("{} : @{line}", name(A)).into_bytes();
        let now = w.now + 10;
        seen_by(&w.at(now, &[(A, ClientIntent::Chat { text })]), A)
    };
    let got = say(&mut w, "zeny 5000");
    assert_eq!(
        got[0],
        ServerEvent::LongParameterChanged {
            var_id: SP_ZENY,
            value: 5000
        }
    );
    assert!(matches!(got[1], ServerEvent::SelfChat { .. }));
    assert_eq!(w.status(A).zeny, 5000);
    let got = say(&mut w, "zeny -6000");
    assert!(matches!(got[..], [ServerEvent::SelfChat { .. }]), "{got:?}");
    assert_eq!(w.status(A).zeny, 5000);
    let got = say(&mut w, "zeny -5000");
    assert_eq!(
        got[0],
        ServerEvent::LongParameterChanged {
            var_id: SP_ZENY,
            value: 0
        }
    );

    // SP is full already: only HP is pushed.
    let status = w.status(A);
    let got = say(&mut w, "heal");
    assert_eq!(got[0], param(SP_HP, status.max_hp as i32));
    assert!(matches!(got[1], ServerEvent::SelfChat { .. }), "{got:?}");
    let got = say(&mut w, "heal -100000 -3");
    assert_eq!(
        got[..2],
        [param(SP_HP, 1), param(SP_SP, status.max_sp as i32 - 3)]
    );
    let got = say(&mut w, "heal 0 0");
    assert!(matches!(got[..], [ServerEvent::SelfChat { .. }]), "{got:?}");
}
