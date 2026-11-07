//! Raising skills: unlocks, refusals, level gates and `@allskill` (ENG-SKILL-05, 06, 09, 11).

use super::*;

/// ENG-SKILL-11: `@blvl` / `@jlvl` mark the tree; a gated entry unlocking sends the list in
/// the tick's pass, before the diff; no change, no list.
#[test]
fn eng_skill_11_level_gates() {
    let mut world = World::with_data(gated_skills(), SWORDMAN, (49, 39), &[(NV_BASIC, 9)], 0);
    assert_eq!(levels(&only_list(&world.login)), [(1, 9), (2, 0)]);
    let (a, _) = world.command("blvl 1");
    let list = only_list(&a);
    assert_eq!(levels(&list), [(1, 9), (2, 0), (900, 0)]);
    // The list comes before the recalc's diff and the queued follow-ups (the message).
    let at = position(&a, |e| matches!(e, ServerEvent::SkillList { .. }));
    let message = position(&a, |e| matches!(e, ServerEvent::SelfChat { .. }));
    assert!(at < message, "{a:?}");
    let diff = position(&a, |e| {
        matches!(
            e,
            ServerEvent::CoupleStatus { .. } | ServerEvent::ParameterChanged { .. }
        )
    });
    assert!(at < diff, "{a:?}");
    // Another level: nothing new, no list.
    let (a, _) = world.command("blvl 1");
    assert!(lists(&a).is_empty(), "{a:?}");

    // @jlvl: its pushes go out at once; the list follows in the pass.
    let (a, _) = world.command("jlvl 1");
    let list = only_list(&a);
    assert!(list.iter().any(|s| s.id == 901));
    let job_level = position(
        &a,
        |e| matches!(e, ServerEvent::ParameterChanged { var_id, .. } if *var_id == SP_JOBLEVEL),
    );
    let points = position(
        &a,
        |e| matches!(e, ServerEvent::ParameterChanged { var_id, .. } if *var_id == SP_SKILLPOINT),
    );
    let at = position(&a, |e| matches!(e, ServerEvent::SkillList { .. }));
    assert!(job_level < points && points < at, "{a:?}");
    let (a, _) = world.command("jlvl 1");
    assert!(lists(&a).is_empty(), "{a:?}");
}

/// ENG-SKILL-05: the captured skill-up replies.
#[test]
fn eng_skill_05_novice_basic_skill() {
    // A Novice spends 9 points on NV_BASIC in one burst (captured 472-507): 9 x (`0x010e`,
    // `SP_SKILLPOINT` 8..0, list), no pre-list, level 9 `upFlag` 0.
    let mut world = World::new(NOVICE, (1, 10), &[], 9);
    let (a, b) = world.upgrade(&[NV_BASIC; 9]);
    let groups = reply_groups(&a);
    assert_eq!(groups.len(), 9, "{a:?}");
    for (i, group) in groups.iter().enumerate() {
        let level = i as u16 + 1;
        assert_eq!(group.len(), 3, "no pre-list: {group:?}");
        assert_eq!(group[0], update(NV_BASIC, level, 0, 0, level < 9));
        assert_eq!(group[1], skill_point(8 - i as i32));
        assert_eq!(ids_of(&group[2]), [NV_BASIC]);
    }
    assert!(!b.iter().any(is_skill_event), "the observer gets nothing");
    // The tick's pass sends nothing more.
    let out = world.tick(&[]);
    assert!(!seen_by(&out, A).iter().any(is_skill_event));
}

#[test]
fn eng_skill_05_passive_unlock_sends_a_pre_list() {
    // Swordman: SM_SWORD 1 opens Two-Handed: the new list first, then the usual three
    // (captured 594-597).
    let mut world = World::new(SWORDMAN, (1, 10), &[(NV_BASIC, 9)], 9);
    let (a, _) = world.upgrade(&[SM_SWORD]);
    let groups = reply_groups(&a);
    assert_eq!(groups.len(), 1);
    let g = &groups[0];
    assert_eq!(g.len(), 4, "{g:?}");
    assert_eq!(ids_of(&g[0]), [1, 2, 3, 4, 5, 6]);
    assert_eq!(g[1], update(SM_SWORD, 1, 0, 0, true));
    assert_eq!(g[2], skill_point(8));
    assert_eq!(ids_of(&g[3]), [1, 2, 3, 4, 5, 6]);

    // An active skill never sends a pre-list: Bash x5, the 5th's trailing list adds Magnum
    // (captured 607-626).
    let (a, _) = world.upgrade(&[SM_BASH; 5]);
    let groups = reply_groups(&a);
    assert_eq!(groups.len(), 5);
    for (i, g) in groups.iter().enumerate() {
        assert_eq!(g.len(), 3, "{g:?}");
        assert_eq!(g[0], update(SM_BASH, i as u16 + 1, 8, 1, true));
        assert_eq!(g[1], skill_point(7 - i as i32));
    }
    assert_eq!(ids_of(&groups[3][2]), [1, 2, 3, 4, 5, 6]);
    assert_eq!(ids_of(&groups[4][2]), [1, 2, 3, 4, 5, 6, 7]);

    // NV_BASIC 9 on a Swordman that hadn't spent it opens the Swordman tree (captured 1436).
    let mut world = World::new(SWORDMAN, (1, 10), &[(NV_BASIC, 8)], 1);
    assert_eq!(levels(&only_list(&world.login)), [(1, 8)]);
    let (a, _) = world.upgrade(&[NV_BASIC]);
    let g = &reply_groups(&a)[0];
    assert_eq!(g.len(), 4, "{g:?}");
    assert_eq!(ids_of(&g[0]), [1, 2, 4, 5, 6]);
    assert_eq!(g[1], update(NV_BASIC, 9, 0, 0, false));
}

