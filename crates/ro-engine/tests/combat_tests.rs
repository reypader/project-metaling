//! Player auto-attack in a map instance (spec `specs/combat/spec.md` ENG-CMB-01..10). Expected
//! packet orders are the captured ones (catalog `mechanics/auto_attack.md` §8, `combat.md` §7).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::{Arc, OnceLock};

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterInventory, CharacterSpawn, CharacterStatus,
    ClientIntent, InventoryItem, ItemId, MapPosition, ScopedServerEvent, ServerEvent, SessionId,
    SessionIntent, Sex,
};
use ro_db::{
    AmmoType, CellType, ItemData, ItemDb, ItemType, JobDb, MapGrid, MapMobSpawns, MapNpcs, MobData,
    MobDb, MobId, MobSpawn, NpcKind, NpcSpawn, NpcState, WarpDest, WeaponType,
};
use ro_engine::{MapInstance, Registries};

const A: u32 = 1;
const B: u32 = 2;

const KNIFE: u32 = 1201;
const BOW: u32 = 1701;
const ARROW: u32 = 1750;
const REVOLVER: u32 = 13100;

const EQP_HAND_R: u32 = 0x02;
const EQP_ARMS: u32 = 0x22;
const EQP_AMMO: u32 = 0x8000;

const ACT_ATTACK: u8 = 0;
const ACT_ATTACK_REPEAT: u8 = 7;
const ACT_SIT: u8 = 2;

