//! Trait stats, zeny, skills and change levels (PER-TRAIT-01, PER-EFF-01, PER-SKILL-01).

use ro_core::{
    CharacterInventory, CharacterLooks, CharacterProgress, CharacterSelectStats, CharacterStatus,
};
use ro_persistence::{CharacterRepository, InventoryRepository};

use crate::common::*;

/// PER-TRAIT-01 (`specs/traits` §2.9): the trait columns, `trait_point` and AP save with the play
/// state and load back; the summary carries the select-screen values (HP/SP, max, points,
/// STR..LUK, exp).
#[tokio::test]
async fn per_trait_01_traits_and_select_values_round_trip() {
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    let status = CharacterStatus {
        str: 21,
        agi: 22,
        vit: 23,
        int: 24,
        dex: 25,
        luk: 26,
        status_point: 1325,
        skill_point: 11,
        base_exp: 6_000_000_000,
        job_exp: 77,
        hp: 8251,
        max_hp: 11_575,
        sp: 315,
        max_sp: 467,
        traits: [1, 2, 3, 4, 5, 110],
        trait_point: 19,
        ap: 150,
        zeny: 123_456,
        ..CharacterStatus::default()
    };
    let progress = CharacterProgress {
        job_class: 4252,
        base_level: 205,
        status,
        change_level: [0; 3],
    };
    repo.save_play_state(
        char_id,
        &CharacterInventory::default(),
        &CharacterLooks::default(),
        &progress,
        &[],
    )
    .await
    .expect("saves");
    let state = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(state.status, status);
    let summary = CharacterRepository::find_by_id(&repo, char_id)
        .await
        .expect("query")
        .expect("exists");
    assert_eq!(
        summary.select_stats,
        CharacterSelectStats {
            base_exp: 6_000_000_000,
            job_exp: 77,
            hp: 8251,
            max_hp: 11_575,
            sp: 315,
            max_sp: 467,
            status_point: 1325,
            skill_point: 11,
            stats: [21, 22, 23, 24, 25, 26],
            zeny: 123_456,
        }
    );
}

/// PER-EFF-01 (`specs/effects` §2.8): a new character has 0 zeny; zeny saves with the play state
/// (also covered by the round trip above) and the char-select summary carries it.
#[tokio::test]
async fn per_eff_01_zeny_defaults_to_zero_and_round_trips() {
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    let state = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(state.status.zeny, 0);
    let progress = CharacterProgress {
        job_class: 0,
        base_level: 1,
        status: CharacterStatus {
            zeny: 4000,
            ..state.status
        },
        change_level: [0; 3],
    };
    repo.save_play_state(
        char_id,
        &state.inventory,
        &CharacterLooks::default(),
        &progress,
        &[],
    )
    .await
    .expect("saves");
    let state = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(state.status.zeny, 4000);
    let summary = CharacterRepository::find_by_id(&repo, char_id)
        .await
        .expect("query")
        .expect("exists");
    assert_eq!(summary.select_stats.zeny, 4000);
}

/// PER-EFF-01 (`specs/effects` §2.8, final review): a stored zeny above `MAX_ZENY` (a hand edit)
/// loads capped, in the play state and in the char-select summary.
#[tokio::test]
async fn per_eff_01_loaded_zeny_is_capped() {
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    sqlx::query("UPDATE characters SET zeny = 1500000000 WHERE char_id = $1")
        .bind(i32::try_from(char_id.0).expect("fits"))
        .execute(repo.pool())
        .await
        .expect("updates");
    let state = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(state.status.zeny, ro_core::MAX_ZENY);
    let summary = CharacterRepository::find_by_id(&repo, char_id)
        .await
        .expect("query")
        .expect("exists");
    assert_eq!(summary.select_stats.zeny, ro_core::MAX_ZENY);
}

/// PER-SKILL-01 (`specs/skills` §2.9): the skill rows (levels above 0 only) and the class-change
/// levels save in the play-state transaction and load back; a later save rewrites the rows.
#[tokio::test]
async fn per_skill_01_skills_and_change_levels_round_trip() {
    use ro_core::{LearnedSkill, SkillFlag};
    let skill = |id, level, flag| LearnedSkill { id, level, flag };
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    let progress = CharacterProgress {
        job_class: 7,
        base_level: 50,
        status: CharacterStatus::default(),
        change_level: [45, 0, 0],
    };
    let first = [
        skill(1, 9, SkillFlag::Permanent),
        skill(2, 10, SkillFlag::Permanent),
        skill(3, 0, SkillFlag::Permanent),
        skill(142, 1, SkillFlag::Granted),
    ];
    repo.save_play_state(
        char_id,
        &CharacterInventory::default(),
        &CharacterLooks::default(),
        &progress,
        &first,
    )
    .await
    .expect("saves");
    let state = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(
        state.skills,
        [
            skill(1, 9, SkillFlag::Permanent),
            skill(2, 10, SkillFlag::Permanent),
            skill(142, 1, SkillFlag::Granted),
        ],
        "level 0 is not a row; ordered by id"
    );
    assert_eq!(state.change_level, [45, 0, 0]);

    // A later save replaces the rows (Knight levels gone after a job change).
    let progress = CharacterProgress {
        job_class: 2,
        change_level: [45, 50, 1],
        ..progress
    };
    repo.save_play_state(
        char_id,
        &CharacterInventory::default(),
        &CharacterLooks::default(),
        &progress,
        &[skill(1, 9, SkillFlag::Permanent)],
    )
    .await
    .expect("saves again");
    let state = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(state.skills, [skill(1, 9, SkillFlag::Permanent)]);
    assert_eq!(state.change_level, [45, 50, 1]);

    // No rows at all.
    repo.save_play_state(
        char_id,
        &CharacterInventory::default(),
        &CharacterLooks::default(),
        &progress,
        &[],
    )
    .await
    .expect("saves empty");
    let state = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert!(state.skills.is_empty());
}
