//! Trait points: level changes, resets and job changes (ENG-TRAIT-03..06).

use super::*;

/// ENG-TRAIT-03: level-ups add the trait-point table's gains (captured 436-474 B200, 551-569
/// B201, 577-600 B205): 7 at B200 is the `@job` grant alone, B201 adds 3, B201→B205 adds 16.
/// MaxAP 200 arrives in the B200 diff.
#[test]
fn eng_trait_03_level_ups_add_trait_points() {
    let mut world = World::new();
    world.login(spawn(DRAGON_KNIGHT, 199, dk_status([0; 6], 7, 0)));
    let out = world.command("blvl 1");
    assert_eq!(tail_trait_point(&out, 200), p(SP_TRAITPOINT, 7));
    assert!(
        index_of(&out, &p(SP_MAXAP, 200)) < index_of(&out, &p(SP_BASELEVEL, 200)),
        "MAXAP comes from the diff, before the tail: {out:?}"
    );
    for _ in 0..3 {
        raise(&mut world, SP_POW as u16, 1);
    }
    for _ in 0..4 {
        raise(&mut world, SP_CON, 1);
    }
    assert_eq!(world.status().trait_point, 0);
    let out = world.command("blvl 1");
    assert_eq!(tail_trait_point(&out, 201), p(SP_TRAITPOINT, 3));
    assert!(!out.contains(&p(SP_MAXAP, 200)), "MaxAP unchanged");
    let out = world.command("blvl 4");
    assert_eq!(tail_trait_point(&out, 205), p(SP_TRAITPOINT, 19));
    assert!(
        !out.iter()
            .any(|e| matches!(e, ServerEvent::ParameterChanged { var_id: SP_AP, .. })),
        "a level change never pushes AP: {out:?}"
    );
}

/// ENG-TRAIT-03: `@blvl -10` from B205 with 2 trait points (captured 720-782): the trait pool
/// alone triggers the reset (status points suffice). The reset pushes 4151 (4099 + 52) and 26
/// (19 + 7), then the message; after the subtraction the tail sends 3980 and 7, and the diff
/// drops MaxAP to 0.
#[test]
fn eng_trait_03_level_down_resets_on_the_trait_pool() {
    assert_eq!(
        (table_points(205), table_points(195)),
        (4099, 3928),
        "the captured tables"
    );
    let mut world = World::new();
    world.login(spawn(
        DRAGON_KNIGHT,
        205,
        dk_status([0, 0, 0, 0, 0, 17], 2, 0),
    ));
    let out = world.command("blvl -10");
    let reset = reset_pushes(1, 4151, 26);
    assert_eq!(out[..reset.len()], reset[..], "{out:?}");
    assert!(is_message(&out[reset.len()]), "{out:?}");
    let max_ap = index_of(&out, &p(SP_MAXAP, 0));
    let sp = out
        .iter()
        .position(|e| matches!(e, ServerEvent::ParameterChanged { var_id: SP_SP, .. }))
        .expect("SP in the diff");
    assert!(sp < max_ap, "{out:?}");
    let tail = base_tail(DRAGON_KNIGHT, 3980, 7, 195);
    assert_eq!(out[out.len() - tail.len()..], tail[..], "{out:?}");
    let status = world.status();
    assert_eq!(
        (status.status_point, status.trait_point, status.traits),
        (3980, 7, [0; 6])
    );

    // Enough of both pools: no reset, each pool just loses its share.
    let mut world = World::new();
    world.login(spawn(DRAGON_KNIGHT, 205, dk_status([0; 6], 20, 0)));
    let out = world.command("blvl -10");
    assert!(is_message(&out[0]), "no reset first: {out:?}");
    let status = world.status();
    assert_eq!((status.status_point, status.trait_point), (5000 - 171, 1));
}

