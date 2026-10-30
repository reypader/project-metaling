//! Trait stats and AP in the engine (spec `specs/traits/spec.md` ENG-TRAIT-01..08), with the
//! shipped job data read from `crates/ro-db/data/job/`. The orders are the captured ones
//! (`reference/catalog/captures/rathena_traits_sequence.txt`; file line = proxy # + 10).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::{Arc, OnceLock};

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterInventory, CharacterSpawn, CharacterStatus,
    ClientIntent, MapPosition, ScopedServerEvent, ServerEvent, SessionId, SessionIntent, Sex,
};
use ro_db::{JobDb, MapGrid};
use ro_engine::{MapInstance, Registries};

/// The player (a GM, so it may run the level commands).
const A: u32 = 1;
/// An observer next to A.
const B: u32 = 2;

const DRAGON_KNIGHT: u16 = 4252;
const KNIGHT: u16 = 7;
const NOVICE_HIGH: u16 = 4001;
const SUPER_NOVICE: u16 = 23;
const HYPER_NOVICE: u16 = 4307;
const RUNE_KNIGHT_T: u16 = 4060;
const MEISTER: u16 = 4253;

const SP_BASEEXP: u16 = 1;
const SP_HP: u16 = 5;
const SP_SP: u16 = 7;
const SP_STATUSPOINT: u16 = 9;
const SP_BASELEVEL: u16 = 11;
const SP_NEXTBASEEXP: u16 = 22;
const SP_WEIGHT: u16 = 24;
const SP_USTR: u16 = 32;
const SP_ATK1: u16 = 41;
const SP_HIT: u16 = 49;
const SP_FLEE1: u16 = 50;
const SP_ASPD: u16 = 53;
const SP_JOBLEVEL: u16 = 55;
const SP_POW: u32 = 219;
const SP_CON: u16 = 223;
const SP_PATK: u16 = 225;
const SP_RES: u16 = 227;
const SP_TRAITPOINT: u16 = 231;
const SP_AP: u16 = 232;
const SP_MAXAP: u16 = 233;
const SP_UPOW: u16 = 247;
/// The next-point cost of a stat at 1.
const STAT_COST_AT_1: u8 = 2;
const NEXT_BASE_EXP: i64 = 99_999_999;

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn gid(n: u32) -> u32 {
    2_000_000 + n
}

fn jobs() -> Arc<JobDb> {
    static JOBS: OnceLock<Arc<JobDb>> = OnceLock::new();
    Arc::clone(JOBS.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ro-db/data/job");
        Arc::new(JobDb::load_dir(&dir).expect("shipped job data loads"))
    }))
}

fn instance() -> MapInstance {
    let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("dims"));
    let mut engine = MapInstance::new("prontera.gat", grid);
    engine.set_registries(Registries {
        jobs: jobs(),
        ..Registries::default()
    });
    engine
}

/// A Dragon Knight J1 (job bonus STR +1, POW +1), STR..LUK allocated 1, with `traits`
/// allocated, `trait_point` unspent and `ap`.
fn dk_status(traits: [u16; 6], trait_point: u16, ap: u32) -> CharacterStatus {
    CharacterStatus {
        traits,
        trait_point,
        ap,
        hp: 1_000_000,
        sp: 1_000_000,
        status_point: 5000,
        ..CharacterStatus::default()
    }
}

fn spawn(job: u16, base_level: u16, status: CharacterStatus) -> CharacterSpawn {
    CharacterSpawn {
        account_id: AccountId(gid(A)),
        char_id: char_id(A),
        name: format!("Char{A}"),
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
        group_id: 99,
        inventory: CharacterInventory::default(),
        skills: Vec::new(),
        hidden_skills: Vec::new(),
        change_level: [0; 3],
    }
}

struct World {
    engine: MapInstance,
    now: u64,
    /// What B saw in the last tick.
    observed: Vec<ServerEvent>,
}

impl World {
    fn new() -> Self {
        Self {
            engine: instance(),
            now: 0,
            observed: Vec::new(),
        }
    }

    /// Spawns B, a Knight, next to A.
    fn observer(&mut self) {
        let data = CharacterSpawn {
            account_id: AccountId(gid(B)),
            char_id: char_id(B),
            name: format!("Char{B}"),
            group_id: 0,
            pos: MapPosition {
                x: 52,
                y: 52,
                dir: 0,
            },
            ..spawn(7, 99, CharacterStatus::default())
        };
        self.engine.spawn_character(data, 0).expect("spawn B");
        self.tick(&[(B, ClientIntent::NotifyActorInit)]);
    }