/// The mob's fixed cell.
const MOB: (u16, u16) = (50, 50);
const MOB_DMOTION: i32 = 480;
const SP_WEIGHT: u16 = 24;

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn gid(n: u32) -> u32 {
    2_000_000 + n
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

fn weapon(id: u32, weapon_type: WeaponType, locations: u32, attack: u32, range: u8) -> ItemData {
    ItemData {
        weapon_type,
        attack,
        range,
        weapon_level: 1,
        ..item_data(id, ItemType::Weapon, locations)
    }
}

fn items() -> Arc<ItemDb> {
    Arc::new(
        ItemDb::from_items(
            "test",
            vec![
                weapon(KNIFE, WeaponType::Dagger, EQP_HAND_R, 17, 1),
                weapon(BOW, WeaponType::Bow, EQP_ARMS, 15, 5),
                weapon(REVOLVER, WeaponType::Revolver, EQP_HAND_R, 30, 9),
                ItemData {
                    ammo_type: Some(AmmoType::Arrow),
                    attack: 25,
                    weight: 1,
                    ..item_data(ARROW, ItemType::Ammo, EQP_AMMO)
                },
            ],
        )
        .expect("items"),
    )
}

/// A Poring that never dies here (M4 has no death) and is always hit by a level-99 attacker.
fn mobs() -> Arc<MobDb> {
    let poring = MobData {
        id: MobId(1002),
        aegis_name: "PORING".into(),
        name: "Poring".into(),
        level: 1,
        hp: 1_000_000,
        luk: 10,
        attack_motion: 672,
        attack_delay: 1872,
        damage_motion: 480,
        walk_speed: 400,
        ..MobData::default()
    };
    Arc::new(MobDb::from_mobs("test", vec![poring]).expect("mobs"))
}

fn worn(id: u32, equip: u32, amount: u16) -> Option<InventoryItem> {
    let mut item = InventoryItem::new(ItemId(id), amount);
    item.equip = equip;
    Some(item)
}

fn spawn_data(n: u32, at: (u16, u16), inventory: Vec<Option<InventoryItem>>) -> CharacterSpawn {
    CharacterSpawn {
        account_id: AccountId(gid(n)),
        char_id: char_id(n),
        name: format!("Char{n}"),
        sex: Sex::Male,
        job_class: 0,
        base_level: 99,
        hair_style: 2,
        hair_color: 3,
        pos: MapPosition {
            x: at.0,
            y: at.1,
            dir: 0,
        },
        speed: 150,
        status: CharacterStatus::default(),
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

fn intent(n: u32, intent: ClientIntent) -> SessionIntent {
    SessionIntent::new(
        SessionId(u64::from(n)),
        Some(AccountId(gid(n))),
        Some(char_id(n)),
        intent,
    )
}

fn tick(engine: &mut MapInstance, now: u64) -> Vec<ScopedServerEvent> {
    let mut out = Vec::new();
    engine.step_tick(now, &mut out).expect("tick");
    out
}

fn send(engine: &mut MapInstance, n: u32, i: ClientIntent, now: u64) -> Vec<ScopedServerEvent> {
    engine.push_intent(intent(n, i));
    tick(engine, now)
}

fn attack(engine: &mut MapInstance, n: u32, action: u8, now: u64) -> Vec<ScopedServerEvent> {
    let target_id = mob_gid(engine);
    send(
        engine,
        n,
        ClientIntent::RequestAction {
            target_id,
            action_type: action,
        },
        now,
    )
}

/// A player: number, cell and inventory.
type Player = (u32, (u16, u16), Vec<Option<InventoryItem>>);

/// A map with one Poring at `MOB` (plus `npcs`), and the players at their cells, entered at 0.
fn world(players: Vec<Player>, npcs: Vec<NpcSpawn>) -> MapInstance {
    let spawns = players
        .into_iter()
        .map(|(n, at, inventory)| spawn_data(n, at, inventory))
        .collect();
    let registries = Registries {
        items: items(),
        mobs: mobs(),
        ..Registries::default()
    };
    world_with(registries, spawns, npcs, &[MOB])
}

/// `world` with any registries, full character spawns and a Poring (id 1002) on each of
/// `mob_cells`, in gid order.
fn world_with(
    registries: Registries,
    players: Vec<CharacterSpawn>,
    npcs: Vec<NpcSpawn>,
    mob_cells: &[(u16, u16)],
) -> MapInstance {
    let grid = Arc::new(MapGrid::all_walkable(200, 200).expect("dims"));
    let mut engine = MapInstance::new("prt_fild08.gat", grid);
    engine.set_registries(registries);
    let spawns: Vec<MobSpawn> = mob_cells
        .iter()
        .map(|&(x, y)| MobSpawn {
            name: "Poring".into(),
            mob_id: MobId(1002),
            x,
            y,
            span: (0, 0),
            amount: 1,
            delay1: 5000,
            delay2: 0,
            level: None,
            boss: false,
            event: None,
            source: "test:poring".into(),
        })
        .collect();
    assert_eq!(engine.spawn_mobs(&MapMobSpawns { spawns }), mob_cells.len());
    let cells: Vec<(u16, u16)> = engine.mobs().map(|m| (m.pos.x, m.pos.y)).collect();
    assert_eq!(cells, mob_cells, "1x1 lines place the mobs exactly");
    let count = npcs.len();
    assert_eq!(engine.spawn_npcs(&MapNpcs { npcs }), count);
    for spawn in players {
        let n = spawn.char_id.0 - char_id(0).0;
        engine.spawn_character(spawn, 0).expect("spawn");
        send(&mut engine, n, ClientIntent::NotifyActorInit, 0);
    }
    engine
}

/// A bare-handed attacker `A` at `at`.
fn solo(at: (u16, u16)) -> MapInstance {
    world(vec![(A, at, Vec::new())], Vec::new())
}

fn mob_gid(engine: &MapInstance) -> u32 {
    engine.mobs().next().expect("mob").gid
}

fn mob_hp(engine: &MapInstance) -> u32 {
    engine.mobs().next().expect("mob").hp
}

fn seen_by(events: &[ScopedServerEvent], n: u32) -> Vec<ServerEvent> {
    events
        .iter()
        .filter(|e| match &e.scope {
            BroadcastScope::SelfOnly(c) => *c == char_id(n),
            BroadcastScope::Characters(cs) => cs.contains(&char_id(n)),
            _ => false,
        })
        .map(|e| e.event.clone())
        .collect()
}

fn stopped(n: u32, at: (u16, u16)) -> ServerEvent {
    ServerEvent::UnitStopped {
        gid: gid(n),
        x: at.0,
        y: at.1,
    }
}

/// `events` without the `0x0977`s, for ticks that also land an earlier hit.
fn without_bars(events: Vec<ServerEvent>) -> Vec<ServerEvent> {
    events
        .into_iter()
        .filter(|e| !matches!(e, ServerEvent::MobHpBar { .. }))
        .collect()
}

/// The swing packets in `events`: `(tick, damage, dmg_speed, src_speed)`.
fn swings(events: &[ServerEvent]) -> Vec<(u32, i32, i32, i32)> {
    events
        .iter()
        .filter_map(|e| match e {
            ServerEvent::Damage {
                tick,
                damage,
                dmg_speed,
                src_speed,
                ..
            } => Some((*tick, *damage, *dmg_speed, *src_speed)),
            _ => None,
        })
        .collect()
}

/// Ticks every `step` ms over `(from, to]`, collecting what `n` sees.
fn run(engine: &mut MapInstance, n: u32, from: u64, to: u64, step: u64) -> Vec<ServerEvent> {
    let mut seen = Vec::new();
    let mut now = from + step;
    while now <= to {
        seen.extend(seen_by(&tick(engine, now), n));
        now += step;
    }
    seen
}

/// ENG-CMB-01: action 7 on an adjacent Poring: `0x0088` ×2 then `0x08c8` to the attacker and the
/// observer; the HP lands at swing + amotion; the next swing at + adelay; no drift.
#[test]
fn eng_cmb_01_continuous_attack_on_adjacent_mob() {
    let a_at = (51, 50);
    let mut engine = world(
        vec![(A, a_at, Vec::new()), (B, (53, 53), Vec::new())],
        Vec::new(),
    );
    let events = attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000);
    let to_a = seen_by(&events, A);
    assert_eq!(to_a.len(), 3, "{to_a:?}");
    assert_eq!(to_a[..2], [stopped(A, a_at), stopped(A, a_at)]);
    let ServerEvent::Damage {
        source_id,
        target_id,
        tick: swing_tick,
        src_speed: amotion,
        dmg_speed,
        damage,
        div,
        damage_type,
        damage2,
    } = to_a[2].clone()
    else {
        panic!("third packet is the swing: {to_a:?}");
    };
    assert_eq!(
        (
            source_id,
            target_id,
            swing_tick,
            dmg_speed,
            div,
            damage_type,
            damage2
        ),
        (gid(A), mob_gid(&engine), 1000, MOB_DMOTION, 1, 0, 0)
    );
    assert!(damage > 0);
    assert_eq!(seen_by(&events, B), to_a, "the observer sees the same");

    // The HP changes at swing + amotion, not before.
    let amotion = u64::try_from(amotion).expect("positive amotion");
    let full = 1_000_000;
    assert_eq!(mob_hp(&engine), full);
    tick(&mut engine, 1000 + amotion - 1);
    assert_eq!(mob_hp(&engine), full);
    let landed = tick(&mut engine, 1000 + amotion);
    let hp = full - u32::try_from(damage).expect("positive");
    assert_eq!(mob_hp(&engine), hp);
    // The HP bar goes to the damage log only (captured).
    assert_eq!(
        seen_by(&landed, A),
        vec![ServerEvent::MobHpBar {
            gid: mob_gid(&engine),
            hp,
            max_hp: full
        }]
    );
    assert!(seen_by(&landed, B).is_empty(), "{landed:?}");

    // Ten swings at a 37 ms tick (it doesn't divide adelay = 2 × amotion, so most swings fire on a
    // late tick): each swing's tick is the previous due time + adelay, never the late tick.
    let adelay = 2 * amotion;
    let seen = run(&mut engine, A, 1000 + amotion, 1000 + 9 * adelay + 36, 37);
    let ticks: Vec<u32> = swings(&seen).iter().map(|s| s.0).collect();
    let expected: Vec<u32> = (1..=9).map(|k| (1000 + k * adelay) as u32).collect();
    assert_eq!(ticks, expected);
    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, ServerEvent::UnitStopped { .. })),
        "loop swings are bare"
    );
}

