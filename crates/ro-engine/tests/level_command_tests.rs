//! `@blvl`, `@jlvl` and `@job` (spec `specs/stats/spec.md` §2.5-§2.6, ENG-STAT-06..08), with
//! the shipped job data read from `crates/ro-db/data/job/` and hand-written items. The orders
//! are the captured ones (catalog `packets/status.md` "Captured stat-up, level and job
//! sequences", "Capture 2", "Capture 3").
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::{Arc, OnceLock};

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterInventory, CharacterSpawn, CharacterStatus,
    ClientIntent, InventoryItem, ItemId, MapPosition, ScopedServerEvent, ServerEvent, SessionId,
    SessionIntent, Sex,
};
use ro_db::{ItemData, ItemDb, ItemType, JobDb, MapGrid, WeaponType};
use ro_engine::{MapInstance, Registries};

/// The GM who runs the commands.
const A: u32 = 1;
/// An observer next to A.
const B: u32 = 2;

const KNIFE: u32 = 1201;
/// A dagger only a Novice may use.
const NOVICE_KNIFE: u32 = 13040;
const COTTON_SHIRT: u32 = 2301;
/// Beginner Mark: head top, View 1348, `EquipLevelMax` 99 (capture 3).
const MARK: u32 = 19256;
/// Mana Boots: shoes, `EquipLevelMax` 100 (capture 3).
const BOOTS: u32 = 22012;

const EQP_HAND_R: u32 = 0x02;
const EQP_ARMOR: u32 = 0x10;
const EQP_SHOES: u32 = 0x40;
const EQP_HEAD_TOP: u32 = 0x100;

const NOVICE: u16 = 0;
const SWORDMAN: u16 = 1;
const BARD: u16 = 19;
const DANCER: u16 = 20;
const HYPER_NOVICE: u16 = 4307;

const SP_BASEEXP: u16 = 1;
const SP_JOBEXP: u16 = 2;
const SP_HP: u16 = 5;
const SP_MAXHP: u16 = 6;
const SP_SP: u16 = 7;
const SP_MAXSP: u16 = 8;
const SP_STATUSPOINT: u16 = 9;
const SP_BASELEVEL: u16 = 11;
const SP_SKILLPOINT: u16 = 12;
const SP_STR: u32 = 13;
const SP_NEXTBASEEXP: u16 = 22;
const SP_NEXTJOBEXP: u16 = 23;
const SP_MAXWEIGHT: u16 = 25;
const SP_USTR: u16 = 32;
const SP_JOBLEVEL: u16 = 55;
const SP_TRAITPOINT: u16 = 231;
const SP_UPOW: u16 = 247;

const LOOK_BASE: u8 = 0;
const LOOK_HAIR: u8 = 1;
const LOOK_WEAPON: u8 = 2;
const LOOK_HEAD_BOTTOM: u8 = 3;
const LOOK_HEAD_TOP: u8 = 4;
const LOOK_HEAD_MID: u8 = 5;
const LOOK_CLOTHES_COLOR: u8 = 7;
const LOOK_SHOES: u8 = 9;
const LOOK_ROBE: u8 = 12;
const LOOK_BODY2: u8 = 13;

/// Next exp until exp tables exist (spec §2.5).
const NEXT_BASE_EXP: i64 = 99_999_999;
const NEXT_JOB_EXP: i64 = 999_999_999;

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn gid(n: u32) -> u32 {
    2_000_000 + n
}

fn name(n: u32) -> String {
    format!("Char{n}")
}

fn jobs() -> Arc<JobDb> {
    static JOBS: OnceLock<Arc<JobDb>> = OnceLock::new();
    Arc::clone(JOBS.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ro-db/data/job");
        Arc::new(JobDb::load_dir(&dir).expect("shipped job data loads"))
    }))
}

fn item_data(id: u32, item_type: ItemType, locations: u32) -> ItemData {
    ItemData {
        id: ItemId(id),
        aegis_name: format!("Item_{id}"),
        name: format!("Item {id}"),
        item_type,
        weight: 10,
        locations,
        ..ItemData::default()
    }
}

