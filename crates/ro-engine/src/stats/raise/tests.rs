use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterSpawn, ClientIntent, MapPosition,
    ScopedServerEvent, SessionId, SessionIntent, Sex,
};
use ro_db::MapGrid;
use ro_mechanics::BattleConfig;

use super::*;
use crate::map_instance::Registries;
use crate::protocol_consts::DEAD;

const CHAR: CharId = CharId(150_001);

/// A Novice with no job data (so the battle config alone sets the cap) and `status`.
fn novice(battle: BattleConfig, status: CharacterStatus) -> (MapInstance, Entity) {
    player(0, battle, status)
}

/// A `job` player with no job data, as `novice`.
fn player(job: u16, battle: BattleConfig, status: CharacterStatus) -> (MapInstance, Entity) {
    let grid = Arc::new(MapGrid::all_walkable(50, 50).expect("dims"));
    let mut engine = MapInstance::new("prontera.gat", grid);
    engine.set_registries(Registries {
        battle: Arc::new(battle),
        ..Registries::default()
    });
    let spawn = CharacterSpawn {
        account_id: AccountId(2_000_001),
        char_id: CHAR,
        name: "C1".into(),
        sex: Sex::Male,
        job_class: job,
        base_level: 1,
        hair_style: 1,
        hair_color: 1,
        pos: MapPosition {
            x: 10,
            y: 10,
            dir: 0,
        },
        speed: 150,
        status,
        group_id: 0,
        inventory: Default::default(),
        skills: Vec::new(),
        hidden_skills: Vec::new(),
        change_level: [0; 3],
    };
    engine.spawn_character(spawn, 0).expect("spawn");
    send(&mut engine, &[ClientIntent::NotifyActorInit]);
    let e = engine.world.entity_by_char(CHAR).expect("entity");
    (engine, e)
}

/// One tick with `intents`; what the player got.
fn send(engine: &mut MapInstance, intents: &[ClientIntent]) -> Vec<ServerEvent> {
    for intent in intents {
        engine.push_intent(SessionIntent::new(
            SessionId(1),
            Some(AccountId(2_000_001)),
            Some(CHAR),
            intent.clone(),
        ));
    }
    let mut out: Vec<ScopedServerEvent> = Vec::new();
    engine.step_tick(100, &mut out).expect("tick");
    out.into_iter()
        .filter(|e| e.scope == BroadcastScope::SelfOnly(CHAR))
        .map(|e| e.event)
        .collect()
}

fn raise(engine: &mut MapInstance, var: u16, amount: u8) -> Vec<ServerEvent> {
    send(engine, &[ClientIntent::RaiseStat { var, amount }])
}

/// Spec §2.4: refused while dead (our ruling), with nothing changed.
#[test]
fn eng_stat_05_refused_while_dead() {
    let (mut engine, e) = novice(BattleConfig::default(), CharacterStatus::default());
    engine.world.dead_sit[e.index()] = DEAD;
    let out = raise(&mut engine, SP_STR, 1);
    assert_eq!(
        out,
        vec![ServerEvent::StatusChangeAck {
            var_id: SP_STR,
            ok: false,
            value: 0
        }]
    );
    let status = engine.world.players.get(e).expect("player").status;
    assert_eq!((status.str, status.status_point), (1, 48));
}

/// Spec §2.4: a stat above 255 acks 255, then its real value follows as `0x0141` from the
/// committed snapshot (only reachable with a raised `max_parameter`).
#[test]
fn eng_stat_05_a_stat_above_255_gets_a_trailing_couple() {
    let battle = BattleConfig {
        max_parameter: 300,
        ..BattleConfig::default()
    };
    let status = CharacterStatus {
        str: 255,
        status_point: 1000,
        ..CharacterStatus::default()
    };
    let (mut engine, _) = novice(battle, status);
    let out = raise(&mut engine, SP_STR, 2);
    assert_eq!(
        out[out.len() - 2..],
        [
            ServerEvent::StatusChangeAck {
                var_id: SP_STR,
                ok: true,
                value: 255
            },
            ServerEvent::CoupleStatus {
                var_id: SP_STR.into(),
                base: 257,
                bonus: 0
            },
        ],
        "{out:?}"
    );
}