/// ENG-CMB-02: action 0 swings once. A re-request before `canact` is dropped silently and ends
/// the loop. A request after `canact` but before `attackable` swings at `attackable`, bare.
#[test]
fn eng_cmb_02_single_swing_canact_and_deferred() {
    let mut engine = solo((51, 50));
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK, 1000), A);
    let amotion = u64::try_from(swings(&events)[0].3).expect("amotion");
    let adelay = 2 * amotion;
    assert!(swings(&run(&mut engine, A, 1000, 1000 + 3 * adelay, 20)).is_empty());

    // Continuous, then a re-request at +100 ms (inside amotion + 100): nothing, loop over.
    let start = 10_000;
    attack(&mut engine, A, ACT_ATTACK_REPEAT, start);
    assert!(seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, start + 100), A).is_empty());
    assert!(swings(&run(&mut engine, A, start + 100, start + 3 * adelay, 20)).is_empty());

    // Single swing, then a request between canact (amotion + 100) and attackable (adelay).
    let start = 20_000;
    attack(&mut engine, A, ACT_ATTACK, start);
    let between = start + amotion + 120;
    // (The first swing's hit lands in this tick: only its HP bar.)
    assert!(without_bars(seen_by(
        &attack(&mut engine, A, ACT_ATTACK_REPEAT, between),
        A
    ))
    .is_empty());
    let seen = run(&mut engine, A, between, start + adelay, 10);
    assert_eq!(swings(&seen).len(), 1);
    assert_eq!(swings(&seen)[0].0, (start + adelay) as u32);
    assert!(!seen
        .iter()
        .any(|e| matches!(e, ServerEvent::UnitStopped { .. })));
    // It was a continuous request: the loop goes on.
    let seen = run(&mut engine, A, start + adelay, start + 2 * adelay, 10);
    assert_eq!(swings(&seen).len(), 1);
}

/// ENG-CMB-03: out of range gets one `0x0088` then `0x0139`, and the loop ends. Range 1 hits at
/// 2 cells straight and misses at (2,1). A wall between a bow and its target ends the attack
/// after one `0x0088`: no `0x0139`, no swing, no arrow spent.
#[test]
fn eng_cmb_03_range_and_line_of_sight() {
    let a_at = (53, 51);
    let mut engine = solo(a_at);
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000), A);
    assert_eq!(
        events,
        [
            stopped(A, a_at),
            ServerEvent::AttackOutOfRange {
                target_id: mob_gid(&engine),
                target_x: MOB.0,
                target_y: MOB.1,
                x: a_at.0,
                y: a_at.1,
                range: 1,
            }
        ]
    );
    assert!(run(&mut engine, A, 1000, 5000, 20).is_empty());

    let mut engine = solo((52, 50));
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK, 1000), A);
    assert_eq!(swings(&events).len(), 1, "2 cells straight is in range 1");

    let mut engine = solo((52, 51));
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK, 1000), A);
    assert!(matches!(
        events.as_slice(),
        [
            ServerEvent::UnitStopped { .. },
            ServerEvent::AttackOutOfRange { .. }
        ]
    ));

    // Out of range and behind a wall: the OFFICIAL_WALKPATH line check runs before the range
    // test, so the drop is silent (no `0x0139`).
    let a_at = (45, 50);
    let mut engine = solo(a_at);
    assert!(engine.set_cell(47, 50, CellType::Wall));
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000), A);
    assert_eq!(events, [stopped(A, a_at)]);

    let archer = vec![worn(BOW, EQP_ARMS, 1), worn(ARROW, EQP_AMMO, 10)];
    let a_at = (45, 50);
    let mut engine = world(vec![(A, a_at, archer)], Vec::new());
    assert!(engine.set_cell(47, 50, CellType::Wall));
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000), A);
    assert_eq!(events, [stopped(A, a_at)]);
    assert!(run(&mut engine, A, 1000, 5000, 20).is_empty());
    assert_eq!(
        engine.weight(char_id(A)).expect("weight").0,
        10 + 10,
        "no arrow spent"
    );
}

/// ENG-CMB-04: a move request, `0x0118`, sit, a warp and logout each stop the loop. A hit
/// already swung still lands, except after logout or a warp beyond its distance snapshot.
#[test]
fn eng_cmb_04_stops_and_pending_hits() {
    type Stop = fn(&mut MapInstance, u64);
    let cases: [(&str, Stop, bool); 5] = [
        (
            "move",
            |engine, now| {
                send(
                    engine,
                    A,
                    ClientIntent::RequestMove {
                        dest_x: 51,
                        dest_y: 47,
                    },
                    now,
                );
            },
            true,
        ),
        (
            "cancel",
            |engine, now| {
                send(engine, A, ClientIntent::CancelAttack, now);
            },
            true,
        ),
        (
            "sit",
            |engine, now| {
                send(
                    engine,
                    A,
                    ClientIntent::RequestAction {
                        target_id: 0,
                        action_type: ACT_SIT,
                    },
                    now,
                );
            },
            true,
        ),
        (
            "warp",
            |engine, now| {
                // Two cells onto a same-map warp to (150, 150): it fires at now + 300.
                send(
                    engine,
                    A,
                    ClientIntent::RequestMove {
                        dest_x: 51,
                        dest_y: 52,
                    },
                    now,
                );
            },
            false,
        ),
        (
            "logout",
            |engine, _| {
                engine.remove_character(char_id(A)).expect("remove");
            },
            false,
        ),
    ];
    for (name, stop, lands) in cases {
        let warp = NpcSpawn {
            name: "w".into(),
            unique_name: "w".into(),
            x: 51,
            y: 52,
            dir: 0,
            sprite: 45,
            sprite_is_mob: false,
            state: NpcState::Normal,
            trigger: Some((0, 0)),
            duplicate_of: None,
            kind: NpcKind::Warp(WarpDest {
                map: "prt_fild08".into(),
                x: 150,
                y: 150,
                trigger_while_hidden: false,
            }),
            source: "test:warp".into(),
        };
        // B watches the swings so they are visible after A is gone.
        let mut engine = world(
            vec![(A, (51, 50), Vec::new()), (B, (53, 53), Vec::new())],
            vec![warp],
        );
        let first = swings(&seen_by(
            &attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000),
            A,
        ));
        let (amotion, damage) = (u64::try_from(first[0].3).expect("amotion"), first[0].1);
        stop(&mut engine, 1010);
        let seen = run(&mut engine, B, 1010, 1000 + 4 * amotion, 10);
        assert!(swings(&seen).is_empty(), "{name}: the loop stopped");
        let expected = if lands {
            1_000_000 - u32::try_from(damage).expect("positive")
        } else {
            1_000_000
        };
        assert_eq!(mob_hp(&engine), expected, "{name}: pending hit");
    }
}

