//! Skills, learn only (spec `specs/skills/spec.md` §2, ENG-SKILL-03..11), with the shipped job
//! and skill data read from `crates/ro-db/data/` (and hand-written trees where a gate is needed).
//! The orders are the captured ones (catalog `mechanics/skills.md` §10;
//! `reference/catalog/captures/rathena_skills_sequence.txt`, file lines).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::{Arc, OnceLock};

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterInventory, CharacterSpawn, CharacterStatus,
    ClientIntent, LearnedSkill, MapPosition, ScopedServerEvent, ServerEvent, SessionId,
    SessionIntent, Sex, SkillFlag, SkillInfo,
};
use ro_db::{JobDb, MapGrid, SkillDb, SkillTreeDb};
use ro_engine::{MapInstance, Registries};

/// The player under test (a GM) and an observer beside it.
const A: u32 = 1;
const B: u32 = 2;

const NOVICE: u16 = 0;
const SWORDMAN: u16 = 1;
const MAGE: u16 = 2;
const KNIGHT: u16 = 7;

const NV_BASIC: u16 = 1;
const SM_SWORD: u16 = 2;
const SM_TWOHAND: u16 = 3;
const SM_BASH: u16 = 5;
const SM_PROVOKE: u16 = 6;
const SM_MAGNUM: u16 = 7;
const SM_ENDURE: u16 = 8;
const KN_SPEARMASTERY: u16 = 55;

const SP_SKILLPOINT: u16 = 12;
const SP_JOBLEVEL: u16 = 55;

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
            "Header: {Type: SKILL_DATA, Version: 1}
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

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn gid(n: u32) -> u32 {
    2_000_000 + n
}

fn name(n: u32) -> String {
    format!("Skiller{n}")
}