/// Spec §2.4: two raises in one tick share one recalc; each one's follow-ups carry its own
/// values.
#[test]
fn eng_stat_05_two_raises_in_one_tick() {
    let (mut engine, _) = novice(BattleConfig::default(), CharacterStatus::default());
    let up = ClientIntent::RaiseStat {
        var: SP_STR,
        amount: 1,
    };
    let out = send(&mut engine, &[up.clone(), up]);
    let couples = out
        .iter()
        .filter(|ev| matches!(ev, ServerEvent::CoupleStatus { .. }))
        .count();
    assert_eq!(couples, 1, "one diff: {out:?}");
    let acks: Vec<_> = out
        .iter()
        .filter_map(|ev| match ev {
            ServerEvent::StatusChangeAck { value, .. } => Some(*value),
            ServerEvent::ParameterChanged {
                var_id: SP_STATUSPOINT,
                value,
            } => u8::try_from(*value).ok(),
            _ => None,
        })
        .collect();
    assert_eq!(acks, vec![46, 2, 44, 3], "points then ack, per raise");
}

/// A Dragon Knight: a trait job, so the cap is `max_trait_parameter`.
const DRAGON_KNIGHT: u16 = 4252;

fn raise_trait(engine: &mut MapInstance, var: u16, amount: i16) -> Vec<ServerEvent> {
    send(engine, &[ClientIntent::RaiseTrait { var, amount }])
}

/// Traits spec §2.4 (ENG-TRAIT-02): refused while dead, as stat-up, with nothing changed;
/// the same request alive succeeds.
#[test]
fn eng_trait_02_refused_while_dead() {
    let status = CharacterStatus {
        trait_point: 5,
        ..CharacterStatus::default()
    };
    let (mut engine, e) = player(DRAGON_KNIGHT, BattleConfig::default(), status);
    engine.world.dead_sit[e.index()] = DEAD;
    let out = raise_trait(&mut engine, SP_POW, 1);
    assert_eq!(
        out,
        vec![ServerEvent::StatusChangeAck {
            var_id: SP_POW,
            ok: false,
            value: 0
        }]
    );
    let status = engine.world.players.get(e).expect("player").status;
    assert_eq!((status.traits[0], status.trait_point), (0, 5));

    engine.world.dead_sit[e.index()] = 0;
    let out = raise_trait(&mut engine, SP_POW, 1);
    assert_eq!(
        out.last(),
        Some(&ServerEvent::StatusChangeAck {
            var_id: SP_POW,
            ok: true,
            value: 1
        })
    );
}

/// Traits spec §2.4: a trait above 255 (only with a raised `max_trait_parameter`) acks 255,
/// then its real value follows as the trait's `0x0141`.
#[test]
fn eng_trait_01_a_trait_above_255_gets_a_trailing_couple() {
    let battle = BattleConfig {
        max_trait_parameter: 300,
        ..BattleConfig::default()
    };
    let mut traits = [0; 6];
    traits[4] = 255;
    let status = CharacterStatus {
        traits,
        trait_point: 10,
        ..CharacterStatus::default()
    };
    let (mut engine, _) = player(DRAGON_KNIGHT, battle, status);
    let con = SP_POW + 4;
    let out = raise_trait(&mut engine, con, 2);
    assert_eq!(
        out[out.len() - 2..],
        [
            ServerEvent::StatusChangeAck {
                var_id: con,
                ok: true,
                value: 255
            },
            ServerEvent::CoupleStatus {
                var_id: con.into(),
                base: 257,
                bonus: 0
            },
        ],
        "{out:?}"
    );
}