/// ENG-CMB-05: a request while walking out of `range - 1` attacks half a step after the next
/// step ends, with range + 1: a mob at (2,1) from where the walk ends is hit.
#[test]
fn eng_cmb_05_step_action() {
    let mut engine = solo((40, 49));
    // Straight east, 8 cells at 150 ms each: steps end at 1150, 1300, ... 2200 at (48, 49).
    send(
        &mut engine,
        A,
        ClientIntent::RequestMove {
            dest_x: 48,
            dest_y: 49,
        },
        1000,
    );
    // Mid-walk at (43, 49), far from the mob: stored, nothing sent.
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, 1500), A);
    assert!(events.is_empty(), "{events:?}");
    let seen = run(&mut engine, A, 1500, 2274, 1);
    assert!(
        swings(&seen).is_empty(),
        "nothing before half a step after the last step"
    );
    let events = seen_by(&tick(&mut engine, 2275), A);
    // The walk ended at 2200 on (48, 49): (2, 1) from the mob, in range 1 + 1.
    assert_eq!(events[..2], [stopped(A, (48, 49)), stopped(A, (48, 49))]);
    assert_eq!(swings(&events).len(), 1);
    assert_eq!(swings(&events)[0].0, 2275);

    // Still walking when the target comes within range - 1: the walk stops with a third
    // `0x0088`. Walk past the mob along y = 49 from (44, 49) to (56, 49).
    let mut engine = solo((44, 49));
    send(
        &mut engine,
        A,
        ClientIntent::RequestMove {
            dest_x: 56,
            dest_y: 49,
        },
        1000,
    );
    attack(&mut engine, A, ACT_ATTACK_REPEAT, 1100);
    // (49, 49) is diagonal to the mob (range 0 fails); (50, 49) is straight below it at 1900.
    let seen = run(&mut engine, A, 1100, 1974, 1);
    assert!(swings(&seen).is_empty());
    let events = seen_by(&tick(&mut engine, 1975), A);
    assert_eq!(
        events[..3],
        [
            stopped(A, (50, 49)),
            stopped(A, (50, 49)),
            stopped(A, (50, 49))
        ]
    );
    assert_eq!(swings(&events)[0].0, 1975);
    assert!(!engine.is_walking(char_id(A)));
}

/// ENG-CMB-06: a bow with 2 arrows spends them in the captured order; the second takes the
/// stack off first; the third swing gets `0x013b` 0 and stops. A gun with no bullets gets
/// `0x0110` cause 84.
#[test]
fn eng_cmb_06_ammo() {
    let a_at = (46, 50);
    let archer = vec![worn(BOW, EQP_ARMS, 1), worn(ARROW, EQP_AMMO, 2)];
    let mut engine = world(
        vec![(A, a_at, archer), (B, (53, 53), Vec::new())],
        Vec::new(),
    );
    let all = attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000);
    let to_a = seen_by(&all, A);
    let arrows = 3; // wire index of slot 1
    assert_eq!(
        to_a[..4],
        [
            stopped(A, a_at),
            stopped(A, a_at),
            ServerEvent::ItemDeleted {
                index: arrows,
                amount: 1,
                reason: 1,
            },
            ServerEvent::ParameterChanged {
                var_id: SP_WEIGHT,
                value: 10 + 1,
            },
        ]
    );
    assert!(matches!(to_a[4], ServerEvent::Damage { .. }));
    assert_eq!(to_a.len(), 5, "{to_a:?}");
    let to_b = seen_by(&all, B);
    assert_eq!(to_b.len(), 3, "the observer sees no ammo packets: {to_b:?}");

    let amotion = u64::try_from(swings(&to_a)[0].3).expect("amotion");
    let adelay = 2 * amotion;
    // Each tick below also lands the previous swing's hit (its HP bar comes first).
    let second = without_bars(seen_by(&tick(&mut engine, 1000 + adelay), A));
    assert_eq!(
        second[..3],
        [
            ServerEvent::UnequipAck {
                index: arrows,
                location: EQP_AMMO,
                failed: false,
            },
            ServerEvent::ItemDeleted {
                index: arrows,
                amount: 1,
                reason: 1,
            },
            ServerEvent::ParameterChanged {
                var_id: SP_WEIGHT,
                value: 10,
            },
        ]
    );
    assert_eq!(
        swings(&second).len(),
        1,
        "the last arrow still fires: {second:?}"
    );

    let third = without_bars(seen_by(&tick(&mut engine, 1000 + 2 * adelay), A));
    assert_eq!(third, [ServerEvent::ActionFailure { failure_type: 0 }]);
    assert!(run(&mut engine, A, 1000 + 2 * adelay, 1000 + 5 * adelay, 20).is_empty());

    // A fresh click with no ammo: `0x0088` ×2 then `0x013b` 0.
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK, 20_000), A);
    assert_eq!(
        events,
        [
            stopped(A, a_at),
            stopped(A, a_at),
            ServerEvent::ActionFailure { failure_type: 0 }
        ]
    );

    let gunner = vec![worn(REVOLVER, EQP_HAND_R, 1)];
    let mut engine = world(vec![(A, a_at, gunner)], Vec::new());
    let events = seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000), A);
    assert_eq!(
        events,
        [
            stopped(A, a_at),
            stopped(A, a_at),
            ServerEvent::SkillFail {
                skill_id: 0,
                cause: 84,
            }
        ]
    );
}