fn items() -> Arc<ItemDb> {
    let knife = ItemData {
        weapon_type: WeaponType::Dagger,
        attack: 17,
        range: 1,
        weapon_level: 1,
        view: 1,
        ..item_data(KNIFE, ItemType::Weapon, EQP_HAND_R)
    };
    let novice_knife = ItemData {
        jobs: [1, 0, 0],
        ..item_data(NOVICE_KNIFE, ItemType::Weapon, EQP_HAND_R)
    };
    let novice_knife = ItemData {
        weapon_type: WeaponType::Dagger,
        attack: 10,
        range: 1,
        weapon_level: 1,
        view: 1,
        ..novice_knife
    };
    let shirt = ItemData {
        defense: 10,
        ..item_data(COTTON_SHIRT, ItemType::Armor, EQP_ARMOR)
    };
    let mark = ItemData {
        view: 1348,
        equip_level_max: 99,
        ..item_data(MARK, ItemType::Armor, EQP_HEAD_TOP)
    };
    let boots = ItemData {
        equip_level_max: 100,
        ..item_data(BOOTS, ItemType::Armor, EQP_SHOES)
    };
    Arc::new(
        ItemDb::from_items("levels", vec![knife, novice_knife, shirt, mark, boots]).expect("items"),
    )
}

fn worn(id: u32, equip: u32) -> Option<InventoryItem> {
    let mut item = InventoryItem::new(ItemId(id), 1);
    item.equip = equip;
    Some(item)
}

struct World {
    engine: MapInstance,
    now: u64,
}

impl World {
    /// A (GM) as `job`/`base_level` with `status` and `slots`, and B (a plain player) beside.
    fn new(
        job: u16,
        base_level: u16,
        status: CharacterStatus,
        slots: Vec<Option<InventoryItem>>,
    ) -> Self {
        Self::with_sex(job, base_level, status, slots, Sex::Male)
    }

