//! Skills, learn only (spec `specs/skills/spec.md` §2, ENG-SKILL-03..11), with the shipped job
//! and skill data read from `crates/ro-db/data/` (and hand-written trees where a gate is needed).
//! The orders are the captured ones (catalog `mechanics/skills.md` §10;
//! `reference/catalog/captures/rathena_skills_sequence.txt`, file lines).

use std::path::Path;
use std::sync::{Arc, OnceLock};

use ro_core::{
    AccountId, CharacterInventory, CharacterSpawn, CharacterStatus, ClientIntent, LearnedSkill,
    MapPosition, ScopedServerEvent, ServerEvent, SessionId, SessionIntent, Sex, SkillFlag,
    SkillInfo,
};
use ro_db::{JobDb, MapGrid, SkillDb, SkillTreeDb};
use ro_engine::{MapInstance, Registries};

use crate::common::{
    char_id, gid, learned, seen_by, A, ASSASSIN, B, BARD, KNIGHT, NOVICE, NV_BASIC, SM_BASH,
    SP_JOBLEVEL, SP_MAXWEIGHT, SP_SKILLPOINT, SP_STATUSPOINT, SP_TRAITPOINT, SWORDMAN,
};

mod job_and_reset;
mod skill_list;
mod skill_up;

const MAGE: u16 = 2;

const SM_SWORD: u16 = 2;
const SM_TWOHAND: u16 = 3;
const SM_PROVOKE: u16 = 6;
const SM_MAGNUM: u16 = 7;
const SM_ENDURE: u16 = 8;
const KN_SPEARMASTERY: u16 = 55;

fn data_dir(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ro-db/data")
        .join(name)
}

fn jobs() -> Arc<JobDb> {
    static JOBS: OnceLock<Arc<JobDb>> = OnceLock::new();
    JOBS.get_or_init(|| Arc::new(JobDb::load_dir(&data_dir("job")).expect("shipped jobs")))
        .clone()
}

fn shipped_skills() -> (Arc<SkillDb>, Arc<SkillTreeDb>) {
    static SKILLS: OnceLock<(Arc<SkillDb>, Arc<SkillTreeDb>)> = OnceLock::new();
    SKILLS
        .get_or_init(|| {
            let dir = data_dir("skill");
            let skills = SkillDb::load_dir(&dir).expect("shipped skills");
            let trees = SkillTreeDb::load_dir(&dir, &skills).expect("shipped trees");
            (Arc::new(skills), Arc::new(trees))
        })
        .clone()
}

/// Hand-written: a Swordman tree with one skill gated on base level 50 and one on job level
/// 40 (no shipped first-class tree has a gate).
fn gated_skills() -> (Arc<SkillDb>, Arc<SkillTreeDb>) {
    let skills = SkillDb::from_rows(
        "gated",
        SkillDb::parse(
            "gated",
            "Header: {Type: SKILL_DATA, Version: 2}
Body:
- {Id: 1, Name: NV_BASIC, MaxLevel: 9}
- {Id: 2, Name: SM_SWORD, MaxLevel: 10}
- {Id: 900, Name: GATED_BASE, MaxLevel: 1}
- {Id: 901, Name: GATED_JOB, MaxLevel: 1}
",
        )
        .expect("parse"),
    )
    .expect("skills");
    let trees = SkillTreeDb::from_rows(
        "gated",
        SkillTreeDb::parse(
            "gated",
            "Header: {Type: SKILL_TREE_DATA, Version: 1}
Body:
- Job: 0
  Name: Novice
  Tree:
  - {Skill: 1, MaxLevel: 9}
- Job: 1
  Name: Swordman
  Tree:
  - {Skill: 1, MaxLevel: 9}
  - {Skill: 2, MaxLevel: 10}
  - {Skill: 900, MaxLevel: 1, BaseLevel: 50}
  - {Skill: 901, MaxLevel: 1, JobLevel: 40}
",
        )
        .expect("parse"),
        &skills,
    )
    .expect("trees");
    (Arc::new(skills), Arc::new(trees))
}

fn name(n: u32) -> String {
    format!("Skiller{n}")
}

struct World {
    engine: MapInstance,
    now: u64,
    /// What A got when it entered.
    login: Vec<ServerEvent>,
}

impl World {
    /// A as `job` at `base_level`/`job_level` with `skills` learned and `skill_point` free,
    /// with the shipped skill data; B (a Novice) beside it.
    fn new(job: u16, levels: (u16, u16), skills: &[(u16, u8)], skill_point: u16) -> Self {
        Self::with_data(shipped_skills(), job, levels, skills, skill_point)
    }