/// ENG-CMB-05: a move and an attack in one tick run in order (captured chase: `0x035f` to the
/// cell next to the target, then `0x0437` 40 ms later; both can land in one tick). The walk
/// starts, the request becomes a step action, and the swing comes on arrival.
#[test]
fn eng_cmb_05_move_then_attack_in_one_tick() {
    // (2, 1) from the mob: out of range 1 while standing.
    let mut engine = solo((52, 51));
    engine.push_intent(intent(
        A,
        ClientIntent::RequestMove {
            dest_x: 51,
            dest_y: 51,
        },
    ));
    let target_id = mob_gid(&engine);
    engine.push_intent(intent(
        A,
        ClientIntent::RequestAction {
            target_id,
            action_type: ACT_ATTACK_REPEAT,
        },
    ));
    let events = seen_by(&tick(&mut engine, 1000), A);
    assert!(
        events
            .iter()
            .all(|e| matches!(e, ServerEvent::PlayerMoved { .. })),
        "only the walk starts: {events:?}"
    );
    // One straight step ends at 1150; the step action fires half a step later.
    let seen = run(&mut engine, A, 1000, 1224, 1);
    assert!(swings(&seen).is_empty(), "{seen:?}");
    let events = seen_by(&tick(&mut engine, 1225), A);
    assert_eq!(events[..2], [stopped(A, (51, 51)), stopped(A, (51, 51))]);
    assert_eq!(swings(&events).len(), 1);
}

/// ENG-CMB-04: a successful pickup stops the attack silently (`pc_takeitem` →
/// `unit_stop_attack`).
#[test]
fn eng_cmb_04_pickup_stops_the_loop() {
    let knife = Some(InventoryItem::new(ItemId(KNIFE), 1));
    let mut engine = world(
        vec![(A, (51, 50), vec![knife]), (B, (53, 53), Vec::new())],
        Vec::new(),
    );
    let dropped = send(
        &mut engine,
        A,
        ClientIntent::DropItem {
            index: 2,
            amount: 1,
        },
        100,
    );
    let floor_id = dropped
        .iter()
        .find_map(|e| match &e.event {
            ServerEvent::FloorItemDropped { item } => Some(item.id),
            _ => None,
        })
        .expect("dropped");
    let first = swings(&seen_by(
        &attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000),
        A,
    ));
    let adelay = 2 * u64::try_from(first[0].3).expect("amotion");
    send(&mut engine, A, ClientIntent::PickUpItem { floor_id }, 1010);
    assert!(
        engine.weight(char_id(A)).expect("weight").0 > 0,
        "picked up"
    );
    assert!(swings(&run(&mut engine, B, 1010, 1000 + 3 * adelay, 10)).is_empty());
}

// Kills, exp and level-ups (M5): the shipped job data, a Poring with its real exp.

const SP_BASEEXP: u16 = 1;
const SP_JOBEXP: u16 = 2;
const SP_HP: u16 = 5;
const SP_SP: u16 = 7;
const SP_STATUSPOINT: u16 = 9;
const SP_BASELEVEL: u16 = 11;
const SP_SKILLPOINT: u16 = 12;
const SP_NEXTBASEEXP: u16 = 22;
const SP_NEXTJOBEXP: u16 = 23;
const SP_JOBLEVEL: u16 = 55;
const SP_TRAITPOINT: u16 = 231;
const NOVICE: u16 = 0;
const VANISH_DIED: u8 = 1;
/// Poring 1002's db exp (`combat_sequence_1.txt`).
const PORING_EXP: (u64, u64) = (150, 40);

fn jobs() -> Arc<JobDb> {
    static JOBS: OnceLock<Arc<JobDb>> = OnceLock::new();
    Arc::clone(JOBS.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ro-db/data/job");
        Arc::new(JobDb::load_dir(&dir).expect("shipped job data loads"))
    }))
}

/// A level-1 Poring with `hp`, its real exp, and LUK 10 so a Novice never crits (bare-hand
/// damage is then fixed).
fn poring(hp: u32) -> Arc<MobDb> {
    let poring = MobData {
        id: MobId(1002),
        aegis_name: "PORING".into(),
        name: "Poring".into(),
        level: 1,
        hp,
        base_exp: PORING_EXP.0,
        job_exp: PORING_EXP.1,
        luk: 10,
        attack_motion: 672,
        attack_delay: 1872,
        damage_motion: 480,
        walk_speed: 400,
        ..MobData::default()
    };
    Arc::new(MobDb::from_mobs("test", vec![poring]).expect("mobs"))
}

/// A bare-handed Novice at `base_level` with `status`.
fn fighter(n: u32, at: (u16, u16), base_level: u16, status: CharacterStatus) -> CharacterSpawn {
    CharacterSpawn {
        base_level,
        status,
        ..spawn_data(n, at, Vec::new())
    }
}

fn kill_world(mob_hp: u32, players: Vec<CharacterSpawn>) -> MapInstance {
    kill_world_at(mob_hp, players, &[MOB])
}

fn kill_world_at(
    mob_hp: u32,
    players: Vec<CharacterSpawn>,
    mob_cells: &[(u16, u16)],
) -> MapInstance {
    let registries = Registries {
        items: items(),
        mobs: poring(mob_hp),
        jobs: jobs(),
        ..Registries::default()
    };
    world_with(registries, players, Vec::new(), mob_cells)
}

fn p(var_id: u16, value: i32) -> ServerEvent {
    ServerEvent::ParameterChanged { var_id, value }
}

fn long(var_id: u16, value: i64) -> ServerEvent {
    ServerEvent::LongLongParameterChanged { var_id, value }
}

fn gained(n: u32, var_id: u16, amount: i64) -> ServerEvent {
    ServerEvent::ExpGained {
        gid: gid(n),
        var_id,
        amount,
    }
}

