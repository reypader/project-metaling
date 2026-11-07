//! Player auto-attack in a map instance (spec `specs/combat/spec.md` ENG-CMB-01..10). Expected
//! packet orders are the captured ones (catalog `mechanics/auto_attack.md` §8, `combat.md` §7).

use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharacterInventory, CharacterSpawn, CharacterStatus, ClientIntent,
    InventoryItem, ItemId, MapPosition, ScopedServerEvent, ServerEvent, SessionId, SessionIntent,
    Sex,
};
use ro_db::{
    AmmoType, CellType, ItemData, ItemDb, ItemType, MapGrid, MapMobSpawns, MapNpcs, MobData, MobDb,
    MobId, MobSpawn, NpcKind, NpcSpawn, NpcState, WarpDest, WeaponType,
};
use ro_engine::{MapInstance, Registries};

use crate::common::{
    char_id, gid, item_data, jobs, long, p, A, ARROW, B, BOW, EQP_AMMO, EQP_ARMS, EQP_HAND_R,
    KNIFE, MOB, MOB_DMOTION, NOVICE, SP_BASEEXP, SP_BASELEVEL, SP_HP, SP_JOBEXP, SP_JOBLEVEL,
    SP_NEXTBASEEXP, SP_NEXTJOBEXP, SP_SKILLPOINT, SP_SP, SP_STATUSPOINT, SP_TRAITPOINT, SP_WEIGHT,
};

mod attack_loop;
mod kill;

const REVOLVER: u32 = 13100;

const ACT_ATTACK: u8 = 0;
const ACT_ATTACK_REPEAT: u8 = 7;
const ACT_SIT: u8 = 2;

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

const VANISH_DIED: u8 = 1;
/// Poring 1002's db exp (`combat_sequence_1.txt`).
const PORING_EXP: (u64, u64) = (150, 40);

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