    fn with_data(
        (skills_db, trees): (Arc<SkillDb>, Arc<SkillTreeDb>),
        job: u16,
        (base_level, job_level): (u16, u16),
        skills: &[(u16, u8)],
        skill_point: u16,
    ) -> Self {
        let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("dims"));
        let mut engine = MapInstance::new("prontera.gat", grid);
        engine.set_registries(Registries {
            jobs: jobs(),
            skills: skills_db,
            skill_tree: trees,
            ..Registries::default()
        });
        let mut world = Self {
            engine,
            now: 0,
            login: Vec::new(),
        };
        let status = CharacterStatus {
            job_level,
            skill_point,
            ..CharacterStatus::default()
        };
        world.login = world.enter(A, job, base_level, status, learned(skills), 99);
        world.enter(B, NOVICE, 1, CharacterStatus::default(), Vec::new(), 0);
        world
    }

    fn enter(
        &mut self,
        n: u32,
        job: u16,
        base_level: u16,
        status: CharacterStatus,
        skills: Vec<LearnedSkill>,
        group_id: i32,
    ) -> Vec<ServerEvent> {
        let spawn = CharacterSpawn {
            account_id: AccountId(gid(n)),
            char_id: char_id(n),
            name: name(n),
            sex: Sex::Male,
            job_class: job,
            base_level,
            hair_style: 3,
            hair_color: 1,
            pos: MapPosition {
                x: 50 + n as u16,
                y: 50,
                dir: 0,
            },
            speed: 150,
            status,
            group_id,
            inventory: CharacterInventory::default(),
            skills,
            hidden_skills: Vec::new(),
            change_level: [0; 3],
        };
        self.engine.spawn_character(spawn, 0).expect("spawn");
        let out = self.tick(&[(n, ClientIntent::NotifyActorInit)]);
        seen_by(&out, n)
    }

    fn tick(&mut self, intents: &[(u32, ClientIntent)]) -> Vec<ScopedServerEvent> {
        for (n, intent) in intents {
            self.engine.push_intent(SessionIntent::new(
                SessionId(u64::from(*n)),
                Some(AccountId(gid(*n))),
                Some(char_id(*n)),
                intent.clone(),
            ));
        }
        self.now += 50;
        let mut out = Vec::new();
        self.engine.step_tick(self.now, &mut out).expect("tick");
        out
    }

    /// A says `@<line>`; what (A, B) got that tick.
    fn command(&mut self, line: &str) -> (Vec<ServerEvent>, Vec<ServerEvent>) {
        let text = format!("{} : @{line}", name(A)).into_bytes();
        let out = self.tick(&[(A, ClientIntent::Chat { text })]);
        (seen_by(&out, A), seen_by(&out, B))
    }

    fn change_level(&self) -> [u16; 3] {
        self.engine
            .progress_snapshot(char_id(A))
            .expect("A is on the map")
            .change_level
    }
}

/// The skill lists in `events`.
fn lists(events: &[ServerEvent]) -> Vec<Vec<SkillInfo>> {
    events
        .iter()
        .filter_map(|e| match e {
            ServerEvent::SkillList { skills } => Some(skills.clone()),
            _ => None,
        })
        .collect()
}

fn only_list(events: &[ServerEvent]) -> Vec<SkillInfo> {
    let mut all = lists(events);
    assert_eq!(all.len(), 1, "one list: {events:?}");
    all.remove(0)
}

/// `(id, level)` of each entry.
fn levels(list: &[SkillInfo]) -> Vec<(u16, u16)> {
    list.iter().map(|s| (s.id, s.level)).collect()
}

fn entry(list: &[SkillInfo], id: u16) -> SkillInfo {
    list.iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("{id} listed in {list:?}"))
        .clone()
}

fn is_skill_event(e: &ServerEvent) -> bool {
    matches!(
        e,
        ServerEvent::SkillList { .. }
            | ServerEvent::SkillUpdate { .. }
            | ServerEvent::SkillDelete { .. }
    )
}

fn position(events: &[ServerEvent], wanted: impl Fn(&ServerEvent) -> bool) -> usize {
    events
        .iter()
        .position(wanted)
        .unwrap_or_else(|| panic!("not found in {events:?}"))
}

impl World {
    /// A sends `0x0112` for each id, all in one tick; what (A, B) got.
    fn upgrade(&mut self, ids: &[u16]) -> (Vec<ServerEvent>, Vec<ServerEvent>) {
        let intents: Vec<(u32, ClientIntent)> = ids
            .iter()
            .map(|&id| (A, ClientIntent::UpgradeSkill { id }))
            .collect();
        let out = self.tick(&intents);
        (seen_by(&out, A), seen_by(&out, B))
    }
}

fn update(id: u16, level: u16, sp: u16, range: u16, upgradable: bool) -> ServerEvent {
    ServerEvent::SkillUpdate {
        id,
        level,
        sp,
        range,
        upgradable,
    }
}

fn skill_point(value: i32) -> ServerEvent {
    ServerEvent::ParameterChanged {
        var_id: SP_SKILLPOINT,
        value,
    }
}

/// Splits A's events into reply groups, each ending at a list that follows an
/// `SP_SKILLPOINT` (a pre-list belongs to the next group).
fn reply_groups(events: &[ServerEvent]) -> Vec<Vec<ServerEvent>> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    let mut seen_points = false;
    for e in events.iter().filter(|e| is_skill_event(e) || is_points(e)) {
        current.push(e.clone());
        if is_points(e) {
            seen_points = true;
        }
        if seen_points && matches!(e, ServerEvent::SkillList { .. }) {
            out.push(std::mem::take(&mut current));
            seen_points = false;
        }
    }
    assert!(current.is_empty(), "trailing {current:?}");
    out
}

fn is_points(e: &ServerEvent) -> bool {
    matches!(e, ServerEvent::ParameterChanged { var_id, .. } if *var_id == SP_SKILLPOINT)
}

fn ids_of(list: &ServerEvent) -> Vec<u16> {
    match list {
        ServerEvent::SkillList { skills } => skills.iter().map(|s| s.id).collect(),
        other => panic!("not a list: {other:?}"),
    }
}

fn is_var(e: &ServerEvent, var: u16) -> bool {
    matches!(e, ServerEvent::ParameterChanged { var_id, .. } if *var_id == var)
}

/// The skill events in order, as compact labels: `D<id>`, `L[ids]`, `P<points>`.
fn skill_trace(events: &[ServerEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            ServerEvent::SkillDelete { id } => Some(format!("D{id}")),
            ServerEvent::SkillList { skills } => Some(format!(
                "L{:?}",
                skills.iter().map(|s| (s.id, s.level)).collect::<Vec<_>>()
            )),
            ServerEvent::ParameterChanged { var_id, value } if *var_id == SP_SKILLPOINT => {
                Some(format!("P{value}"))
            }
            _ => None,
        })
        .collect()
}