fn effect(n: u32, effect: u32) -> ServerEvent {
    ServerEvent::SpecialEffect {
        gid: gid(n),
        effect,
    }
}

fn dead_bar(mob: u32, max_hp: u32) -> ServerEvent {
    ServerEvent::MobHpBar {
        gid: mob,
        hp: 0,
        max_hp,
    }
}

/// The `(base, job)` amounts of the `0x0acc`s `n` got.
fn exp_of(events: &[ScopedServerEvent], n: u32) -> (i64, i64) {
    let mut out = (0, 0);
    for event in seen_by(events, n) {
        if let ServerEvent::ExpGained { var_id, amount, .. } = event {
            match var_id {
                SP_BASEEXP => out.0 += amount,
                SP_JOBEXP => out.1 += amount,
                other => panic!("0x0acc var {other}"),
            }
        }
    }
    out
}

/// The damage `n`'s swings dealt, as one recipient saw them.
fn dealt_by(events: &[ServerEvent], n: u32) -> u64 {
    events
        .iter()
        .filter_map(|e| match e {
            ServerEvent::Damage {
                source_id, damage, ..
            } if *source_id == gid(n) => Some(u64::try_from(*damage).expect("damage")),
            _ => None,
        })
        .sum()
}

/// rAthena's share arithmetic (spec §2.9), to compare against.
fn expected_exp(db_exp: u64, damage: u64, total: u64, attackers: u32) -> i64 {
    let mut share = (damage as f64 / total as f64).min(1.0);
    if attackers > 1 {
        share += share * f64::from((attackers - 1) * 25) / 100.0;
    }
    i64::try_from(((db_exp as f64 * share) as u64).max(1)).expect("exp")
}

/// A's bare-hand hit on the Poring, and A's amotion (both fixed: no weapon, no crit).
fn bare_hit(base_level: u16) -> (u32, u64) {
    let status = CharacterStatus {
        job_level: 9,
        ..CharacterStatus::default()
    };
    let mut engine = kill_world(1_000, vec![fighter(A, (51, 50), base_level, status)]);
    let swing = swings(&seen_by(&attack(&mut engine, A, ACT_ATTACK, 1000), A));
    (
        u32::try_from(swing[0].1).expect("a hit"),
        u64::try_from(swing[0].3).expect("amotion"),
    )
}

/// A one-hit kill by A (single swing at 1000) on a 1-HP Poring with B watching from (53, 53):
/// what A and B get in the kill tick, and the engine after it.
fn one_hit_kill(
    base_level: u16,
    status: CharacterStatus,
) -> (Vec<ServerEvent>, Vec<ServerEvent>, MapInstance) {
    let mut engine = kill_world(
        1,
        vec![
            fighter(A, (51, 50), base_level, status),
            fighter(B, (53, 53), 1, CharacterStatus::default()),
        ],
    );
    let swing = swings(&seen_by(&attack(&mut engine, A, ACT_ATTACK, 1000), A));
    assert!(swing[0].1 > 0, "a hit: {swing:?}");
    let amotion = u64::try_from(swing[0].3).expect("amotion");
    let kill = tick(&mut engine, 1000 + amotion);
    (seen_by(&kill, A), seen_by(&kill, B), engine)
}

/// ENG-CMB-07: the killing hit sends `0x0977` hp 0, then the exp packets, in the hit's tick and to
/// the attacker only. The mob leaves the grid at once (new attacks on it are silent); `0x0080`
/// type 1 goes to the death cell's area at +250 ms, and the mob is gone after.
#[test]
fn eng_cmb_07_kill_and_vanish() {
    // Job 9 with an empty bar (340) and base 1 (548): no level-up.
    let status = CharacterStatus {
        job_level: 9,
        ..CharacterStatus::default()
    };
    let (a, b, mut engine) = one_hit_kill(1, status);
    let mob = mob_gid(&engine);
    assert_eq!(
        a,
        vec![
            dead_bar(mob, 1),
            long(SP_BASEEXP, 150),
            long(SP_JOBEXP, 40),
            gained(A, SP_BASEEXP, 150),
            gained(A, SP_JOBEXP, 40),
        ]
    );
    assert!(b.is_empty(), "the observer isn't in the log: {b:?}");

    let (_, amotion) = bare_hit(1);
    let dies = 1000 + amotion;
    let mut quiet = Vec::new();
    for now in dies + 1..dies + 250 {
        // B never attacked (no canact); A is past its canact by +150.
        if now == dies + 1 || now == dies + 150 {
            let n = if now == dies + 1 { B } else { A };
            engine.push_intent(intent(
                n,
                ClientIntent::RequestAction {
                    target_id: mob,
                    action_type: ACT_ATTACK_REPEAT,
                },
            ));
        }
        quiet.extend(tick(&mut engine, now));
    }
    assert!(quiet.is_empty(), "a dying mob can't be attacked: {quiet:?}");
    let vanish = tick(&mut engine, dies + 250);
    let died = ServerEvent::UnitVanished {
        gid: mob,
        vanish_type: VANISH_DIED,
    };
    assert_eq!(seen_by(&vanish, A), vec![died.clone()]);
    assert_eq!(seen_by(&vanish, B), vec![died]);
    assert!(engine.mobs().next().is_none(), "despawned, gid free");
}