#[test]
fn eng_skill_05_burst_sees_earlier_unlocks() {
    // The 14-request burst (captured 697-710 -> 711-763): Sword, Two-Handed, Bash x5, Magnum,
    // Provoke x5, Endure; all accepted, 44 -> 30 points.
    let mut world = World::new(SWORDMAN, (1, 45), &[(NV_BASIC, 9)], 44);
    let mut burst = vec![SM_SWORD, SM_TWOHAND];
    burst.extend([SM_BASH; 5]);
    burst.push(SM_MAGNUM);
    burst.extend([SM_PROVOKE; 5]);
    burst.push(SM_ENDURE);
    let (a, _) = world.upgrade(&burst);
    let groups = reply_groups(&a);
    assert_eq!(groups.len(), 14, "{a:?}");
    let updates: Vec<ServerEvent> = a
        .iter()
        .filter(|e| matches!(e, ServerEvent::SkillUpdate { .. }))
        .cloned()
        .collect();
    assert_eq!(updates.len(), 14);
    assert_eq!(updates[1], update(SM_TWOHAND, 1, 0, 0, true));
    assert_eq!(updates[7], update(SM_MAGNUM, 1, 30, 0, true));
    assert_eq!(updates[12], update(SM_PROVOKE, 5, 8, 9, true));
    assert_eq!(updates[13], update(SM_ENDURE, 1, 10, 0, true));
    assert_eq!(groups[13][groups[13].len() - 2], skill_point(30));
    // Two-Handed is passive but opens nothing: no pre-list (captured 725).
    assert_eq!(groups[1].len(), 3);
}

/// ENG-SKILL-06: refusals send nothing.
#[test]
fn eng_skill_06_refusals_are_silent() {
    let quiet = |world: &mut World, id: u16| {
        let (a, _) = world.upgrade(&[id]);
        assert!(
            !a.iter().any(|e| is_skill_event(e) || is_points(e)),
            "{id}: {a:?}"
        );
    };
    // Unknown id; a hidden id with points (captured 1395-1397, 1559-1575).
    let mut world = World::new(SWORDMAN, (1, 10), &[(NV_BASIC, 9)], 9);
    quiet(&mut world, 60_000);
    quiet(&mut world, SM_TWOHAND);
    quiet(&mut world, KN_SPEARMASTERY);
    // No points (the client blocks this; source-only).
    let mut world = World::new(SWORDMAN, (1, 10), &[(NV_BASIC, 9)], 0);
    quiet(&mut world, SM_BASH);
    // At the tree max (the client blocks this; source-only).
    let mut world = World::new(NOVICE, (1, 10), &[(NV_BASIC, 9)], 3);
    quiet(&mut world, NV_BASIC);
    // A granted slot.
    let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("dims"));
    let (skills, trees) = shipped_skills();
    let mut engine = MapInstance::new("prontera.gat", grid);
    engine.set_registries(Registries {
        jobs: jobs(),
        skills,
        skill_tree: trees,
        ..Registries::default()
    });
    let mut world = World {
        engine,
        now: 0,
        login: Vec::new(),
    };
    let mut rows = learned(&[(NV_BASIC, 9)]);
    rows.push(LearnedSkill {
        id: SM_BASH,
        level: 1,
        flag: SkillFlag::Granted,
    });
    let status = CharacterStatus {
        job_level: 10,
        skill_point: 5,
        ..CharacterStatus::default()
    };
    world.login = world.enter(A, SWORDMAN, 1, status, rows, 99);
    assert!(levels(&only_list(&world.login)).contains(&(SM_BASH, 1)));
    assert!(!entry(&only_list(&world.login), SM_BASH).upgradable);
    quiet(&mut world, SM_BASH);
}

/// ENG-SKILL-09: `@allskill` on a Swordman (captured 895-898).
#[test]
fn eng_skill_09_all_skills() {
    let mut world = World::new(SWORDMAN, (1, 45), &[(NV_BASIC, 9)], 44);
    let (a, _) = world.command("allskill");
    assert_eq!(
        skill_trace(&a),
        [
            "L[(1, 9), (2, 10), (3, 10), (4, 10), (5, 10), (6, 10), (7, 10), (8, 10)]",
            "P0"
        ]
    );
    let list = only_list(&a);
    assert!(list.iter().all(|s| !s.upgradable), "all at the tree max");
    assert_eq!(
        (entry(&list, SM_BASH).sp, entry(&list, SM_BASH).range),
        (15, 1)
    );
    let points = position(&a, |e| is_var(e, SP_SKILLPOINT));
    let message = position(&a, |e| matches!(e, ServerEvent::SelfChat { .. }));
    assert!(points < message);
    // No deletes and nothing more from the tick's pass.
    let out = world.tick(&[]);
    assert!(!seen_by(&out, A).iter().any(is_skill_event));
    // The aliases.
    for alias in ["allskills", "skillall", "skillsall"] {
        let (a, _) = world.command(alias);
        assert_eq!(lists(&a).len(), 1, "@{alias}");
    }
}