/// ENG-TRAIT-04: `@streset` / `@resetstat` on a DK at B195 (captured 870-922): `0x00be` ×12
/// (trait costs 1), `SP_STATUSPOINT` 3980 (table + 52), `SP_TRAITPOINT` 7 (table 0 + 7), then
/// the diff (traits back to 0), then our message. The observer gets nothing.
#[test]
fn eng_trait_04_streset() {
    for command in ["streset", "resetstat"] {
        let mut world = World::new();
        let status = CharacterStatus {
            str: 50,
            ..dk_status([2, 3, 0, 0, 0, 0], 0, 0)
        };
        world.login(spawn(DRAGON_KNIGHT, 195, status));
        world.observer();
        let out = world.command(command);
        let reset = reset_pushes(1, 3980, 7);
        assert_eq!(out[..reset.len()], reset[..], "{command}: {out:?}");
        let diff = &out[reset.len()..out.len() - 1];
        for wanted in [
            couple(13, 1, 1),
            couple(SP_POW, 0, 1),
            couple(SP_POW + 1, 0, 0),
            p(SP_PATK, 0),
            p(SP_RES, 0),
        ] {
            assert!(diff.contains(&wanted), "{command}: {wanted:?} in {diff:?}");
        }
        assert!(is_message(&out[out.len() - 1]), "{command}: {out:?}");
        assert!(world.observed.is_empty(), "{command}: {:?}", world.observed);
        let status = world.status();
        assert_eq!(
            (
                status.str,
                status.traits,
                status.status_point,
                status.trait_point
            ),
            (1, [0; 6], 3980, 7)
        );
    }
}

/// ENG-TRAIT-05: `@job 4252` from a Knight at B99 (captured 352-399): +52 (1273 → 1325), the
/// stat costs, `SP_TRAITPOINT` 7, the trait costs (1), then the job pushes.
#[test]
fn eng_trait_05_into_a_trait_job() {
    assert_eq!(table_points(99), 1273);
    let mut world = World::new();
    let status = CharacterStatus {
        status_point: 1273,
        ..CharacterStatus::default()
    };
    world.login(spawn(KNIGHT, 99, status));
    let out = world.command("job 4252");
    let mut want = vec![p(SP_STATUSPOINT, 1325)];
    want.extend(stat_costs(STAT_COST_AT_1));
    want.push(p(SP_TRAITPOINT, 7));
    want.extend(trait_cost_pushes(1));
    want.push(p(SP_JOBLEVEL, 1));
    assert_eq!(out[..want.len()], want[..], "{out:?}");
    let status = world.status();
    assert_eq!((status.status_point, status.trait_point), (1325, 7));
}

/// ENG-TRAIT-05: the transcendent grant alone (capture `attribute_sequence_2.txt` 1214, 1264,
/// 1365-1371): Novice → Novice High +52; Novice High → Super Novice −52 with no reset (64
/// left); Super Novice → Hyper Novice +7 trait points and no `SP_STATUSPOINT` (not a primary
/// 4th class).
#[test]
fn eng_trait_05_transcendent_and_hyper_novice_grants() {
    let mut world = World::new();
    let status = CharacterStatus {
        status_point: 64,
        ..CharacterStatus::default()
    };
    world.login(spawn(0, 10, status));
    let job_level = p(SP_JOBLEVEL, 1);

    let out = world.command(&format!("job {NOVICE_HIGH}"));
    let mut want = vec![p(SP_STATUSPOINT, 116)];
    want.extend(stat_costs(STAT_COST_AT_1));
    want.push(job_level.clone());
    assert_eq!(out[..want.len()], want[..], "{out:?}");

    let out = world.command(&format!("job {SUPER_NOVICE}"));
    want[0] = p(SP_STATUSPOINT, 64);
    assert_eq!(out[..want.len()], want[..], "{out:?}");
    assert_eq!(world.status().str, 1, "no reset");

    let out = world.command(&format!("job {HYPER_NOVICE}"));
    let mut want = stat_costs(STAT_COST_AT_1);
    want.push(p(SP_TRAITPOINT, 7));
    want.extend(trait_cost_pushes(1));
    want.push(job_level);
    assert_eq!(out[..want.len()], want[..], "{out:?}");
    let status = world.status();
    assert_eq!((status.status_point, status.trait_point), (64, 7));
}

/// ENG-TRAIT-05/06: a DK at B99 with 0 trait points → Rune Knight T (`rathena_trait_sequence_2.txt`
/// 416-489): no ±52 (both classes count as upper or primary 4th); leaving the trait job forces a
/// reset, whose pool is +52 because Rune Knight T is upper (1325).
#[test]
fn eng_trait_06_trait_job_to_upper_third() {
    let mut world = World::new();
    world.login(spawn(
        DRAGON_KNIGHT,
        99,
        dk_status([2, 0, 0, 0, 0, 0], 0, 0),
    ));
    let out = world.command(&format!("job {RUNE_KNIGHT_T}"));
    let mut want = stat_costs(STAT_COST_AT_1);
    want.extend(reset_pushes(0, 1325, 0));
    want.push(p(SP_JOBLEVEL, 1));
    assert_eq!(out[..want.len()], want[..], "{out:?}");
    let status = world.status();
    assert_eq!((status.status_point, status.traits), (1325, [0; 6]));
}