/// ENG-CMB-08: exp by damage share. Solo at a level difference of -31 pays the full db exp (no
/// penalty). Two attackers share by logged damage (overkill included) with the 25 % bonus. An
/// attacker offline at death gets nothing, and the others don't get its share.
#[test]
fn eng_cmb_08_exp_shares() {
    let status = CharacterStatus {
        job_level: 9,
        ..CharacterStatus::default()
    };
    let (a, _, _) = one_hit_kill(32, status);
    assert_eq!(
        a[a.len() - 2..],
        [gained(A, SP_BASEEXP, 150), gained(A, SP_JOBEXP, 40)],
        "full exp at 32 vs 1: {a:?}"
    );

    // HP 2d + 1: A (continuous, from 1000) hits at +am and +3am; B's single swing at 3am + 10
    // lands at 4am + 10 and kills with d - 1 overkill; A's third hit (+5am) is dropped.
    let (d, am) = bare_hit(32);
    let mut engine = kill_world(
        2 * d + 1,
        vec![
            fighter(A, (51, 50), 32, status),
            fighter(B, (49, 50), 32, status),
        ],
    );
    let mob = mob_gid(&engine);
    let mut seen = seen_by(&attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000), A);
    seen.extend(run(&mut engine, A, 1000, 1000 + 3 * am + 9, 1));
    seen.extend(seen_by(
        &attack(&mut engine, B, ACT_ATTACK, 1000 + 3 * am + 10),
        A,
    ));
    seen.extend(run(
        &mut engine,
        A,
        1000 + 3 * am + 10,
        1000 + 4 * am + 9,
        1,
    ));
    let kill = tick(&mut engine, 1000 + 4 * am + 10);
    for n in [A, B] {
        assert_eq!(
            seen_by(&kill, n)[0],
            dead_bar(mob, 2 * d + 1),
            "{n}: {kill:?}"
        );
    }
    // A swung three times, but its third hit (due at +5am) came after the kill.
    assert_eq!(swings(&seen).len(), 4, "{seen:?}");
    assert_eq!(dealt_by(&seen, B), u64::from(d), "{seen:?}");
    let (a_dealt, b_dealt) = (2 * u64::from(d), u64::from(d));
    let total = a_dealt + b_dealt;
    for (n, dealt) in [(A, a_dealt), (B, b_dealt)] {
        assert_eq!(
            exp_of(&kill, n),
            (
                expected_exp(PORING_EXP.0, dealt, total, 2),
                expected_exp(PORING_EXP.1, dealt, total, 2)
            ),
            "{n}"
        );
    }

    // B hits once (at +am, before A's first hit) and logs out; A's second hit kills. The log
    // holds A 2d (overkill included) and B d: A gets 2/3 with no attacker bonus, and B's third
    // isn't handed on.
    let mut engine = kill_world(
        2 * d + 1,
        vec![
            fighter(A, (51, 50), 32, status),
            fighter(B, (49, 50), 32, status),
        ],
    );
    attack(&mut engine, B, ACT_ATTACK, 1000);
    attack(&mut engine, A, ACT_ATTACK_REPEAT, 1000);
    run(&mut engine, A, 1000, 1000 + am + 5, 1);
    engine.remove_character(char_id(B)).expect("logout");
    run(&mut engine, A, 1000 + am + 5, 1000 + 3 * am - 1, 1);
    let kill = tick(&mut engine, 1000 + 3 * am);
    assert_eq!(seen_by(&kill, A)[0], dead_bar(mob_gid(&engine), 2 * d + 1));
    let (a_dealt, total) = (2 * u64::from(d), 3 * u64::from(d));
    assert_eq!(
        exp_of(&kill, A),
        (
            expected_exp(PORING_EXP.0, a_dealt, total, 1),
            expected_exp(PORING_EXP.1, a_dealt, total, 1)
        )
    );
}

/// ENG-CMB-09: natural level-ups in the captured order (`experience.md` "Capture"): job only;
/// base and job in one kill; base only; and at the max job level.
#[test]
fn eng_cmb_09_level_ups() {
    // Job 1 -> 2 (file 686-698): the job tail inline, the diff, effect 1, then both 0x0acc. The
    // leftover 30 is clamped to the consumed bar - 1.
    let (a, b, engine) = one_hit_kill(1, CharacterStatus::default());
    let mob = mob_gid(&engine);
    assert_eq!(
        a[..6],
        [
            dead_bar(mob, 1),
            long(SP_BASEEXP, 150),
            p(SP_JOBLEVEL, 2),
            long(SP_JOBEXP, 9),
            long(SP_NEXTJOBEXP, 18),
            p(SP_SKILLPOINT, 1),
        ],
        "{a:?}"
    );
    assert_eq!(
        a[a.len() - 3..],
        [
            effect(A, 1),
            gained(A, SP_BASEEXP, 150),
            gained(A, SP_JOBEXP, 40)
        ],
        "{a:?}"
    );
    assert!(
        a[6..a.len() - 3]
            .iter()
            .any(|e| matches!(e, ServerEvent::CoupleStatus { .. })),
        "the job-bonus LUK in the diff: {a:?}"
    );
    assert!(!a.contains(&p(SP_HP, 40)), "no heal on a job level-up");
    assert_eq!(b, vec![effect(A, 1)]);

    // Base 1 -> 2 and job 4 -> 5 (file 1179-1203): the base tail inline, the one diff, the heal,
    // effect 0, then the job tail, effect 1 and both 0x0acc.
    let double = CharacterStatus {
        base_exp: 450,
        job_level: 4,
        job_exp: 27,
        skill_point: 3,
        ..CharacterStatus::default()
    };
    let (a, b, engine) = one_hit_kill(1, double);
    let base_burst = [
        dead_bar(mob, 1),
        p(SP_STATUSPOINT, 51),
        p(SP_TRAITPOINT, 0),
        p(SP_BASELEVEL, 2),
        long(SP_BASEEXP, 52),
        long(SP_NEXTBASEEXP, 894),
    ];
    assert_eq!(a[..6], base_burst, "{a:?}");
    let tail = [
        p(SP_HP, 45),
        p(SP_SP, 12),
        effect(A, 0),
        p(SP_JOBLEVEL, 5),
        long(SP_JOBEXP, 27),
        long(SP_NEXTJOBEXP, 91),
        p(SP_SKILLPOINT, 4),
        effect(A, 1),
        gained(A, SP_BASEEXP, 150),
        gained(A, SP_JOBEXP, 40),
    ];
    assert_eq!(a[a.len() - tail.len()..], tail, "{a:?}");
    assert!(a.contains(&p(6, 45)), "the diff has MAXHP 45: {a:?}");
    assert_eq!(b, vec![effect(A, 0), effect(A, 1)]);
    let progress = engine.progress_snapshot(char_id(A)).expect("A");
    assert_eq!(
        (
            progress.base_level,
            progress.status.job_level,
            progress.status.status_point,
            progress.status.skill_point,
            progress.status.base_exp,
            progress.status.job_exp,
            progress.status.hp,
        ),
        (2, 5, 51, 4, 52, 27, 45)
    );

    // Base only: the base burst, then SP_JOBEXP, then both 0x0acc (source order).
    let base_only = CharacterStatus {
        base_exp: 450,
        job_level: 9,
        ..CharacterStatus::default()
    };
    let (a, _, _) = one_hit_kill(1, base_only);
    assert_eq!(a[..6], base_burst, "{a:?}");
    assert_eq!(
        a[a.len() - 6..],
        [
            p(SP_HP, 45),
            p(SP_SP, 12),
            effect(A, 0),
            long(SP_JOBEXP, 40),
            gained(A, SP_BASEEXP, 150),
            gained(A, SP_JOBEXP, 40),
        ],
        "{a:?}"
    );

    // Novice job 10 is the max: the exp still accrues, the job 0x0acc shows 0.
    let at_max = CharacterStatus {
        job_level: 10,
        ..CharacterStatus::default()
    };
    let (a, b, engine) = one_hit_kill(1, at_max);
    assert_eq!(
        a,
        vec![
            dead_bar(mob, 1),
            long(SP_BASEEXP, 150),
            long(SP_JOBEXP, 40),
            gained(A, SP_BASEEXP, 150),
            gained(A, SP_JOBEXP, 0),
        ]
    );
    assert!(b.is_empty());
    let progress = engine.progress_snapshot(char_id(A)).expect("A");
    assert_eq!(
        (progress.status.job_level, progress.status.job_exp),
        (10, 40)
    );
}