    fn with_sex(
        job: u16,
        base_level: u16,
        status: CharacterStatus,
        slots: Vec<Option<InventoryItem>>,
        sex: Sex,
    ) -> Self {
        let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("dims"));
        let mut engine = MapInstance::new("prontera.gat", grid);
        engine.set_registries(Registries {
            items: items(),
            jobs: jobs(),
            ..Registries::default()
        });
        let mut world = Self { engine, now: 0 };
        world.enter(A, job, base_level, status, slots, sex, 99);
        world.enter(
            B,
            NOVICE,
            1,
            CharacterStatus::default(),
            Vec::new(),
            Sex::Female,
            0,
        );
        world
    }

    #[allow(clippy::too_many_arguments)]
    fn enter(
        &mut self,
        n: u32,
        job: u16,
        base_level: u16,
        status: CharacterStatus,
        slots: Vec<Option<InventoryItem>>,
        sex: Sex,
        group_id: i32,
    ) {
        let spawn = CharacterSpawn {
            account_id: AccountId(gid(n)),
            char_id: char_id(n),
            name: name(n),
            sex,
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
            inventory: CharacterInventory {
                slots,
                unique_item_counter: 0,
            },
            skills: Vec::new(),
            hidden_skills: Vec::new(),
            change_level: [0; 3],
        };
        self.engine.spawn_character(spawn, 0).expect("spawn");
        self.tick(&[(n, ClientIntent::NotifyActorInit)]);
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

    fn progress(&self) -> ro_core::CharacterProgress {
        self.engine
            .progress_snapshot(char_id(A))
            .expect("A is on the map")
    }

    fn saves(&mut self) -> Vec<CharId> {
        let mut out = Vec::new();
        self.engine.drain_save_requests(&mut out);
        out
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

fn p(var_id: u16, value: i32) -> ServerEvent {
    ServerEvent::ParameterChanged { var_id, value }
}

fn long(var_id: u16, value: i64) -> ServerEvent {
    ServerEvent::LongLongParameterChanged { var_id, value }
}

fn effect(effect: u32) -> ServerEvent {
    ServerEvent::SpecialEffect {
        gid: gid(A),
        effect,
    }
}

fn look(look_type: u8, value: u32, value2: u32) -> ServerEvent {
    ServerEvent::SpriteChanged {
        gid: gid(A),
        look_type,
        value,
        value2,
    }
}

fn is_message(event: &ServerEvent) -> bool {
    matches!(event, ServerEvent::SelfChat { .. })
}

fn message_text(event: &ServerEvent) -> String {
    match event {
        ServerEvent::SelfChat { text } => String::from_utf8_lossy(text).into_owned(),
        other => panic!("not a message: {other:?}"),
    }
}

fn index_of(events: &[ServerEvent], wanted: &ServerEvent) -> usize {
    events
        .iter()
        .position(|e| e == wanted)
        .unwrap_or_else(|| panic!("{wanted:?} not in {events:?}"))
}

fn base_tail(status_point: u32, level: u16) -> Vec<ServerEvent> {
    vec![
        p(SP_STATUSPOINT, status_point as i32),
        p(SP_TRAITPOINT, 0),
        p(SP_BASELEVEL, level.into()),
        long(SP_BASEEXP, 0),
        long(SP_NEXTBASEEXP, NEXT_BASE_EXP),
    ]
}

fn full(status: CharacterStatus) -> CharacterStatus {
    CharacterStatus {
        hp: 1_000_000,
        sp: 1_000_000,
        ..status
    }
}

/// ENG-STAT-06: `@blvl 10` on a level-1 Novice: the diff, the heal to the new max, `0x019b`
/// effect 0 (the observer gets only this), the message, then the tail (capture 1 lines
/// 401-423).
#[test]
fn eng_stat_06_base_level_up_order() {
    let mut w = World::new(NOVICE, 1, full(CharacterStatus::default()), Vec::new());
    let (a, b) = w.command("blvl 10");
    let points = jobs().status_points(11);
    assert_eq!(a[a.len() - 5..], base_tail(points, 11)[..], "{a:?}");
    let max_hp = index_of(&a, &p(SP_MAXHP, 90));
    index_of(&a, &p(SP_MAXSP, 21));
    let heal = index_of(&a, &p(SP_HP, 90));
    assert!(max_hp < heal, "diff before the heal: {a:?}");
    assert_eq!(
        a[heal..a.len() - 5]
            .iter()
            .filter(|e| !is_message(e))
            .cloned()
            .collect::<Vec<_>>(),
        vec![p(SP_HP, 90), p(SP_SP, 21), effect(0)],
        "{a:?}"
    );
    assert!(is_message(&a[a.len() - 6]), "the message before the tail");
    assert_eq!(b, vec![effect(0)], "the observer sees the effect only");
    let progress = w.progress();
    assert_eq!(progress.base_level, 11);
    assert_eq!(u32::from(progress.status.status_point), points);
    assert_eq!((progress.status.hp, progress.status.sp), (90, 21));
}

/// ENG-STAT-06: an `elvmax` item comes off after the tail (capture 3): head top `0x099a` then
/// the look; shoes the look then `0x099a`. The observer gets the effect and the look.
#[test]
fn eng_stat_06_elvmax_unequip_comes_last() {
    let slots = vec![worn(MARK, EQP_HEAD_TOP), worn(BOOTS, EQP_SHOES)];
    let mut w = World::new(HYPER_NOVICE, 99, full(CharacterStatus::default()), slots);
    let (a, b) = w.command("blvl 1");
    assert_eq!(
        a[a.len() - 2..],
        [
            ServerEvent::UnequipAck {
                index: 2,
                location: EQP_HEAD_TOP,
                failed: false
            },
            look(LOOK_HEAD_TOP, 0, 0),
        ],
        "{a:?}"
    );
    assert_eq!(a[a.len() - 3], long(SP_NEXTBASEEXP, NEXT_BASE_EXP));
    assert_eq!(b, vec![effect(0), look(LOOK_HEAD_TOP, 0, 0)]);

    let (a, b) = w.command("blvl 1");
    assert_eq!(
        a[a.len() - 2..],
        [
            look(LOOK_SHOES, 0, 0),
            ServerEvent::UnequipAck {
                index: 3,
                location: EQP_SHOES,
                failed: false
            },
        ],
        "{a:?}"
    );
    assert_eq!(b, vec![effect(0), look(LOOK_SHOES, 0, 0)]);
    assert_eq!(w.progress().base_level, 101);
    let (a, _) = w.command("blvl 1");
    assert!(
        !a.iter()
            .any(|e| matches!(e, ServerEvent::UnequipAck { .. })),
        "nothing left to take off: {a:?}"
    );
}

/// ENG-STAT-06: `@blvl -5` with enough unspent points: the message, the clamp pushes, the diff,
/// the tail; no heal, no effect (capture 1 lines 679-703).
#[test]
fn eng_stat_06_base_level_down_order() {
    let status = CharacterStatus {
        status_point: u16::try_from(jobs().status_points(11)).expect("small"),
        ..full(CharacterStatus::default())
    };
    let mut w = World::new(NOVICE, 11, status, Vec::new());
    let (a, b) = w.command("blvl -5");
    assert!(is_message(&a[0]), "{a:?}");
    let max_hp = jobs().get(NOVICE).expect("novice").base_hp(6);
    let hp = i32::try_from(max_hp).expect("small") * 101 / 100;
    assert_eq!(a[1], p(SP_HP, hp), "clamp push first: {a:?}");
    let remove = jobs().status_points(11) - jobs().status_points(6);
    let tail = base_tail(jobs().status_points(11) - remove, 6);
    assert_eq!(a[a.len() - 5..], tail[..], "{a:?}");
    assert!(!a.contains(&effect(0)));
    assert!(b.is_empty(), "nothing for the observer: {b:?}");
}

/// ENG-STAT-06: `@blvl -n` with too few unspent points resets the stats first: the costs, the
/// points (table at the old level), then the message; the `0x0141`s come from the diff.
#[test]
fn eng_stat_06_base_level_down_resets_stats() {
    let status = CharacterStatus {
        str: 10,
        status_point: 0,
        ..full(CharacterStatus::default())
    };
    let mut w = World::new(NOVICE, 11, status, Vec::new());
    let (a, _) = w.command("blvl -5");
    let old_points = jobs().status_points(11);
    let mut reset: Vec<ServerEvent> = (0..6)
        .map(|i| ServerEvent::StatusCost {
            var_id: SP_USTR + i,
            value: 2,
        })
        .collect();
    reset.extend((0..6).map(|i| ServerEvent::StatusCost {
        var_id: SP_UPOW + i,
        value: 0,
    }));
    reset.push(p(SP_STATUSPOINT, old_points as i32));
    reset.push(p(SP_TRAITPOINT, 0));
    assert_eq!(a[..reset.len()], reset[..], "{a:?}");
    assert!(is_message(&a[reset.len()]), "{a:?}");
    assert!(
        a.iter().any(|e| matches!(
            e,
            ServerEvent::CoupleStatus {
                var_id: SP_STR,
                base: 1,
                ..
            }
        )),
        "STR's new base from the diff: {a:?}"
    );
    let left = old_points - (old_points - jobs().status_points(6));
    let progress = w.progress();
    assert_eq!((progress.status.str, progress.base_level), (1, 6));
    assert_eq!(u32::from(progress.status.status_point), left);
}

/// Spec §2.5: a bad argument gets the usage line; the level limits get their own line; nothing
/// changes.
#[test]
fn eng_stat_06_refusals() {
    let mut w = World::new(NOVICE, 1, CharacterStatus::default(), Vec::new());
    for line in ["blvl", "blvl abc", "blvl 0", "jlvl", "jlvl x"] {
        let (a, _) = w.command(line);
        assert_eq!(a.len(), 1, "{line}: {a:?}");
        assert!(message_text(&a[0]).starts_with("Usage"), "{line}: {a:?}");
    }
    for line in ["blvl -1", "jlvl -3"] {
        let (a, _) = w.command(line);
        assert_eq!(a.len(), 1, "{line}: {a:?}");
        assert!(!message_text(&a[0]).starts_with("Usage"), "{line}");
    }
    let (a, _) = w.command("blevel 500");
    assert_eq!(
        a[a.len() - 5..],
        base_tail(jobs().status_points(99), 99)[..]
    );
    let (a, _) = w.command("lvup 1");
    assert_eq!(a.len(), 1, "already at the Novice max: {a:?}");
    assert_eq!(w.progress().base_level, 99);
}

/// ENG-STAT-07: `@jlvl` up: effect 1 (the observer gets only this), the message, the job-level
/// pushes, `SP_SKILLPOINT`, then the diff (capture 1 lines 428-450; capture 3 line 931).
#[test]
fn eng_stat_07_job_level_up_and_down() {
    let mut w = World::new(NOVICE, 1, full(CharacterStatus::default()), Vec::new());
    let (a, b) = w.command("jlvl 9");
    assert_eq!(a[0], effect(1), "{a:?}");
    assert!(is_message(&a[1]));
    assert_eq!(
        a[2..6],
        [
            p(SP_JOBLEVEL, 10),
            long(SP_JOBEXP, 0),
            long(SP_NEXTJOBEXP, NEXT_JOB_EXP),
            p(SP_SKILLPOINT, 9),
        ],
        "{a:?}"
    );
    assert!(
        matches!(
            a[6],
            ServerEvent::CoupleStatus {
                var_id: SP_STR,
                base: 1,
                bonus: 1
            }
        ),
        "the diff follows: {a:?}"
    );
    assert_eq!(b, vec![effect(1)]);
    assert_eq!(w.progress().status.job_level, 10);

    let (a, b) = w.command("joblevel -2");
    assert!(is_message(&a[0]));
    assert_eq!(
        a[1..5],
        [
            p(SP_JOBLEVEL, 8),
            long(SP_JOBEXP, 0),
            long(SP_NEXTJOBEXP, NEXT_JOB_EXP),
            p(SP_SKILLPOINT, 7),
        ],
        "{a:?}"
    );
    assert!(!a.contains(&effect(1)));
    assert!(b.is_empty(), "{b:?}");
}

/// ENG-STAT-08: `@job swordman` wearing a Novice-only knife: the costs, the job-level pushes,
/// the knife's look and `0x099a`, the class looks (also to the observer), `0x010f`, then
/// `SP_MAXWEIGHT` and the diff, the equip looks, the message; a save is requested (capture 1
/// lines 1714-1764).
#[test]
fn eng_stat_08_job_change_order() {
    let slots = vec![
        worn(NOVICE_KNIFE, EQP_HAND_R),
        worn(COTTON_SHIRT, EQP_ARMOR),
    ];
    let mut w = World::new(NOVICE, 10, full(CharacterStatus::default()), slots);
    let (a, b) = w.command("job swordman");
    let costs: Vec<ServerEvent> = (0..6)
        .map(|i| ServerEvent::StatusCost {
            var_id: SP_USTR + i,
            value: 2,
        })
        .collect();
    assert_eq!(a[..6], costs[..], "{a:?}");
    assert_eq!(
        a[6..9],
        [
            p(SP_JOBLEVEL, 1),
            long(SP_JOBEXP, 0),
            long(SP_NEXTJOBEXP, NEXT_JOB_EXP),
        ]
    );
    let class_looks = vec![
        look(LOOK_BASE, SWORDMAN.into(), 0),
        look(LOOK_HAIR, 3, 0),
        look(LOOK_CLOTHES_COLOR, 0, 0),
        look(LOOK_BODY2, SWORDMAN.into(), 0),
    ];
    assert_eq!(
        a[9..15],
        [
            look(LOOK_WEAPON, 0, 0),
            ServerEvent::UnequipAck {
                index: 2,
                location: EQP_HAND_R,
                failed: false
            },
            class_looks[0].clone(),
            class_looks[1].clone(),
            class_looks[2].clone(),
            class_looks[3].clone(),
        ],
        "{a:?}"
    );
    assert!(matches!(a[15], ServerEvent::SkillList { .. }), "{a:?}");
    assert_eq!(a[16], p(SP_MAXWEIGHT, 28_000 + 300), "{a:?}");
    let equip_looks = vec![
        look(LOOK_WEAPON, 0, 0),
        look(LOOK_SHOES, 0, 0),
        look(LOOK_HEAD_BOTTOM, 0, 0),
        look(LOOK_HEAD_TOP, 0, 0),
        look(LOOK_HEAD_MID, 0, 0),
        look(LOOK_ROBE, 0, 0),
    ];
    let n = a.len();
    assert_eq!(a[n - 7..n - 1], equip_looks[..], "{a:?}");
    assert!(is_message(&a[n - 1]));
    let mut seen = vec![look(LOOK_WEAPON, 0, 0)];
    seen.extend(class_looks);
    seen.extend(equip_looks);
    assert_eq!(b, seen, "the observer gets the looks only");
    assert_eq!(w.saves(), vec![char_id(A)]);
    let progress = w.progress();
    assert_eq!(
        (
            progress.job_class,
            progress.status.job_level,
            progress.base_level
        ),
        (SWORDMAN, 1, 10)
    );
}

/// ENG-STAT-08: a base level above the new job's max is capped with a stat reset (its pushes,
/// for the new class: trait costs 0) and the base tail, sent before the costs (traits spec
/// §2.7; leaving Hyper Novice then resets again, ENG-TRAIT-06).
#[test]
fn eng_stat_08_job_change_caps_the_base_level() {
    let status = CharacterStatus {
        str: 50,
        status_point: 0,
        ..full(CharacterStatus::default())
    };
    let mut w = World::new(HYPER_NOVICE, 150, status, Vec::new());
    let (a, _) = w.command("job 1");
    let points = jobs().status_points(99);
    let mut reset: Vec<ServerEvent> = (0..6)
        .map(|i| ServerEvent::StatusCost {
            var_id: SP_USTR + i,
            value: 2,
        })
        .collect();
    reset.extend((0..6).map(|i| ServerEvent::StatusCost {
        var_id: SP_UPOW + i,
        value: 0,
    }));
    reset.push(p(SP_STATUSPOINT, points as i32));
    reset.push(p(SP_TRAITPOINT, 0));
    assert_eq!(a[..reset.len()], reset[..], "{a:?}");
    assert_eq!(
        a[reset.len()..reset.len() + 5],
        base_tail(points, 99)[..],
        "{a:?}"
    );
    let progress = w.progress();
    assert_eq!((progress.base_level, progress.status.str), (99, 1));
    assert_eq!(
        u32::from(progress.status.status_point),
        jobs().status_points(99)
    );
}

/// Spec §2.6: the current job or an unknown name gets our message and changes nothing;
/// Bard and Dancer follow the character's sex.
#[test]
fn eng_stat_08_job_refusals_and_sex() {
    let mut w = World::new(NOVICE, 1, CharacterStatus::default(), Vec::new());
    for line in ["job 0", "job novice", "job not_a_job", "job 99999", "job"] {
        let (a, b) = w.command(line);
        assert!(a.iter().all(is_message), "{line}: {a:?}");
        assert!(!a.is_empty(), "{line}");
        assert!(b.is_empty(), "{line}: {b:?}");
    }
    assert!(w.saves().is_empty());
    assert_eq!(w.progress().job_class, NOVICE);

    w.command("job bard");
    assert_eq!(w.progress().job_class, BARD, "a male stays a Bard");
    let mut w = World::with_sex(
        NOVICE,
        1,
        CharacterStatus::default(),
        Vec::new(),
        Sex::Female,
    );
    w.command("jobchange bard");
    assert_eq!(w.progress().job_class, DANCER);
    let (a, _) = w.command("job 20");
    assert!(a.iter().all(is_message), "already a Dancer: {a:?}");
}

/// Spec §2.6 step 10: `@job`'s equip looks carry each slot's view when they are sent, so an
/// equip later in the same tick isn't undone on screen (Finish review m-1).
#[test]
fn eng_stat_08_equip_looks_are_read_when_sent() {
    let slots = vec![Some(InventoryItem::new(ItemId(KNIFE), 1))];
    let mut w = World::new(NOVICE, 10, CharacterStatus::default(), slots);
    let text = format!("{} : @job swordman", name(A)).into_bytes();
    let out = w.tick(&[
        (A, ClientIntent::Chat { text }),
        (
            A,
            ClientIntent::EquipItem {
                index: 2,
                position: EQP_HAND_R,
            },
        ),
    ]);
    for n in [A, B] {
        let seen = seen_by(&out, n);
        let last_weapon = seen
            .iter()
            .rev()
            .find(|e| matches!(e, ServerEvent::SpriteChanged { look_type, .. } if *look_type == LOOK_WEAPON));
        assert_eq!(
            last_weapon,
            Some(&look(LOOK_WEAPON, KNIFE, 0)),
            "{n}: {seen:?}"
        );
    }
}

/// Spec §2.6: `@job` keeps usable gear on, and sends no heal.
#[test]
fn eng_stat_08_usable_gear_stays() {
    let slots = vec![worn(KNIFE, EQP_HAND_R), worn(MARK, EQP_HEAD_TOP)];
    let mut w = World::new(NOVICE, 10, CharacterStatus::default(), slots);
    let (a, _) = w.command("job swordman");
    assert!(
        !a.iter()
            .any(|e| matches!(e, ServerEvent::UnequipAck { .. })),
        "{a:?}"
    );
    let n = a.len();
    assert_eq!(a[n - 7], look(LOOK_WEAPON, KNIFE, 0), "{a:?}");
    assert_eq!(a[n - 4], look(LOOK_HEAD_TOP, 1348, 0), "{a:?}");
    assert!(
        !a.iter()
            .any(|e| matches!(e, ServerEvent::ParameterChanged { var_id: SP_HP, .. })),
        "no heal: {a:?}"
    );
}
