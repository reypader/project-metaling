//! The stat model in the engine (spec `specs/stats/spec.md` ENG-STAT-02, -03, -09, -11), with
//! the shipped job and refine data read from `crates/ro-db/data/` (as the map-server does). The
//! item rows are hand-written: only the fields the formulas read, with the captured values
//! (catalog `mechanics/stats.md` "Captured status values").
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::{Arc, OnceLock};

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterInventory, CharacterSpawn, CharacterStatus,
    ClientIntent, InventoryItem, ItemId, MapPosition, ScopedServerEvent, ServerEvent, SessionId,
    SessionIntent, Sex, StatusWindow,
};
use ro_db::{ItemData, ItemDb, ItemType, JobDb, MapGrid, RefineDb, WeaponType};
use ro_engine::{MapInstance, Registries};

const A: u32 = 1;

const KNIFE: u32 = 1201;
const BOW: u32 = 1701;
const GUARD: u32 = 2101;
const COTTON_SHIRT: u32 = 2301;

const EQP_HAND_R: u32 = 0x02;
const EQP_ARMOR: u32 = 0x10;
const EQP_HAND_L: u32 = 0x20;
const EQP_ARMS: u32 = 0x22;

const NOVICE: u16 = 0;
const SWORDMAN: u16 = 1;
const KNIGHT: u16 = 7;
const HYPER_NOVICE: u16 = 4307;

/// `SP_*` ids.
const SP_HP: u16 = 5;
const SP_MAXHP: u16 = 6;
const SP_SP: u16 = 7;
const SP_MAXSP: u16 = 8;
const SP_MAXWEIGHT: u16 = 25;
const SP_ATK1: u16 = 41;
const SP_ATK2: u16 = 42;
const SP_DEF1: u16 = 45;
const SP_DEF2: u16 = 46;
const SP_ASPD: u16 = 53;

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn data_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ro-db/data")
}

fn shipped_jobs() -> Arc<JobDb> {
    static JOBS: OnceLock<Arc<JobDb>> = OnceLock::new();
    Arc::clone(JOBS.get_or_init(|| {
        Arc::new(JobDb::load_dir(&data_dir().join("job")).expect("shipped job data loads"))
    }))
}