/// ENG-CMB-10: the login burst sends the real NEXT values from the exp tables (captured fresh
/// Novice: 548 and 10).
#[test]
fn eng_cmb_10_login_sends_real_next_exp() {
    let mut engine = kill_world(1, Vec::new());
    let c = 3;
    let status = CharacterStatus {
        job_level: 9,
        ..CharacterStatus::default()
    };
    engine
        .spawn_character(fighter(c, (60, 60), 32, status), 0)
        .expect("spawn");
    let seen = seen_by(&send(&mut engine, c, ClientIntent::NotifyActorInit, 10), c);
    let next = |job_exp: bool, level: u16| {
        let jobs = jobs();
        let v = if job_exp {
            jobs.next_job_exp(NOVICE, level)
        } else {
            jobs.next_base_exp(NOVICE, level)
        };
        i64::try_from(v).expect("fits")
    };
    assert_eq!((next(false, 1), next(true, 1)), (548, 10), "captured bars");
    assert!(
        seen.contains(&long(SP_NEXTBASEEXP, next(false, 32))),
        "{seen:?}"
    );
    assert!(
        seen.contains(&long(SP_NEXTJOBEXP, next(true, 9))),
        "{seen:?}"
    );
    assert_eq!(next(true, 9), 340);
}

/// ENG-CMB-09 (final review finding 2): two mobs die in one tick and both pay A, the first with
/// a base level-up that queues the rest of its pushes. The values A ends the tick with must be
/// the server's, not the first kill's stale queued `SP_JOBEXP`.
#[test]
fn eng_cmb_09_two_kills_in_one_tick_end_on_live_values() {
    // Base 32 always hits a Poring (level 1 can miss).
    let (d, am) = bare_hit(32);
    // A is one exp short of base 33, at job 9 (bar 340: no job level-up).
    let a_status = CharacterStatus {
        base_exp: jobs().next_base_exp(NOVICE, 32) - 1,
        job_level: 9,
        ..CharacterStatus::default()
    };
    let job9 = CharacterStatus {
        job_level: 9,
        ..CharacterStatus::default()
    };
    let (m1, m2) = ((50, 50), (50, 52));
    let mut engine = kill_world_at(
        d + 1,
        vec![
            fighter(A, (51, 51), 32, a_status),
            // The same level and job, so their amotion matches A's.
            fighter(B, (49, 49), 32, job9),
            fighter(3, (49, 53), 32, job9),
        ],
        &[m1, m2],
    );
    let gids: Vec<u32> = engine.mobs().map(|m| m.gid).collect();
    let act = |target_id| ClientIntent::RequestAction {
        target_id,
        action_type: ACT_ATTACK,
    };
    // A leaves each Poring at 1 HP: mob 1 at +am, mob 2 at +3am.
    send(&mut engine, A, act(gids[0]), 1000);
    tick(&mut engine, 1000 + am);
    send(&mut engine, A, act(gids[1]), 1000 + 2 * am);
    // B and C finish both off with hits landing in the same tick, 1000 + 4am.
    engine.push_intent(intent(B, act(gids[0])));
    engine.push_intent(intent(3, act(gids[1])));
    tick(&mut engine, 1000 + 3 * am);
    let kills = seen_by(&tick(&mut engine, 1000 + 4 * am), A);
    let bars = kills
        .iter()
        .filter(|e| matches!(e, ServerEvent::MobHpBar { hp: 0, .. }))
        .count();
    assert_eq!(bars, 2, "both died in this tick: {kills:?}");
    let progress = engine.progress_snapshot(char_id(A)).expect("A");
    assert_eq!(progress.base_level, 33, "the first kill levelled base");
    let last = |var: u16| {
        kills.iter().rev().find_map(|e| match e {
            ServerEvent::LongLongParameterChanged { var_id, value } if *var_id == var => {
                Some(*value)
            }
            _ => None,
        })
    };
    let wire = |v: u64| Some(i64::try_from(v).expect("fits"));
    assert_eq!(
        last(SP_BASEEXP),
        wire(progress.status.base_exp),
        "{kills:?}"
    );
    assert_eq!(last(SP_JOBEXP), wire(progress.status.job_exp), "{kills:?}");
}
