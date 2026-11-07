//! Trait stats and AP in the engine (spec `specs/traits/spec.md` ENG-TRAIT-01..08), with the
//! shipped job data read from `crates/ro-db/data/job/`. The orders are the captured ones
//! (`reference/catalog/captures/rathena_traits_sequence.txt`; file line = proxy # + 10).

use std::sync::Arc;

use ro_core::{
    AccountId, CharacterInventory, CharacterSpawn, CharacterStatus, ClientIntent, MapPosition,
    ServerEvent, SessionId, SessionIntent, Sex,
};
use ro_db::MapGrid;
use ro_engine::{MapInstance, Registries};

use crate::common::{
    ack, char_id, gid, index_of, is_message, jobs, p, seen_by, A, B, HYPER_NOVICE, KNIGHT, SP_ASPD,
    SP_ATK1, SP_BASEEXP, SP_BASELEVEL, SP_HP, SP_JOBLEVEL, SP_NEXTBASEEXP, SP_SP, SP_STATUSPOINT,
    SP_TRAITPOINT, SP_UPOW, SP_USTR, SP_WEIGHT,
};

mod points;
mod raise;
mod status;

const DRAGON_KNIGHT: u16 = 4252;
const NOVICE_HIGH: u16 = 4001;
const SUPER_NOVICE: u16 = 23;
const RUNE_KNIGHT_T: u16 = 4060;
const MEISTER: u16 = 4253;

const SP_HIT: u16 = 49;
const SP_FLEE1: u16 = 50;
const SP_POW: u32 = 219;
const SP_CON: u16 = 223;
const SP_PATK: u16 = 225;
const SP_RES: u16 = 227;
const SP_AP: u16 = 232;
const SP_MAXAP: u16 = 233;
/// The next-point cost of a stat at 1.
const STAT_COST_AT_1: u8 = 2;

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

fn is_trait_value(event: &ServerEvent) -> bool {
    match event {
        ServerEvent::CoupleStatus { var_id, .. } => (219..=224).contains(var_id),
        ServerEvent::ParameterChanged { var_id, .. } => (225..=233).contains(var_id),
        _ => false,
    }
}

fn raise(world: &mut World, var: u16, amount: i16) -> Vec<ServerEvent> {
    world.tick(&[(A, ClientIntent::RaiseTrait { var, amount })])
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
fn base_tail(job: u16, status_point: i32, trait_point: i32, level: u16) -> Vec<ServerEvent> {
    let next = i64::try_from(jobs().next_base_exp(job, level)).expect("fits");
    vec![
        p(SP_STATUSPOINT, status_point),
        p(SP_TRAITPOINT, trait_point),
        p(SP_BASELEVEL, level.into()),
        ServerEvent::LongLongParameterChanged {
            var_id: SP_BASEEXP,
            value: 0,
        },
        ServerEvent::LongLongParameterChanged {
            var_id: SP_NEXTBASEEXP,
            value: next,
        },
    ]
}

/// `SP_TRAITPOINT` right before `SP_BASELEVEL` `level` in a base tail.
fn tail_trait_point(out: &[ServerEvent], level: i32) -> ServerEvent {
    let at = index_of(out, &p(SP_BASELEVEL, level));
    out[at - 1].clone()
}

fn table_points(level: u16) -> i32 {
    i32::try_from(jobs().status_points(level)).expect("small")
}