fn learned(skills: &[(u16, u8)]) -> Vec<LearnedSkill> {
    skills
        .iter()
        .map(|&(id, level)| LearnedSkill {
            id,
            level,
            flag: SkillFlag::Permanent,
        })
        .collect()
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

fn seen_by(events: &[ScopedServerEvent], n: u32) -> Vec<ServerEvent> {
    events
        .iter()
        .filter(|e| match &e.scope {
            BroadcastScope::SelfOnly(c) => *c == char_id(n),
            BroadcastScope::Characters(chars) => chars.contains(&char_id(n)),
            _ => false,
        })
        .map(|e| e.event.clone())
        .collect()
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

/// ENG-SKILL-03: the captured `sp`/`range2` values (catalog §10.2) with the shipped data.
#[test]
fn eng_skill_03_captured_list_values() {
    let world = World::new(
        SWORDMAN,
        (1, 45),
        &[
            (NV_BASIC, 9),
            (SM_SWORD, 1),
            (SM_BASH, 5),
            (SM_PROVOKE, 5),
            (SM_MAGNUM, 1),
            (SM_ENDURE, 1),
        ],
        0,
    );
    let list = only_list(&world.login);
    assert_eq!(
        list.iter().map(|s| s.id).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5, 6, 7, 8],
        "ascending ids, quest and wedding skills hidden"
    );
    let values = |id| {
        let e = entry(&list, id);
        (e.inf, e.level, e.sp, e.range, e.upgradable)
    };
    assert_eq!(values(SM_BASH), (1, 5, 8, 1, true));
    assert_eq!(values(SM_PROVOKE), (1, 5, 8, 9, true));
    assert_eq!(values(SM_MAGNUM), (4, 1, 30, 0, true));
    assert_eq!(values(SM_ENDURE), (4, 1, 10, 0, true));
    assert_eq!(values(SM_SWORD), (0, 1, 0, 0, true));
    // Level 0: the real inf, sp and range 0.
    assert_eq!(values(SM_TWOHAND), (0, 0, 0, 0, true));
    assert_eq!(values(NV_BASIC), (0, 9, 0, 0, false));
    assert_eq!(&*entry(&list, SM_BASH).name, "SM_BASH");

    let world = World::new(
        SWORDMAN,
        (1, 45),
        &[(NV_BASIC, 9), (SM_BASH, 10), (SM_PROVOKE, 10)],
        0,
    );
    let list = only_list(&world.login);
    assert_eq!(
        (entry(&list, SM_BASH).sp, entry(&list, SM_BASH).range),
        (15, 1)
    );
    assert_eq!(entry(&list, SM_PROVOKE).sp, 13);
    assert!(!entry(&list, SM_BASH).upgradable, "the tree max");
}

/// ENG-SKILL-04: a fresh Novice's login list is NV_BASIC at level 0 (41 bytes on the wire);
/// a Swordman's is the real one.
#[test]
fn eng_skill_04_login_list() {
    let world = World::new(NOVICE, (1, 1), &[], 0);
    let list = only_list(&world.login);
    assert_eq!(
        list,
        [SkillInfo {
            id: NV_BASIC,
            inf: 0,
            level: 0,
            sp: 0,
            range: 0,
            name: Arc::from("NV_BASIC"),
            upgradable: true,
        }]
    );
    let world = World::new(
        SWORDMAN,
        (1, 10),
        &[(NV_BASIC, 9), (SM_SWORD, 1), (SM_BASH, 5)],
        0,
    );
    assert_eq!(
        levels(&only_list(&world.login)),
        [(1, 9), (2, 1), (3, 0), (4, 0), (5, 5), (6, 0), (7, 0)]
    );
    // NV_BASIC below 9: only the Novice tree (captured 1341).
    let world = World::new(SWORDMAN, (1, 10), &[(NV_BASIC, 3)], 6);
    assert_eq!(levels(&only_list(&world.login)), [(1, 3)]);
}

/// ENG-SKILL-10: `@job` sends one list at the step-8 position, no `SP_SKILLPOINT`, no
/// `0x0441`; records the class-change level; hidden levels stay in memory.
#[test]
fn eng_skill_10_job_change() {
    // Novice (NV_BASIC 9) -> Swordman (captured 536).
    let mut world = World::new(NOVICE, (1, 10), &[(NV_BASIC, 9)], 0);
    let (a, b) = world.command(&format!("job {SWORDMAN}"));
    let list = only_list(&a);
    assert_eq!(levels(&list), [(1, 9), (2, 0), (4, 0), (5, 0), (6, 0)]);
    // After the looks (step 7), before the recalc's diff.
    let looks = position(&a, |e| matches!(e, ServerEvent::SpriteChanged { .. }));
    let at = position(&a, |e| matches!(e, ServerEvent::SkillList { .. }));
    assert!(looks < at, "{a:?}");
    assert!(!a
        .iter()
        .any(|e| matches!(e, ServerEvent::SkillDelete { .. })));
    assert!(!a.iter().any(
        |e| matches!(e, ServerEvent::ParameterChanged { var_id, .. } if *var_id == SP_SKILLPOINT)
    ));
    assert!(!b.iter().any(is_skill_event), "the observer gets nothing");
    assert_eq!(world.change_level(), [0; 3], "a 1st class sets nothing");

    // Swordman at job 10 -> Knight: the 2nd-class level is the job level before the change.
    let mut world = World::new(
        SWORDMAN,
        (1, 10),
        &[(NV_BASIC, 9), (SM_SWORD, 1), (SM_BASH, 5)],
        3,
    );
    let (a, _) = world.command(&format!("job {KNIGHT}"));
    assert_eq!(world.change_level(), [10, 0, 0]);
    // 6 Swordman points < 9: still the Swordman tree's skills.
    let list = only_list(&a);
    assert!(list.iter().all(|s| s.id < KN_SPEARMASTERY), "{list:?}");
    assert_eq!(entry(&list, SM_BASH).level, 5);

    // Knight -> Mage: the Mage list; Sword and Bash hidden.
    let (a, _) = world.command(&format!("job {MAGE}"));
    let list = only_list(&a);
    assert!(list.iter().all(|s| s.id == NV_BASIC || s.id > SM_ENDURE));
    assert!(!a
        .iter()
        .any(|e| matches!(e, ServerEvent::SkillDelete { .. })));
    // The next tick's pass finds nothing to send.
    let out = world.tick(&[]);
    assert!(!seen_by(&out, A).iter().any(is_skill_event));
    // Back to Knight before a save: the levels are still there. The change from Mage (job
    // level 1) sets the 2nd-class level to 1, so the Knight tree shows at once.
    let (a, _) = world.command(&format!("job {KNIGHT}"));
    assert_eq!(world.change_level(), [1, 0, 0]);
    let list = only_list(&a);
    assert_eq!(entry(&list, SM_BASH).level, 5);
    assert_eq!(entry(&list, SM_SWORD).level, 1);
    assert!(list.iter().any(|s| s.id == KN_SPEARMASTERY));
}

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

const SP_STATUSPOINT: u16 = 9;
const SP_TRAITPOINT: u16 = 231;

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

/// ENG-SKILL-07: `@jlvl -5` with 0 free points resets the skills silently; the tick's
/// re-check sends the deletes and the list before the diff (captured 650-663).
#[test]
fn eng_skill_07_job_level_down_resets() {
    let mut world = World::new(
        SWORDMAN,
        (1, 10),
        &[(NV_BASIC, 9), (SM_SWORD, 1), (SM_BASH, 5), (SM_PROVOKE, 3)],
        0,
    );
    let (a, b) = world.command("jlvl -5");
    assert_eq!(
        skill_trace(&a),
        [
            "P4",
            "D3",
            "D7",
            "L[(1, 9), (2, 0), (4, 0), (5, 0), (6, 0)]"
        ]
    );
    let message = position(&a, |e| matches!(e, ServerEvent::SelfChat { .. }));
    let job_level = position(&a, |e| is_var(e, SP_JOBLEVEL));
    let points = position(&a, |e| is_var(e, SP_SKILLPOINT));
    let delete = position(&a, |e| matches!(e, ServerEvent::SkillDelete { .. }));
    assert!(
        message < job_level && job_level < points && points < delete,
        "{a:?}"
    );
    // The stat diff (job bonuses lost) comes after the list.
    let list = position(&a, |e| matches!(e, ServerEvent::SkillList { .. }));
    let diff = position(&a, |e| matches!(e, ServerEvent::CoupleStatus { .. }));
    assert!(list < diff, "{a:?}");
    assert!(!b.iter().any(is_skill_event));

    // Enough free points: no reset, no list.
    let mut world = World::new(SWORDMAN, (1, 10), &[(NV_BASIC, 9), (SM_BASH, 5)], 5);
    let (a, _) = world.command("jlvl -5");
    assert_eq!(skill_trace(&a), ["P0"]);
}

/// ENG-SKILL-08: `@skreset` and `@reset` (captured 766-773, 859-892).
#[test]
fn eng_skill_08_resets() {
    let spent = [
        (NV_BASIC, 9),
        (SM_SWORD, 1),
        (SM_TWOHAND, 1),
        (SM_BASH, 5),
        (SM_MAGNUM, 1),
        (SM_PROVOKE, 5),
        (SM_ENDURE, 1),
    ];
    let mut world = World::new(SWORDMAN, (1, 45), &spent, 30);
    let (a, b) = world.command("skreset");
    assert_eq!(
        skill_trace(&a),
        [
            "P44",
            "L[(1, 9), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0), (8, 0)]",
            "D3",
            "D7",
            "D8",
            "L[(1, 9), (2, 0), (4, 0), (5, 0), (6, 0)]"
        ]
    );
    let list = a
        .iter()
        .rposition(|e| matches!(e, ServerEvent::SkillList { .. }))
        .expect("list");
    let message = position(&a, |e| matches!(e, ServerEvent::SelfChat { .. }));
    assert!(list < message, "the message last: {a:?}");
    assert!(!b.iter().any(is_skill_event), "the observer gets nothing");
    // Nothing left to refund: nothing at all but the message.
    let (a, _) = world.command("resetskill");
    assert!(skill_trace(&a).is_empty(), "{a:?}");

    // As a Novice, NV_BASIC is reset and refunded.
    let mut world = World::new(NOVICE, (1, 10), &[(NV_BASIC, 5)], 4);
    let (a, _) = world.command("skreset");
    assert_eq!(skill_trace(&a), ["P9", "L[(1, 0)]"]);

    // @reset: the stat reset's pushes, then the skill reset's, deletes, list, diff, message.
    let mut world = World::new(SWORDMAN, (1, 45), &spent, 25);
    let (a, b) = world.command("reset");
    assert_eq!(
        skill_trace(&a),
        [
            "P39",
            "L[(1, 9), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0), (8, 0)]",
            "D3",
            "D7",
            "D8",
            "L[(1, 9), (2, 0), (4, 0), (5, 0), (6, 0)]"
        ]
    );
    let status_point = position(&a, |e| is_var(e, SP_STATUSPOINT));
    let trait_point = position(&a, |e| is_var(e, SP_TRAITPOINT));
    let points = position(&a, |e| is_var(e, SP_SKILLPOINT));
    let message = position(&a, |e| matches!(e, ServerEvent::SelfChat { .. }));
    let last_list = a
        .iter()
        .rposition(|e| matches!(e, ServerEvent::SkillList { .. }))
        .expect("list");
    assert!(status_point < trait_point && trait_point < points, "{a:?}");
    assert!(last_list < message, "{a:?}");
    assert!(!b.iter().any(is_skill_event));
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

/// The skills revision (what autosave compares; spec §2.9) moves with a skill-up, a reset and
/// a job change that hides learned levels; not with a refused request.
#[test]
fn skills_revision_tracks_saved_rows() {
    let mut world = World::new(SWORDMAN, (1, 10), &[(NV_BASIC, 9), (SM_BASH, 1)], 2);
    let revision = |world: &World| world.engine.skills_revision(char_id(A)).expect("A");
    let start = revision(&world);
    world.upgrade(&[60_000]);
    assert_eq!(revision(&world), start, "refused");
    world.upgrade(&[SM_BASH]);
    let after_up = revision(&world);
    assert_ne!(after_up, start);
    let snapshot = world.engine.play_snapshot(char_id(A)).expect("A");
    assert_eq!(snapshot.skills, learned(&[(NV_BASIC, 9), (SM_BASH, 2)]));
    world.command(&format!("job {MAGE}"));
    let after_job = revision(&world);
    assert_ne!(after_job, after_up, "Bash hidden");
    let snapshot = world.engine.play_snapshot(char_id(A)).expect("A");
    assert_eq!(
        snapshot.skills,
        learned(&[(NV_BASIC, 9)]),
        "hidden levels aren't saved"
    );
    // The reset refunds the hidden Bash too (rAthena resets every slot with a level): 1 + 2.
    let (a, _) = world.command("skreset");
    assert_ne!(revision(&world), after_job);
    assert_eq!(skill_trace(&a)[0], "P3");
}

const BARD: u16 = 19;
const ASSASSIN: u16 = 12;
const SP_MAXWEIGHT: u16 = 25;

/// `@job` (review fix): the list at step 8 is rAthena's first pass, counted on the old class's
/// visible skills; the tick's re-check counts again on the new set and trims it: deletes after
/// `SP_MAXWEIGHT`, then the final list (captured Dancer -> Assassin, `equip_sequence_4`
/// 2608-2612: {1,48,49,50,52,132,134}, `0x0441` 132, 134, {1,48,49,50,52}).
#[test]
fn job_change_sends_the_second_pass() {
    let archer = [
        (NV_BASIC, 9),
        (43, 10),
        (44, 10),
        (45, 10),
        (46, 10),
        (47, 10),
    ];
    let mut world = World::new(BARD, (90, 50), &archer, 0);
    let (a, _) = world.command(&format!("job {ASSASSIN}"));
    let all = lists(&a);
    assert_eq!(all.len(), 2, "{a:?}");
    let ids = |list: &[SkillInfo]| list.iter().map(|s| s.id).collect::<Vec<_>>();
    assert_eq!(ids(&all[0]), [1, 48, 49, 50, 52, 132, 134]);
    assert_eq!(ids(&all[1]), [1, 48, 49, 50, 52]);
    let deletes: Vec<u16> = a
        .iter()
        .filter_map(|e| match e {
            ServerEvent::SkillDelete { id } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(deletes, [132, 134]);
    let first_delete = position(&a, |e| matches!(e, ServerEvent::SkillDelete { .. }));
    if let Some(weight) = a.iter().position(|e| is_var(e, SP_MAXWEIGHT)) {
        assert!(weight < first_delete, "{a:?}");
    }
    // An Assassin skill is now refused.
    let (a, _) = world.upgrade(&[134]);
    assert!(!a.iter().any(is_skill_event));
}

/// An active skill-up whose full re-check hides slots sends their deletes (review fix): a
/// reset and a skill-up in the same tick, before the pass runs.
#[test]
fn skill_up_after_a_reset_in_the_same_tick_sends_the_deletes() {
    let spent = [
        (NV_BASIC, 9),
        (SM_SWORD, 1),
        (SM_TWOHAND, 1),
        (SM_BASH, 5),
        (SM_MAGNUM, 1),
        (SM_PROVOKE, 5),
        (SM_ENDURE, 1),
    ];
    let mut world = World::new(SWORDMAN, (1, 45), &spent, 30);
    let text = format!("{} : @skreset", name(A)).into_bytes();
    let out = world.tick(&[
        (A, ClientIntent::Chat { text }),
        (A, ClientIntent::UpgradeSkill { id: SM_BASH }),
    ]);
    let a = seen_by(&out, A);
    let trace = skill_trace(&a);
    assert_eq!(
        trace,
        [
            "P44",
            "L[(1, 9), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0), (8, 0)]",
            "P43",
            "D3",
            "D7",
            "D8",
            "L[(1, 9), (2, 0), (4, 0), (5, 1), (6, 0)]"
        ],
        "{a:?}"
    );
}
