//! Skills through the map-server (spec `specs/skills/spec.md` BIN-SKILL-01, BIN-SKILL-02).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::path::Path;

use map_server::{load_registries, MapConfig, RegistryPaths};

fn config(env: &[(&str, &str)]) -> MapConfig {
    let env: HashMap<String, String> = env
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    MapConfig::from_args_and_env(Vec::<String>::new(), |key| env.get(key).cloned())
}

/// BIN-SKILL-02: `SKILL_DATA_DIR` unset or pointing at a bad dir fails startup, naming it.
#[test]
fn bin_skill_02_skill_data_dir_is_required() {
    let unset = config(&[("JOB_DATA_DIR", "/data/job")]);
    let err = unset
        .check_required()
        .expect_err("SKILL_DATA_DIR is required");
    assert!(format!("{err:#}").contains("SKILL_DATA_DIR"), "{err:#}");
    let set = config(&[
        ("JOB_DATA_DIR", "/data/job"),
        ("SKILL_DATA_DIR", "/data/skill"),
    ]);
    set.check_required().expect("both set");
    assert_eq!(
        set.skill_data_dir.as_deref(),
        Some(Path::new("/data/skill"))
    );

    let missing = std::env::temp_dir().join("metaling-no-such-skill-dir");
    let err = load_registries(RegistryPaths {
        skills: Some(&missing),
        ..RegistryPaths::default()
    })
    .expect_err("a missing dir fails");
    assert!(
        format!("{err:#}").contains(&missing.display().to_string()),
        "{err:#}"
    );

    // A directory with the registry but a tree naming an unknown skill fails too.
    let dir = std::env::temp_dir().join(format!("metaling-bin-skill-02-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join(ro_db::SKILL_DATA_FILE),
        "Header: {Type: SKILL_DATA, Version: 1}\nBody:\n- {Id: 1, Name: NV_BASIC, MaxLevel: 9}\n",
    )
    .expect("write skills");
    std::fs::write(
        dir.join(ro_db::SKILL_TREE_DATA_FILE),
        "Header: {Type: SKILL_TREE_DATA, Version: 1}\nBody:\n- {Job: 0, Name: Novice, Tree: [{Skill: 2, MaxLevel: 1}]}\n",
    )
    .expect("write trees");
    let err = load_registries(RegistryPaths {
        skills: Some(&dir),
        ..RegistryPaths::default()
    })
    .expect_err("a bad tree fails");
    assert!(
        format!("{err:#}").contains(&dir.display().to_string()),
        "{err:#}"
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");

    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/ro-db/data/skill");
    let registries = load_registries(RegistryPaths {
        skills: Some(&shipped),
        ..RegistryPaths::default()
    })
    .expect("the shipped skill data loads");
    assert!(registries.skills.len() > 1000);
    assert!(!registries.skill_tree.tree(0).is_empty());
}

mod common;

use std::time::Duration;

use common::*;
use ro_core::{CharacterProgress, CharacterStatus, LearnedSkill, SkillFlag};
use ro_packets::RawPacket;

const A: u32 = 1;
const LONG: Duration = Duration::from_secs(2);
const SWORDMAN: u16 = 1;
const MAGE: u16 = 2;
const KNIGHT: u16 = 7;
const NV_BASIC: u16 = 1;
const SM_BASH: u16 = 5;
/// `0x010f` entry size and the offset of `level` in it.
const ENTRY: usize = 37;
const ENTRY_LEVEL: usize = 6;

fn chat(n: u32, message: &str) -> Vec<u8> {
    let text = format!("Char{n} : {message}");
    let len = u16::try_from(4 + text.len()).expect("short");
    let mut frame = vec![0xf3, 0x00];
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(text.as_bytes());
    frame
}

/// `CZ_UPGRADE_SKILLLEVEL`.
fn upgrade(id: u16) -> Vec<u8> {
    let mut frame = vec![0x12, 0x01];
    frame.extend_from_slice(&id.to_le_bytes());
    frame
}

fn row(id: u16, level: u8) -> LearnedSkill {
    LearnedSkill {
        id,
        level,
        flag: SkillFlag::Permanent,
    }
}

/// `(id, level)` of each `0x010f` entry.
fn list_levels(frame: &RawPacket) -> Vec<(u16, u16)> {
    let len = frame_len(frame);
    (0..(len - 4) / ENTRY)
        .map(|i| {
            let at = 4 + i * ENTRY;
            (frame_u16(frame, at), frame_u16(frame, at + ENTRY_LEVEL))
        })
        .collect()
}

fn login_list(frames: &[RawPacket]) -> Vec<(u16, u16)> {
    list_levels(
        frames
            .iter()
            .find(|f| f.opcode.0 == 0x010f)
            .expect("skill list on login"),
    )
}

/// BIN-SKILL-01: skill-ups survive a relog; `@job` to Mage from a Knight, a relog, `@job`
/// back: the Knight levels are gone and NV_BASIC 9 stays (captured 1039-1287).
#[tokio::test]
async fn bin_skill_01_skills_persist_and_hidden_levels_are_lost() {
    let repo = InMemoryRepo::seeded(&[(A, 100, 100)]);
    repo.set_gm(A);
    repo.set_progress(
        A,
        CharacterProgress {
            job_class: SWORDMAN,
            base_level: 50,
            status: CharacterStatus {
                job_level: 10,
                skill_point: 3,
                ..CharacterStatus::default()
            },
            change_level: [0; 3],
        },
    );
    repo.set_skills(A, vec![row(NV_BASIC, 9)]);
    let server = TestServer::start_with(repo, &[]).await;
    let (mut a, frames) = enter(&server, A).await;
    assert!(login_list(&frames).contains(&(SM_BASH, 0)));
    for _ in 0..3 {
        a.send(&upgrade(SM_BASH)).await;
        assert!(a.wait_opcode(0x010e, LONG).await.is_some(), "0x010e");
    }
    a.close().await;
    let repo = server.repo.clone();
    assert!(
        wait_until(|| repo.stored_skills(char_id(A)) == [row(NV_BASIC, 9), row(SM_BASH, 3)]).await,
        "saved on logout: {:?}",
        repo.stored_skills(char_id(A))
    );

    let (mut a, frames) = enter(&server, A).await;
    assert!(
        login_list(&frames).contains(&(SM_BASH, 3)),
        "{:?}",
        login_list(&frames)
    );
    // Swordman -> Knight (records the 2nd-class level), Knight -> Mage: each saves at once.
    for (job, name) in [(KNIGHT, "knight"), (MAGE, "mage")] {
        a.send(&chat(A, &format!("@job {name}"))).await;
        assert!(a.wait_opcode(0x010f, LONG).await.is_some(), "@job {name}");
        assert!(
            wait_until(|| repo.stored_progress(A).is_some_and(|p| p.job_class == job)).await,
            "@job {name} saved"
        );
    }
    assert_eq!(
        repo.stored_progress(A).map(|p| p.change_level),
        Some([10, 0, 0])
    );
    assert_eq!(
        repo.stored_skills(char_id(A)),
        [row(NV_BASIC, 9)],
        "Bash hidden: not saved"
    );
    a.close().await;

    let (mut a, frames) = enter(&server, A).await;
    assert_eq!(login_list(&frames)[0], (NV_BASIC, 9));
    a.send(&chat(A, "@job knight")).await;
    let list = a.wait_opcode(0x010f, LONG).await.expect("@job knight");
    let levels = list_levels(&list);
    assert!(levels.contains(&(NV_BASIC, 9)), "{levels:?}");
    assert!(
        levels.contains(&(SM_BASH, 0)),
        "the Knight levels are gone: {levels:?}"
    );
}
