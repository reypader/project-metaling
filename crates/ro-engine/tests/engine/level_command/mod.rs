//! `@blvl`, `@jlvl` and `@job` (spec `specs/stats/spec.md` §2.5-§2.6, ENG-STAT-06..08), with
//! the shipped job data read from `crates/ro-db/data/job/` and hand-written items. The orders
//! are the captured ones (catalog `packets/status.md` "Captured stat-up, level and job
//! sequences", "Capture 2", "Capture 3").

use std::sync::Arc;

use ro_core::{
    AccountId, CharId, CharacterInventory, CharacterSpawn, CharacterStatus, ClientIntent,
    InventoryItem, ItemId, MapPosition, ScopedServerEvent, ServerEvent, SessionId, SessionIntent,
    Sex,
};
use ro_db::{ItemData, ItemDb, ItemType, MapGrid, WeaponType};
use ro_engine::{MapInstance, Registries};

use crate::common::{
    char_id, gid, index_of, is_message, item_data, jobs, long, look, name, p, seen_by, A, B, BARD,
    COTTON_SHIRT, EQP_ARMOR, EQP_HAND_R, EQP_HEAD_TOP, EQP_SHOES, HYPER_NOVICE, KNIFE,
    LOOK_HEAD_BOTTOM, LOOK_HEAD_MID, LOOK_HEAD_TOP, LOOK_ROBE, LOOK_SHOES, LOOK_WEAPON, NOVICE,
    SP_BASEEXP, SP_BASELEVEL, SP_HP, SP_JOBEXP, SP_JOBLEVEL, SP_MAXHP, SP_MAXSP, SP_MAXWEIGHT,
    SP_NEXTBASEEXP, SP_NEXTJOBEXP, SP_SKILLPOINT, SP_SP, SP_STATUSPOINT, SP_TRAITPOINT, SP_UPOW,
    SP_USTR, SWORDMAN,
};

mod base_level;
mod job;

/// A dagger only a Novice may use.
const NOVICE_KNIFE: u32 = 13040;
/// Beginner Mark: head top, View 1348, `EquipLevelMax` 99 (capture 3).
const MARK: u32 = 19256;
/// Mana Boots: shoes, `EquipLevelMax` 100 (capture 3).
const BOOTS: u32 = 22012;

const DANCER: u16 = 20;

const SP_STR: u32 = 13;

const LOOK_BASE: u8 = 0;
const LOOK_HAIR: u8 = 1;
const LOOK_CLOTHES_COLOR: u8 = 7;
const LOOK_BODY2: u8 = 13;

/// The exp bars the pushes carry, from the shipped tables (combat spec ENG-CMB-10).
fn next_base(job: u16, level: u16) -> i64 {
    i64::try_from(jobs().next_base_exp(job, level)).expect("fits")
}

fn next_job(job: u16, level: u16) -> i64 {
    i64::try_from(jobs().next_job_exp(job, level)).expect("fits")
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

fn effect(effect: u32) -> ServerEvent {
    ServerEvent::SpecialEffect {
        gid: gid(A),
        effect,
    }
}

fn message_text(event: &ServerEvent) -> String {
    match event {
        ServerEvent::SelfChat { text } => String::from_utf8_lossy(text).into_owned(),
        other => panic!("not a message: {other:?}"),
    }
}

fn base_tail(job: u16, status_point: u32, level: u16) -> Vec<ServerEvent> {
    vec![
        p(SP_STATUSPOINT, status_point as i32),
        p(SP_TRAITPOINT, 0),
        p(SP_BASELEVEL, level.into()),
        long(SP_BASEEXP, 0),
        long(SP_NEXTBASEEXP, next_base(job, level)),
    ]
}

fn full(status: CharacterStatus) -> CharacterStatus {
    CharacterStatus {
        hp: 1_000_000,
        sp: 1_000_000,
        ..status
    }
}