    /// Spawns A: (the pre-ready login push, the actor init) as A sees them.
    fn login(&mut self, data: CharacterSpawn) -> (Vec<ServerEvent>, Vec<ServerEvent>) {
        let burst = seen_by(&self.engine.spawn_character(data, 0).expect("spawn"), A);
        let init = self.tick(&[(A, ClientIntent::NotifyActorInit)]);
        (burst, init)
    }

    fn tick(&mut self, intents: &[(u32, ClientIntent)]) -> Vec<ServerEvent> {
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
        self.observed = seen_by(&out, B);
        seen_by(&out, A)
    }

    /// A says `@<line>`; what A got that tick.
    fn command(&mut self, line: &str) -> Vec<ServerEvent> {
        let text = format!("Char{A} : @{line}").into_bytes();
        self.tick(&[(A, ClientIntent::Chat { text })])
    }

    fn status(&self) -> CharacterStatus {
        self.progress().status
    }

    fn progress(&self) -> ro_core::CharacterProgress {
        self.engine
            .progress_snapshot(char_id(A))
            .expect("A is on the map")
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

fn couple(var_id: u32, base: i32, bonus: i32) -> ServerEvent {
    ServerEvent::CoupleStatus {
        var_id,
        base,
        bonus,
    }
}

fn cost(var_id: u16, value: u8) -> ServerEvent {
    ServerEvent::StatusCost { var_id, value }
}

fn index_of(events: &[ServerEvent], wanted: &ServerEvent) -> usize {
    events
        .iter()
        .position(|e| e == wanted)
        .unwrap_or_else(|| panic!("{wanted:?} not in {events:?}"))
}

fn is_trait_value(event: &ServerEvent) -> bool {
    match event {
        ServerEvent::CoupleStatus { var_id, .. } => (219..=224).contains(var_id),
        ServerEvent::ParameterChanged { var_id, .. } => (225..=233).contains(var_id),
        _ => false,
    }
}

/// ENG-TRAIT-07: the pre-ready push appends only the non-zero trait values after `SP` and before
/// `WEIGHT` (captured 1401-1403: POW base 2 bonus 1, PATK 1, then WEIGHT).
#[test]
fn eng_trait_07_pre_ready_push_appends_non_zero_trait_values() {
    let mut world = World::new();
    let (burst, _) = world.login(spawn(
        DRAGON_KNIGHT,
        99,
        dk_status([2, 0, 0, 0, 0, 0], 5, 0),
    ));
    let traits: Vec<ServerEvent> = burst
        .iter()
        .filter(|e| is_trait_value(e))
        .cloned()
        .collect();
    assert_eq!(traits, vec![couple(SP_POW, 2, 1), p(SP_PATK, 1)]);
    let sp = burst
        .iter()
        .position(|e| matches!(e, ServerEvent::ParameterChanged { var_id: SP_SP, .. }))
        .expect("SP pushed");
    let weight = burst
        .iter()
        .position(|e| {
            matches!(
                e,
                ServerEvent::ParameterChanged {
                    var_id: SP_WEIGHT,
                    ..
                }
            )
        })
        .expect("WEIGHT pushed");
    assert_eq!(index_of(&burst, &couple(SP_POW, 2, 1)), sp + 1);
    assert_eq!(index_of(&burst, &p(SP_PATK, 1)), sp + 2);
    assert_eq!(weight, sp + 3);

    // At B200 MaxAP 200 follows the derived values, then a non-zero AP.
    let mut world = World::new();
    let (burst, _) = world.login(spawn(
        DRAGON_KNIGHT,
        200,
        dk_status([0, 3, 0, 0, 0, 0], 0, 40),
    ));
    let traits: Vec<ServerEvent> = burst
        .iter()
        .filter(|e| is_trait_value(e))
        .cloned()
        .collect();
    assert_eq!(
        traits,
        vec![
            couple(SP_POW, 0, 1),
            couple(SP_POW + 1, 3, 0),
            p(SP_RES, 8),
            p(SP_MAXAP, 200),
            p(SP_AP, 40),
        ]
    );
}

/// ENG-TRAIT-07: the block after `0x00bd` carries the real POW..CRT, PATK..CRATE,
/// `SP_TRAITPOINT`, `SP_AP` (even at 0), `SP_MAXAP` and the next-point costs (captured
/// 1451-1471).
#[test]
fn eng_trait_07_initial_status_block_has_real_values() {
    let mut world = World::new();
    let (_, init) = world.login(spawn(
        DRAGON_KNIGHT,
        99,
        dk_status([2, 0, 0, 0, 0, 0], 5, 0),
    ));
    let aspd = init
        .iter()
        .position(|e| {
            matches!(
                e,
                ServerEvent::ParameterChanged {
                    var_id: SP_ASPD,
                    ..
                }
            )
        })
        .expect("the block's ASPD");
    let mut expected = vec![couple(SP_POW, 2, 1)];
    expected.extend((1..6).map(|i| couple(SP_POW + i, 0, 0)));
    expected.extend([
        p(SP_PATK, 1),
        p(226, 0),
        p(227, 0),
        p(228, 0),
        p(229, 0),
        p(230, 0),
        p(SP_TRAITPOINT, 5),
        p(SP_AP, 0),
        p(SP_MAXAP, 0),
    ]);
    expected.extend((0..6).map(|i| cost(SP_UPOW + i, 1)));
    assert_eq!(init[aspd + 1..aspd + 1 + expected.len()], expected[..]);

    // A non-trait job: every trait cost is 0.
    let mut world = World::new();
    let (_, init) = world.login(spawn(7, 99, CharacterStatus::default()));
    let costs: Vec<u8> = init
        .iter()
        .filter_map(|e| match e {
            ServerEvent::StatusCost { var_id, value } if *var_id >= SP_UPOW => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(costs, vec![0; 6]);
}

/// ENG-TRAIT-07: a map change's actor init sends the real trait `0x0141` ×6.
#[test]
fn eng_trait_07_map_change_sends_real_trait_couples() {
    let mut world = World::new();
    world
        .engine
        .enter_by_warp(
            spawn(DRAGON_KNIGHT, 99, dk_status([2, 0, 0, 0, 4, 0], 0, 0)),
            (60, 60),
            0,
            0,
        )
        .expect("arrives");
    let init = world.tick(&[(A, ClientIntent::NotifyActorInit)]);
    let traits: Vec<ServerEvent> = init
        .iter()
        .filter(|e| matches!(e, ServerEvent::CoupleStatus { var_id, .. } if *var_id >= SP_POW))
        .cloned()
        .collect();
    assert_eq!(
        traits,
        vec![
            couple(SP_POW, 2, 1),
            couple(SP_POW + 1, 0, 0),
            couple(SP_POW + 2, 0, 0),
            couple(SP_POW + 3, 0, 0),
            couple(SP_POW + 4, 4, 0),
            couple(SP_POW + 5, 0, 0),
        ]
    );
}

/// ENG-TRAIT-08: a stored AP above the new MaxAP (B200 → B199: 200 → 0) is clamped and pushed
/// with the HP/SP clamp pushes, before the diff; the diff then ends with `MAXAP` and `AP`.
#[test]
fn eng_trait_08_ap_clamp_after_a_level_drop() {
    let mut world = World::new();
    world.login(spawn(DRAGON_KNIGHT, 200, dk_status([0; 6], 0, 150)));
    assert_eq!(world.status().ap, 150, "within MaxAP 200 at B200");
    let out = world.command("blvl -1");
    let clamp = index_of(&out, &p(SP_AP, 0));
    let max_ap = index_of(&out, &p(SP_MAXAP, 0));
    assert!(
        clamp < max_ap,
        "the clamp push comes before the diff: {out:?}"
    );
    assert_eq!(
        out.iter().filter(|e| **e == p(SP_AP, 0)).count(),
        2,
        "the clamp and the diff's AP: {out:?}"
    );
    assert_eq!(out[max_ap + 1], p(SP_AP, 0), "AP right after MAXAP");
    // MaxHP and MaxSP drop with the level too: the clamps go HP, SP, then AP.
    let var_at = |i: usize| match &out[i] {
        ServerEvent::ParameterChanged { var_id, .. } => *var_id,
        other => panic!("not a 0x00b0: {other:?}"),
    };
    assert_eq!((var_at(clamp - 2), var_at(clamp - 1)), (SP_HP, SP_SP));
    assert_eq!(world.status().ap, 0);
}

/// ENG-TRAIT-08: at spawn the clamp is silent: no `SP_AP` with the old value, and the stored AP
/// is the clamped one.
#[test]
fn eng_trait_08_spawn_clamps_ap_silently() {
    let mut world = World::new();
    let (burst, init) = world.login(spawn(DRAGON_KNIGHT, 199, dk_status([0; 6], 0, 150)));
    assert_eq!(world.status().ap, 0);
    assert!(
        !burst
            .iter()
            .any(|e| matches!(e, ServerEvent::ParameterChanged { var_id: SP_AP, .. })),
        "AP 0 is not in the pre-ready push"
    );
    assert!(init.contains(&p(SP_AP, 0)), "the status block sends AP 0");
    assert!(!init.contains(&p(SP_AP, 150)));
}

fn raise(world: &mut World, var: u16, amount: i16) -> Vec<ServerEvent> {
    world.tick(&[(A, ClientIntent::RaiseTrait { var, amount })])
}

fn ack(var_id: u16, ok: bool, value: u8) -> ServerEvent {
    ServerEvent::StatusChangeAck { var_id, ok, value }
}

fn param_value(events: &[ServerEvent], var: u16) -> i32 {
    events
        .iter()
        .find_map(|e| match e {
            ServerEvent::ParameterChanged { var_id, value } if *var_id == var => Some(*value),
            _ => None,
        })
        .unwrap_or_else(|| panic!("var {var} not in {events:?}"))
}

/// ENG-TRAIT-01: trait-up success in the captured order (`rathena_traits_sequence.txt`
/// 488-506, 515-521): the diff (`ATK1` +5, then POW `0x0141` base = allocated, bonus = the job's
/// POW 1, then `PATK` once POW totals 3), `0x00be` `SP_UPOW` 1, `SP_TRAITPOINT`, then `0x00bc`
/// with the allocated value.
#[test]
fn eng_trait_01_pow_up_in_the_captured_order() {
    let mut world = World::new();
    world.login(spawn(DRAGON_KNIGHT, 200, dk_status([0; 6], 7, 0)));

    let first = raise(&mut world, SP_POW as u16, 1);
    assert_eq!(
        first[first.len() - 4..],
        [
            couple(SP_POW, 1, 1),
            cost(SP_UPOW, 1),
            p(SP_TRAITPOINT, 6),
            ack(SP_POW as u16, true, 1),
        ],
        "{first:?}"
    );
    assert_eq!(first.len(), 5, "ATK1, then the tail: {first:?}");

    let second = raise(&mut world, SP_POW as u16, 1);
    assert_eq!(
        second[1..],
        [
            couple(SP_POW, 2, 1),
            p(SP_PATK, 1),
            cost(SP_UPOW, 1),
            p(SP_TRAITPOINT, 5),
            ack(SP_POW as u16, true, 2),
        ],
        "{second:?}"
    );
    assert_eq!(
        param_value(&second, SP_ATK1) - param_value(&first, SP_ATK1),
        5,
        "status ATK +5 per POW"
    );
    let status = world.status();
    assert_eq!((status.traits[0], status.trait_point), (2, 5));
}

/// ENG-TRAIT-01: CON pushes `HIT` and `FLEE1` (+2 each) before its own `0x0141` (captured
/// 516-521).
#[test]
fn eng_trait_01_con_up_pushes_hit_and_flee_first() {
    let mut world = World::new();
    world.login(spawn(DRAGON_KNIGHT, 200, dk_status([0; 6], 7, 0)));
    let before = world.engine.progress_snapshot(char_id(A)).expect("on map");
    assert_eq!(before.status.trait_point, 7);
    let out = raise(&mut world, SP_CON, 1);
    let vars: Vec<u16> = out
        .iter()
        .map(|e| match e {
            ServerEvent::ParameterChanged { var_id, .. } => *var_id,
            ServerEvent::CoupleStatus { var_id, .. } => *var_id as u16,
            ServerEvent::StatusCost { var_id, .. } => *var_id,
            ServerEvent::StatusChangeAck { var_id, .. } => *var_id,
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        vars,
        vec![SP_HIT, SP_FLEE1, SP_CON, SP_UPOW + 4, SP_TRAITPOINT, SP_CON]
    );
    assert_eq!(out[2], couple(SP_CON.into(), 1, 0));
    assert_eq!(out[5], ack(SP_CON, true, 1));
}

/// ENG-TRAIT-01: an amount above the pool is capped to it, not refused (source-only).
#[test]
fn eng_trait_01_amount_is_capped_by_the_points() {
    let mut world = World::new();
    world.login(spawn(DRAGON_KNIGHT, 200, dk_status([0; 6], 3, 0)));
    let out = raise(&mut world, SP_POW as u16, 5);
    assert_eq!(
        out[out.len() - 3..],
        [
            cost(SP_UPOW, 1),
            p(SP_TRAITPOINT, 0),
            ack(SP_POW as u16, true, 3),
        ]
    );
    let status = world.status();
    assert_eq!((status.traits[0], status.trait_point), (3, 0));

    // ...and by the cap (110): the last point costs 0 next.
    let mut world = World::new();
    world.login(spawn(
        DRAGON_KNIGHT,
        200,
        dk_status([108, 0, 0, 0, 0, 0], 9, 0),
    ));
    let out = raise(&mut world, SP_POW as u16, 5);
    assert_eq!(
        out[out.len() - 3..],
        [
            cost(SP_UPOW, 0),
            p(SP_TRAITPOINT, 7),
            ack(SP_POW as u16, true, 110),
        ]
    );
}

/// ENG-TRAIT-02: every failure sends only `0x00bc` ok 0 value 0 for the requested var, at once,
/// and changes nothing (source-only: the client blocks 0-point requests).
#[test]
fn eng_trait_02_failures_send_only_the_refusal() {
    let cases = [
        ("non-trait job", 7, [0; 6], 5, SP_POW as u16, 1),
        ("no points", DRAGON_KNIGHT, [0; 6], 0, SP_POW as u16, 1),
        (
            "at the cap",
            DRAGON_KNIGHT,
            [110, 0, 0, 0, 0, 0],
            5,
            SP_POW as u16,
            1,
        ),
        ("a primary stat", DRAGON_KNIGHT, [0; 6], 5, 13, 1),
        ("amount 0", DRAGON_KNIGHT, [0; 6], 5, SP_POW as u16, 0),
    ];
    for (case, job, traits, points, var, amount) in cases {
        let mut world = World::new();
        world.login(spawn(job, 200, dk_status(traits, points, 0)));
        let out = raise(&mut world, var, amount);
        assert_eq!(out, vec![ack(var, false, 0)], "{case}");
        let status = world.status();
        assert_eq!(
            (status.traits, status.trait_point),
            (traits, points),
            "{case}"
        );
    }
}

/// ENG-TRAIT-02: a negative amount is dropped with no reply.
#[test]
fn eng_trait_02_negative_amount_is_dropped() {
    let mut world = World::new();
    world.login(spawn(DRAGON_KNIGHT, 200, dk_status([0; 6], 5, 0)));
    let out = raise(&mut world, SP_POW as u16, -1);
    assert!(out.is_empty(), "{out:?}");
    let status = world.status();
    assert_eq!((status.traits, status.trait_point), ([0; 6], 5));
}

fn stat_costs(value: u8) -> Vec<ServerEvent> {
    (0..6).map(|i| cost(SP_USTR + i, value)).collect()
}

fn trait_cost_pushes(value: u8) -> Vec<ServerEvent> {
    (0..6).map(|i| cost(SP_UPOW + i, value)).collect()
}

/// `pc_resetstate`'s immediate pushes (spec §2.6): the stat costs, the trait costs, then both
/// pools.
fn reset_pushes(trait_cost: u8, status_point: i32, trait_point: i32) -> Vec<ServerEvent> {
    let mut out = stat_costs(STAT_COST_AT_1);
    out.extend(trait_cost_pushes(trait_cost));
    out.push(p(SP_STATUSPOINT, status_point));
    out.push(p(SP_TRAITPOINT, trait_point));
    out
}

/// The base tail of `@blvl` and a capped `@job`.
fn base_tail(status_point: i32, trait_point: i32, level: i32) -> Vec<ServerEvent> {
    vec![
        p(SP_STATUSPOINT, status_point),
        p(SP_TRAITPOINT, trait_point),
        p(SP_BASELEVEL, level),
        ServerEvent::LongLongParameterChanged {
            var_id: SP_BASEEXP,
            value: 0,
        },
        ServerEvent::LongLongParameterChanged {
            var_id: SP_NEXTBASEEXP,
            value: NEXT_BASE_EXP,
        },
    ]
}

fn is_message(event: &ServerEvent) -> bool {
    matches!(event, ServerEvent::SelfChat { .. })
}

/// `SP_TRAITPOINT` right before `SP_BASELEVEL` `level` in a base tail.
fn tail_trait_point(out: &[ServerEvent], level: i32) -> ServerEvent {
    let at = index_of(out, &p(SP_BASELEVEL, level));
    out[at - 1].clone()
}

fn table_points(level: u16) -> i32 {
    i32::try_from(jobs().status_points(level)).expect("small")
}

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
    let tail = base_tail(3980, 7, 195);
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
    want.extend(base_tail(1273, 0, 99));
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