/// ENG-TRAIT-06: trait job → trait job sends nothing extra and keeps the pool (DK ↔ Meister,
/// `rathena_trait_sequence_2.txt` 2029-2080).
#[test]
fn eng_trait_06_trait_job_to_trait_job() {
    let mut world = World::new();
    world.login(spawn(
        DRAGON_KNIGHT,
        99,
        dk_status([2, 0, 0, 0, 0, 0], 3, 0),
    ));
    let out = world.command(&format!("job {MEISTER}"));
    let mut want = stat_costs(STAT_COST_AT_1);
    want.push(p(SP_JOBLEVEL, 1));
    assert_eq!(out[..want.len()], want[..], "{out:?}");
    let status = world.status();
    assert_eq!((status.trait_point, status.traits[0]), (3, 2));
}

/// ENG-TRAIT-06: `@job 7` from a DK at B195 with < 7 trait points, the captured double reset
/// (`rathena_traits_sequence.txt` 956-1073): reset #1 (level cap, already as a Knight), the
/// base tail to B99, −52, the stat costs, reset #2 (the trait pool is now 0 < 7), then the job
/// pushes. rAthena's stale `0x0141` ×12 before each reset are skipped (spec §2.6).
#[test]
fn eng_trait_06_double_reset_from_b195() {
    let mut world = World::new();
    world.login(spawn(
        DRAGON_KNIGHT,
        195,
        dk_status([3, 0, 0, 0, 0, 0], 2, 0),
    ));
    let out = world.command("job 7");
    let mut want = reset_pushes(0, 1273, 0);
    want.extend(base_tail(KNIGHT, 1273, 0, 99));
    want.push(p(SP_STATUSPOINT, 1221));
    want.extend(stat_costs(STAT_COST_AT_1));
    want.extend(reset_pushes(0, 1273, 0));
    want.push(p(SP_JOBLEVEL, 1));
    assert_eq!(out[..want.len()], want[..], "{out:?}");
    let progress = world.progress();
    assert_eq!(
        (
            progress.base_level,
            progress.status.status_point,
            progress.status.trait_point,
            progress.status.traits
        ),
        (99, 1273, 0, [0; 6])
    );
}

/// ENG-TRAIT-06: from B99 with < 7 (captured 1185-1249): −52, the stat costs, one reset.
#[test]
fn eng_trait_06_single_reset_from_b99() {
    let mut world = World::new();
    let status = CharacterStatus {
        status_point: 1325,
        ..dk_status([0, 7, 0, 0, 0, 0], 0, 0)
    };
    world.login(spawn(DRAGON_KNIGHT, 99, status));
    let out = world.command("job 7");
    let mut want = vec![p(SP_STATUSPOINT, 1273)];
    want.extend(stat_costs(STAT_COST_AT_1));
    want.extend(reset_pushes(0, 1273, 0));
    want.push(p(SP_JOBLEVEL, 1));
    assert_eq!(out[..want.len()], want[..], "{out:?}");
    assert_eq!(world.status().traits, [0; 6]);
}

/// ENG-TRAIT-06: leaving with ≥ 7 unspent (source-only, catalog `trait_stats.md` "trait-job
/// exit"): the pool goes to 0 with `SP_TRAITPOINT` 0 and the trait costs (0, the new cap), and
/// the allocated traits are kept.
#[test]
fn eng_trait_06_enough_unspent_keeps_the_traits() {
    let mut world = World::new();
    let status = CharacterStatus {
        status_point: 1325,
        ..dk_status([3, 0, 0, 0, 2, 0], 9, 0)
    };
    world.login(spawn(DRAGON_KNIGHT, 99, status));
    let out = world.command("job 7");
    let mut want = vec![p(SP_STATUSPOINT, 1273)];
    want.extend(stat_costs(STAT_COST_AT_1));
    want.push(p(SP_TRAITPOINT, 0));
    want.extend(trait_cost_pushes(0));
    want.push(p(SP_JOBLEVEL, 1));
    assert_eq!(out[..want.len()], want[..], "{out:?}");
    let status = world.status();
    assert_eq!(
        (status.trait_point, status.traits, status.status_point),
        (0, [3, 0, 0, 0, 2, 0], 1273)
    );
    // The kept traits still count, with no job bonus: POW 3 → PATK 1.
    assert!(out.contains(&couple(SP_POW, 3, 0)), "{out:?}");
}