fn shipped_refine() -> Arc<RefineDb> {
    static REFINE: OnceLock<Arc<RefineDb>> = OnceLock::new();
    Arc::clone(REFINE.get_or_init(|| {
        Arc::new(RefineDb::load_dir(&data_dir().join("refine")).expect("shipped refine data"))
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
        refineable: true,
        ..ItemData::default()
    }
}

fn items() -> Arc<ItemDb> {
    let knife = ItemData {
        weapon_type: WeaponType::Dagger,
        attack: 17,
        range: 1,
        weapon_level: 1,
        ..item_data(KNIFE, ItemType::Weapon, EQP_HAND_R)
    };
    let bow = ItemData {
        weapon_type: WeaponType::Bow,
        attack: 15,
        range: 5,
        weapon_level: 1,
        ..item_data(BOW, ItemType::Weapon, EQP_ARMS)
    };
    let guard = ItemData {
        defense: 20,
        armor_level: 1,
        ..item_data(GUARD, ItemType::Armor, EQP_HAND_L)
    };
    let shirt = ItemData {
        defense: 10,
        armor_level: 1,
        ..item_data(COTTON_SHIRT, ItemType::Armor, EQP_ARMOR)
    };
    Arc::new(ItemDb::from_items("stats", vec![knife, bow, guard, shirt]).expect("items"))
}

fn instance() -> MapInstance {
    let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("dims"));
    let mut engine = MapInstance::new("prontera.gat", grid);
    engine.set_registries(Registries {
        items: items(),
        jobs: shipped_jobs(),
        refine: shipped_refine(),
        ..Registries::default()
    });
    engine
}

fn worn(id: u32, equip: u32, refine: u8) -> Option<InventoryItem> {
    let mut item = InventoryItem::new(ItemId(id), 1);
    item.equip = equip;
    item.refine = refine;
    Some(item)
}

fn bag(id: u32) -> Option<InventoryItem> {
    Some(InventoryItem::new(ItemId(id), 1))
}

fn stats(allocated: [u16; 6], job_level: u16) -> CharacterStatus {
    let [str, agi, vit, int, dex, luk] = allocated;
    CharacterStatus {
        str,
        agi,
        vit,
        int,
        dex,
        luk,
        job_level,
        // Above any max here: the clamp to the computed max is part of what's checked.
        hp: 100_000,
        sp: 100_000,
        ..CharacterStatus::default()
    }
}

fn spawn(
    job: u16,
    base_level: u16,
    status: CharacterStatus,
    inventory: Vec<Option<InventoryItem>>,
) -> CharacterSpawn {
    CharacterSpawn {
        account_id: AccountId(2_000_000 + A),
        char_id: char_id(A),
        name: "Char".into(),
        sex: Sex::Male,
        job_class: job,
        base_level,
        hair_style: 1,
        hair_color: 1,
        pos: MapPosition {
            x: 50,
            y: 50,
            dir: 0,
        },
        speed: 150,
        status,
        group_id: 0,
        inventory: CharacterInventory {
            slots: inventory,
            unique_item_counter: 0,
        },
        skills: Vec::new(),
        hidden_skills: Vec::new(),
        change_level: [0; 3],
    }
}

fn send(engine: &mut MapInstance, intent: ClientIntent, now: u64) -> Vec<ServerEvent> {
    engine.push_intent(SessionIntent::new(
        SessionId(1),
        Some(AccountId(2_000_000 + A)),
        Some(char_id(A)),
        intent,
    ));
    let mut out = Vec::new();
    engine.step_tick(now, &mut out).expect("tick");
    mine(out)
}

fn mine(events: Vec<ScopedServerEvent>) -> Vec<ServerEvent> {
    events
        .into_iter()
        .filter(|e| match &e.scope {
            BroadcastScope::SelfOnly(c) => *c == char_id(A),
            BroadcastScope::Characters(chars) => chars.contains(&char_id(A)),
            _ => false,
        })
        .map(|e| e.event)
        .collect()
}

/// Spawns and runs the first actor init: (login burst, actor init) as `A` sees them.
fn login(engine: &mut MapInstance, data: CharacterSpawn) -> (Vec<ServerEvent>, Vec<ServerEvent>) {
    let burst = mine(engine.spawn_character(data, 0).expect("spawn"));
    let init = send(engine, ClientIntent::NotifyActorInit, 10);
    (burst, init)
}

fn param_of(events: &[ServerEvent], var_id: u16) -> Option<i32> {
    events.iter().find_map(|e| match e {
        ServerEvent::ParameterChanged { var_id: v, value } if *v == var_id => Some(*value),
        _ => None,
    })
}

fn params(events: &[ServerEvent]) -> Vec<(u16, i32)> {
    events
        .iter()
        .filter_map(|e| match e {
            ServerEvent::ParameterChanged { var_id, value } => Some((*var_id, *value)),
            _ => None,
        })
        .collect()
}

fn window(events: &[ServerEvent]) -> StatusWindow {
    events
        .iter()
        .find_map(|e| match e {
            ServerEvent::StatusWindow { status } => Some(*status),
            _ => None,
        })
        .expect("0x00bd sent")
}

/// The `0x0141` bonuses STR..LUK, in order.
fn couple_bonuses(events: &[ServerEvent]) -> Vec<i32> {
    events
        .iter()
        .filter_map(|e| match e {
            ServerEvent::CoupleStatus { var_id, bonus, .. } if (13..=18).contains(var_id) => {
                Some(*bonus)
            }
            _ => None,
        })
        .take(6)
        .collect()
}

/// The captured window columns (catalog `mechanics/stats.md`): amotion, ATK1/ATK2, MATK2/MATK1,
/// DEF1/DEF2, MDEF1/MDEF2, HIT, FLEE1/FLEE2, CRIT, then MaxHP/MaxSP and MaxWeight from the
/// login push.
fn captured_columns(burst: &[ServerEvent], init: &[ServerEvent]) -> [i32; 16] {
    let w = window(init);
    let p = |var| param_of(burst, var).expect("in the login push");
    [
        w.aspd.into(),
        w.atk.into(),
        w.atk_bonus.into(),
        w.matk.into(),
        w.matk_bonus.into(),
        w.soft_def.into(),
        w.hard_def.into(),
        w.soft_mdef.into(),
        w.hard_mdef.into(),
        w.hit.into(),
        w.flee.into(),
        w.perfect_dodge.into(),
        w.critical.into(),
        p(SP_MAXHP),
        p(SP_MAXSP),
        p(SP_MAXWEIGHT),
    ]
}

fn knife_and_shirt() -> Vec<Option<InventoryItem>> {
    vec![worn(KNIFE, EQP_HAND_R, 0), worn(COTTON_SHIRT, EQP_ARMOR, 0)]
}

/// One captured window: label, job, base level, allocated STR..LUK, job level, inventory and
/// the `captured_columns` values.
type Case = (
    &'static str,
    u16,
    u16,
    [u16; 6],
    u16,
    Vec<Option<InventoryItem>>,
    [i32; 16],
);

/// ENG-STAT-11: the shipped job and refine data reproduce the captured status windows end to
/// end (capture 1 lines 294-321, 402-416, 435-450, 1729-1747, 4576-4602).
#[test]
fn eng_stat_11_shipped_data_reproduces_the_captured_windows() {
    let cases: [Case; 5] = [
        (
            "Novice B1 J1",
            NOVICE,
            1,
            [1; 6],
            1,
            knife_and_shirt(),
            [590, 1, 17, 1, 0, 1, 10, 1, 0, 177, 102, 1, 1, 40, 11, 20300],
        ),
        (
            "Novice B11 J1",
            NOVICE,
            11,
            [1; 6],
            1,
            knife_and_shirt(),
            [590, 4, 17, 3, 0, 6, 10, 4, 0, 187, 112, 1, 1, 90, 21, 20300],
        ),
        (
            "Novice B11 J10",
            NOVICE,
            11,
            [1; 6],
            10,
            knife_and_shirt(),
            [590, 5, 17, 5, 0, 6, 10, 5, 0, 188, 113, 1, 1, 91, 21, 20300],
        ),
        (
            "Swordman B37 J1",
            SWORDMAN,
            37,
            [13, 21, 17, 19, 29, 16],
            1,
            knife_and_shirt(),
            [
                470, 33, 17, 47, 0, 31, 10, 37, 0, 246, 161, 2, 6, 834, 99, 31900,
            ],
        ),
        (
            "Knight B99 J1 (pre-ready ASPD)",
            KNIGHT,
            99,
            [99, 99, 1, 1, 9, 1],
            1,
            vec![worn(COTTON_SHIRT, EQP_ARMOR, 0)],
            [
                270, 125, 0, 26, 0, 70, 10, 27, 0, 283, 298, 1, 2, 8137, 310, 57700,
            ],
        ),
    ];
    for (label, job, base_level, allocated, job_level, inventory, expected) in cases {
        let mut engine = instance();
        let (burst, init) = login(
            &mut engine,
            spawn(job, base_level, stats(allocated, job_level), inventory),
        );
        assert_eq!(captured_columns(&burst, &init), expected, "{label}");
        assert_eq!(
            (param_of(&burst, SP_HP), param_of(&burst, SP_SP)),
            (Some(expected[13]), Some(expected[14])),
            "{label}: HP/SP clamped to the new maxima at spawn"
        );
    }
}

/// ENG-STAT-11 with capture 2 (lines 1381-1399): Hyper Novice B1 J1, all allocated 1, bare: the
/// Super Novice +10 (STR/AGI also +1 from the job bonus), no ×1.25, MaxHP 99, amotion 420.
#[test]
fn eng_stat_11_hyper_novice() {
    let mut engine = instance();
    let (burst, init) = login(
        &mut engine,
        spawn(HYPER_NOVICE, 1, stats([1; 6], 1), vec![]),
    );
    assert_eq!(couple_bonuses(&burst), vec![11, 11, 10, 10, 10, 10]);
    assert_eq!(
        (param_of(&burst, SP_MAXHP), param_of(&burst, SP_MAXSP)),
        (Some(99), Some(11))
    );
    assert_eq!(window(&init).aspd, 420);
}

/// ENG-STAT-09: a bare level-1 Novice logs in with ASPD 440 (DESIGN.md §9.4); the window,
/// the login push and the actor init's `0x0141`s carry the real values, and `0x0141`'s bonus is
/// battle − allocated (job 10: +1 each).
#[test]
fn eng_stat_09_login_carries_real_values() {
    let mut engine = instance();
    let (burst, init) = login(&mut engine, spawn(NOVICE, 1, stats([1; 6], 1), vec![]));
    assert_eq!(param_of(&burst, SP_ASPD), Some(440));
    assert_eq!(window(&init).aspd, 440);
    assert_eq!(
        param_of(&init, SP_ASPD),
        Some(440),
        "the status block's ASPD"
    );

    let mut engine = instance();
    let (burst, init) = login(&mut engine, spawn(NOVICE, 11, stats([1; 6], 10), vec![]));
    assert_eq!(couple_bonuses(&burst), vec![1; 6]);
    assert_eq!(couple_bonuses(&init), vec![1; 6]);
}

/// ENG-STAT-02: a weapon on bare hands sends its acks and look, then one net diff: ATK2 and
/// ASPD (capture 1: 440 ↔ 590). Unequip-all of three items: one diff.
#[test]
fn eng_stat_02_equip_sends_one_net_diff() {
    let mut engine = instance();
    login(
        &mut engine,
        spawn(
            NOVICE,
            1,
            stats([1; 6], 1),
            vec![bag(KNIFE), bag(COTTON_SHIRT), bag(GUARD)],
        ),
    );
    let out = send(
        &mut engine,
        ClientIntent::EquipItem {
            index: 2,
            position: EQP_HAND_R,
        },
        100,
    );
    let ack = out
        .iter()
        .position(|e| matches!(e, ServerEvent::EquipAck { .. }))
        .expect("ack");
    assert_eq!(params(&out[ack..]), vec![(SP_ASPD, 590), (SP_ATK2, 17)]);

    for (index, position) in [(3, EQP_ARMOR), (4, EQP_HAND_L)] {
        send(
            &mut engine,
            ClientIntent::EquipItem { index, position },
            200,
        );
    }
    let out = send(
        &mut engine,
        ClientIntent::UnequipAll { location: 0x83ff },
        300,
    );
    let ack = out
        .iter()
        .position(|e| matches!(e, ServerEvent::UnequipAllAck { .. }))
        .expect("unequip-all ack");
    assert!(
        params(&out[..ack]).is_empty(),
        "no push between the acks: {out:?}"
    );
    let diff = params(&out[ack..]);
    assert_eq!(
        diff.iter().filter(|(var, _)| *var == SP_DEF2).count(),
        1,
        "one diff for three items: {diff:?}"
    );
    assert_eq!(param_of(&out, SP_ASPD), Some(440));
    assert_eq!(param_of(&out, SP_DEF2), Some(0));
}

/// ENG-STAT-03: a shield on and off pushes ASPD; a bow pushes ATK1 (DEX-based status ATK).
#[test]
fn eng_stat_03_shield_and_bow() {
    let mut engine = instance();
    let (burst, _) = login(
        &mut engine,
        spawn(
            NOVICE,
            1,
            stats([1, 1, 1, 1, 9, 1], 1),
            vec![worn(KNIFE, EQP_HAND_R, 0), bag(GUARD), bag(BOW)],
        ),
    );
    let knife = param_of(&burst, SP_ASPD).expect("login ASPD");
    let on = send(
        &mut engine,
        ClientIntent::EquipItem {
            index: 3,
            position: EQP_HAND_L,
        },
        100,
    );
    let slower = param_of(&on, SP_ASPD).expect("ASPD pushed");
    assert!(
        slower > knife,
        "a shield slows the attack: {slower} vs {knife}"
    );
    let off = send(&mut engine, ClientIntent::UnequipItem { index: 3 }, 200);
    assert_eq!(param_of(&off, SP_ASPD), Some(knife));

    let bow = send(
        &mut engine,
        ClientIntent::EquipItem {
            index: 4,
            position: EQP_ARMS,
        },
        300,
    );
    let atk1 = param_of(&bow, SP_ATK1).expect("ATK1 pushed");
    // DEX 9 as the main stat: (9 * 10 + 1 * 10 / 5 + 1 * 10 / 3 + 1 * 10 / 4) / 10 = 9.
    assert_eq!(atk1, 9);
}

/// Refine end to end (spec §2.2; capture 1 "Refine"): Knife +10 ATK2 37, Guard +6 adds 8 hard
/// DEF over its 20 (with the shirt's 10: 38).
#[test]
fn eng_stat_02_refine_from_the_shipped_table() {
    let mut engine = instance();
    let (burst, init) = login(
        &mut engine,
        spawn(
            SWORDMAN,
            48,
            stats([1; 6], 11),
            vec![
                worn(KNIFE, EQP_HAND_R, 10),
                worn(COTTON_SHIRT, EQP_ARMOR, 0),
                worn(GUARD, EQP_HAND_L, 6),
            ],
        ),
    );
    assert_eq!(param_of(&burst, SP_ATK2), Some(37));
    assert_eq!(window(&init).hard_def, 38);
    assert_eq!(param_of(&burst, SP_DEF2), Some(38));
    assert!(param_of(&burst, SP_DEF1).is_some());
}

/// ENG-STAT-09: a cross-map arrival recomputes without a diff push; its actor init sends the
/// `0x0141` ×12 and no derived values.
#[test]
fn eng_stat_09_cross_map_arrival_has_no_diff() {
    let mut engine = instance();
    engine
        .enter_by_warp(
            spawn(NOVICE, 11, stats([1; 6], 10), knife_and_shirt()),
            (60, 60),
            0,
            0,
        )
        .expect("arrives");
    let mut out = Vec::new();
    engine.step_tick(5, &mut out).expect("tick");
    assert!(mine(out).is_empty(), "nothing before the actor init");
    let init = send(&mut engine, ClientIntent::NotifyActorInit, 10);
    let pushed: Vec<u16> = params(&init).into_iter().map(|(var, _)| var).collect();
    assert_eq!(
        pushed,
        vec![24, SP_MAXWEIGHT],
        "only the inventory's weights"
    );
    assert_eq!(param_of(&init, SP_MAXWEIGHT), Some(20300));
    assert_eq!(couple_bonuses(&init), vec![1; 6]);
    let couples = init
        .iter()
        .filter(|e| matches!(e, ServerEvent::CoupleStatus { .. }))
        .count();
    assert_eq!(couples, 12, "STR..LUK and POW..CRT");
}

const SP_STATUSPOINT: u16 = 9;
const SP_STR: u16 = 13;
const SP_DEX: u16 = 17;
const SP_LUK: u16 = 18;
const SP_USTR: u16 = 32;
const SP_FLEE2: u16 = 51;
const SP_CRITICAL: u16 = 52;

fn raise(engine: &mut MapInstance, var: u16, amount: u8, now: u64) -> Vec<ServerEvent> {
    send(engine, ClientIntent::RaiseStat { var, amount }, now)
}

fn ack(var_id: u16, ok: bool, value: u8) -> ServerEvent {
    ServerEvent::StatusChangeAck { var_id, ok, value }
}

/// ENG-STAT-05: a STR raise sends the recalc (`SP_MAXWEIGHT` first, the stat's `0x0141`, then
/// the derived values), then `0x00be`, `SP_STATUSPOINT` and the ack (capture 1 lines 157-162).
#[test]
fn eng_stat_05_stat_up_success_order() {
    let mut engine = instance();
    login(
        &mut engine,
        spawn(NOVICE, 1, stats([1; 6], 1), knife_and_shirt()),
    );
    let out = raise(&mut engine, SP_STR, 1, 100);
    assert_eq!(
        out[..2],
        [
            ServerEvent::ParameterChanged {
                var_id: SP_MAXWEIGHT,
                value: 20600
            },
            ServerEvent::CoupleStatus {
                var_id: SP_STR.into(),
                base: 2,
                bonus: 0
            },
        ],
        "{out:?}"
    );
    assert_eq!(param_of(&out, SP_ATK1), Some(2), "status ATK follows STR");
    assert_eq!(
        out[out.len() - 3..],
        [
            ServerEvent::StatusCost {
                var_id: SP_USTR,
                value: 2
            },
            ServerEvent::ParameterChanged {
                var_id: SP_STATUSPOINT,
                value: 46
            },
            ack(SP_STR, true, 2),
        ],
        "{out:?}"
    );
}

/// ENG-STAT-05: an over-affordable amount raises as far as the points go (capture 1 line 4531:
/// DEX +99 with 17 points stops at 9, leaving 1); a LUK raise re-sends FLEE2 and CRITICAL.
#[test]
fn eng_stat_05_over_affordable_and_luk() {
    let mut engine = instance();
    let mut status = stats([1; 6], 1);
    status.status_point = 17;
    login(&mut engine, spawn(NOVICE, 1, status, knife_and_shirt()));
    let out = raise(&mut engine, SP_DEX, 99, 100);
    assert_eq!(
        out[out.len() - 3..],
        [
            ServerEvent::StatusCost {
                var_id: SP_USTR + 4,
                value: 2
            },
            ServerEvent::ParameterChanged {
                var_id: SP_STATUSPOINT,
                value: 1
            },
            ack(SP_DEX, true, 9),
        ],
        "{out:?}"
    );

    let mut engine = instance();
    login(
        &mut engine,
        spawn(NOVICE, 1, stats([1; 6], 1), knife_and_shirt()),
    );
    let out = raise(&mut engine, SP_LUK, 1, 100);
    let pushed: Vec<u16> = params(&out).into_iter().map(|(var, _)| var).collect();
    assert!(
        pushed.contains(&SP_FLEE2) && pushed.contains(&SP_CRITICAL),
        "{out:?}"
    );
}

/// ENG-STAT-05: a failure sends only `0x00bc` ok 0 at once: no points, at the cap, a bad var,
/// a zero amount (capture 2 line 987).
#[test]
fn eng_stat_05_failures_send_only_the_ack() {
    let mut engine = instance();
    let mut status = stats([99, 1, 1, 1, 1, 1], 1);
    status.status_point = 1;
    login(&mut engine, spawn(NOVICE, 1, status, knife_and_shirt()));
    for (var, amount, why) in [
        (SP_STR, 1, "at the Novice cap 99"),
        (SP_DEX, 1, "1 point, the next costs 2"),
        (19, 1, "not STR..LUK"),
        (12, 1, "not STR..LUK"),
        (SP_LUK, 0, "nothing asked"),
    ] {
        let out = raise(&mut engine, var, amount, 100);
        assert_eq!(out, vec![ack(var, false, 0)], "{why}");
    }
}
