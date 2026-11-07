//! Skill casts, potion use and their effects (spec `specs/effects/spec.md` §2.2-§2.7, §2.9;
//! ENG-EFF-01..17), with the shipped job and skill data from `crates/ro-db/data/`, hand-written
//! mobs and items, and the built-in definitions and Rust hooks. Expected orders are the captured
//! ones (catalog `scripting/skills.md` §14-§15, `mechanics/inventory.md` §7.5).

use std::path::Path;
use std::sync::{Arc, OnceLock};

use ro_core::{
    AccountId, BroadcastScope, CharacterInventory, CharacterSpawn, CharacterStatus, ClientIntent,
    InventoryItem, ItemId, LearnedSkill, MapPosition, ScopedServerEvent, ServerEvent, SessionId,
    SessionIntent, Sex,
};
use ro_db::{
    Element, Gender, ItemData, ItemDb, ItemType, ItemUse, JobDb, MapGrid, MapMobSpawns, MobData,
    MobDb, MobId, MobSpawn, SkillDb, SkillTreeDb, WeaponType,
};
use ro_engine::{MapInstance, Registries, SkillScripts};

use crate::common::{
    char_id, gid, learned, param, A, B, BOW, EQP_ARMS, MOB, MOB_DMOTION, NOVICE, NV_BASIC, PORING,
    SM_BASH, SP_ASPD, SP_HP, SP_SP, SP_WEIGHT, SWORDMAN,
};

mod attack;
mod cast;
mod heal;
mod item_use;

const ACOLYTE: u16 = 4;
const MERCHANT: u16 = 5;

const AL_HEAL: u16 = 28;
const MC_MAMMONITE: u16 = 42;
const NV_FIRSTAID: u16 = 142;

const SP_ZENY: u16 = 20;
const SP_INT: u32 = 16;
const SP_DEX: u32 = 17;
const SP_LUK: u32 = 18;

/// An etc item heavy enough to pass 90 % of any test character's max weight.
const ANVIL: u32 = 7000;
// Hand-written usable items (ENG-EFF-13..15).
const HP_POTION: u32 = 9001;
const SP_POTION: u32 = 9002;
const PERCENT_POTION: u32 = 9003;
/// Healing, but no lifted `Use`.
const NO_USE: u32 = 9004;
/// An `Etc` item that names a heal anyway: the type gate refuses it.
const NOT_USABLE: u32 = 9005;
/// Base level 120 and up.
const LEVEL_LOCKED: u32 = 9006;
/// Female only (A is male).
const GENDER_LOCKED: u32 = 9007;
/// No base job may use it.
const JOB_LOCKED: u32 = 9008;
/// No class (normal, upper, baby, ...) may use it.
const CLASS_LOCKED: u32 = 9009;
const SP_VIT: u32 = 15;

const NEXT_TO_MOB: (u16, u16) = (51, 50);

const ZOMBIE: u32 = 1015;

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

/// The shipped skills with Bash given a 1000 ms cast time (ENG-EFF-12).
fn skills_with_bash_cast_time() -> (Arc<SkillDb>, Arc<SkillTreeDb>) {
    let (shipped, _) = shipped_skills();
    let rows = shipped
        .iter()
        .cloned()
        .map(|mut skill| {
            if skill.id == SM_BASH {
                skill.cast_time = vec![1000; usize::from(skill.max_level)];
            }
            skill
        })
        .collect();
    let skills = SkillDb::from_rows("cast-time", rows).expect("skills");
    let trees = SkillTreeDb::load_dir(&data_dir("skill"), &skills).expect("trees");
    (Arc::new(skills), Arc::new(trees))
}

fn items() -> Arc<ItemDb> {
    let bow = ItemData {
        id: ItemId(BOW),
        aegis_name: "Bow".into(),
        name: "Bow".into(),
        item_type: ItemType::Weapon,
        weight: 50,
        locations: EQP_ARMS,
        weapon_type: WeaponType::Bow,
        attack: 15,
        range: 5,
        weapon_level: 1,
        ..ItemData::default()
    };
    let anvil = ItemData {
        id: ItemId(ANVIL),
        aegis_name: "Anvil".into(),
        name: "Anvil".into(),
        item_type: ItemType::Etc,
        weight: 60_000,
        ..ItemData::default()
    };
    // Our own potions (constant ranges, so the heal is exact).
    let potion = |id: u32, item_type: ItemType, weight: u32, on_use: Option<ItemUse>| ItemData {
        id: ItemId(id),
        aegis_name: format!("Potion{id}"),
        name: format!("Potion {id}"),
        item_type,
        weight,
        on_use,
        ..ItemData::default()
    };
    let item_heal = |hp: u32, sp: u32| {
        Some(ItemUse::ItemHeal {
            hp: (hp, hp),
            sp: (sp, sp),
        })
    };
    let mut locked = potion(LEVEL_LOCKED, ItemType::Healing, 10, item_heal(40, 0));
    locked.equip_level_min = 120;
    let mut female = potion(GENDER_LOCKED, ItemType::Healing, 10, item_heal(40, 0));
    female.gender = Gender::Female;
    let mut no_job = potion(JOB_LOCKED, ItemType::Healing, 10, item_heal(40, 0));
    no_job.jobs = [0; 3];
    let mut no_class = potion(CLASS_LOCKED, ItemType::Healing, 10, item_heal(40, 0));
    no_class.classes = 0;
    Arc::new(
        ItemDb::from_items(
            "test",
            vec![
                bow,
                anvil,
                potion(HP_POTION, ItemType::Healing, 70, item_heal(40, 0)),
                potion(SP_POTION, ItemType::Healing, 150, item_heal(0, 20)),
                potion(
                    PERCENT_POTION,
                    ItemType::Healing,
                    10,
                    Some(ItemUse::PercentHeal { hp: 10, sp: 0 }),
                ),
                potion(NO_USE, ItemType::Healing, 10, None),
                potion(NOT_USABLE, ItemType::Etc, 10, item_heal(40, 0)),
                locked,
                female,
                no_job,
                no_class,
            ],
        )
        .expect("items"),
    )
}

/// Hand-written mobs: a sturdy Poring (`hp`, `agi` vary per test) and an Undead Zombie.
fn mobs(hp: u32, agi: u16) -> Arc<MobDb> {
    let mob = |id: u32, name: &str, element: Element| MobData {
        id: MobId(id),
        aegis_name: name.to_ascii_uppercase(),
        name: name.into(),
        level: 1,
        hp,
        agi,
        vit: 1,
        defense: 2,
        element,
        element_level: 1,
        attack_motion: 672,
        attack_delay: 1872,
        damage_motion: 480,
        walk_speed: 400,
        base_exp: 2,
        job_exp: 1,
        ..MobData::default()
    };
    Arc::new(
        MobDb::from_mobs(
            "test",
            vec![
                mob(PORING, "Poring", Element::Water),
                mob(ZOMBIE, "Zombie", Element::Undead),
            ],
        )
        .expect("mobs"),
    )
}

fn name(n: u32) -> String {
    format!("Caster{n}")
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

/// A player to spawn.
struct Player {
    n: u32,
    job: u16,
    at: (u16, u16),
    status: CharacterStatus,
    skills: Vec<LearnedSkill>,
    inventory: Vec<Option<InventoryItem>>,
}

impl Player {
    fn new(n: u32, job: u16, at: (u16, u16), skills: &[(u16, u8)]) -> Self {
        Self {
            n,
            job,
            at,
            status: CharacterStatus {
                // Clamped to the computed max at spawn.
                sp: 9999,
                hp: 9999,
                ..CharacterStatus::default()
            },
            skills: learned(skills),
            inventory: Vec::new(),
        }
    }

    fn status(mut self, f: impl FnOnce(&mut CharacterStatus)) -> Self {
        f(&mut self.status);
        self
    }
}

struct World {
    engine: MapInstance,
    now: u64,
    /// What each player got when it entered, by number.
    login: Vec<(u32, Vec<ServerEvent>)>,
}

impl World {
    /// Players (A first), and one mob of `mob_id` on `MOB`.
    fn new(players: Vec<Player>, mob_id: u32, mob_db: Arc<MobDb>) -> Self {
        Self::with_skills(players, mob_id, mob_db, shipped_skills())
    }

    fn with_skills(
        players: Vec<Player>,
        mob_id: u32,
        mob_db: Arc<MobDb>,
        (skills, trees): (Arc<SkillDb>, Arc<SkillTreeDb>),
    ) -> Self {
        let grid = Arc::new(MapGrid::all_walkable(100, 100).expect("dims"));
        let mut engine = MapInstance::new("prt_fild08.gat", grid);
        engine.set_registries(Registries {
            items: items(),
            mobs: mob_db,
            jobs: jobs(),
            skills,
            skill_tree: trees,
            scripts: SkillScripts::builtin().expect("builtin scripts"),
            ..Registries::default()
        });
        let spawns = vec![MobSpawn {
            name: "Mob".into(),
            mob_id: MobId(mob_id),
            x: MOB.0,
            y: MOB.1,
            span: (0, 0),
            amount: 1,
            delay1: 5000,
            delay2: 0,
            level: None,
            boss: false,
            event: None,
            source: "test:mob".into(),
        }];
        assert_eq!(engine.spawn_mobs(&MapMobSpawns { spawns }), 1);
        let mut world = Self {
            engine,
            now: 1000,
            login: Vec::new(),
        };
        for p in players {
            let spawn = CharacterSpawn {
                account_id: AccountId(gid(p.n)),
                char_id: char_id(p.n),
                name: name(p.n),
                sex: Sex::Male,
                job_class: p.job,
                base_level: 99,
                hair_style: 3,
                hair_color: 1,
                pos: MapPosition {
                    x: p.at.0,
                    y: p.at.1,
                    dir: 0,
                },
                speed: 150,
                status: p.status,
                group_id: 99,
                inventory: CharacterInventory {
                    slots: p.inventory,
                    unique_item_counter: 0,
                },
                skills: p.skills,
                hidden_skills: Vec::new(),
                change_level: [0; 3],
            };
            world.engine.spawn_character(spawn, 0).expect("spawn");
            let out = world.at(world.now, &[(p.n, ClientIntent::NotifyActorInit)]);
            world.login.push((p.n, seen_by(&out, p.n)));
        }
        world.now += 1000;
        world
    }

    /// One tick at `now` with `intents`.
    fn at(&mut self, now: u64, intents: &[(u32, ClientIntent)]) -> Vec<ScopedServerEvent> {
        for (n, intent) in intents {
            self.engine.push_intent(SessionIntent::new(
                SessionId(u64::from(*n)),
                Some(AccountId(gid(*n))),
                Some(char_id(*n)),
                intent.clone(),
            ));
        }
        self.now = now;
        let mut out = Vec::new();
        self.engine.step_tick(now, &mut out).expect("tick");
        out
    }

    /// `n` casts `skill` at `level` on `target` now.
    fn cast(&mut self, n: u32, skill_id: u16, level: u16, target: u32) -> Vec<ScopedServerEvent> {
        let now = self.now;
        self.at(
            now,
            &[(
                n,
                ClientIntent::UseSkill {
                    level,
                    skill_id,
                    target,
                },
            )],
        )
    }

    /// Ticks every 10 ms over `(now, to]`, collecting what `n` sees.
    fn run_to(&mut self, n: u32, to: u64) -> Vec<ServerEvent> {
        let mut seen = Vec::new();
        while self.now + 10 <= to {
            let now = self.now + 10;
            seen.extend(seen_by(&self.at(now, &[]), n));
        }
        seen
    }

    fn mob_gid(&self) -> u32 {
        self.engine.mobs().next().expect("mob").gid
    }

    fn mob_hp(&self) -> u32 {
        self.engine.mobs().next().expect("mob").hp
    }

    fn status(&self, n: u32) -> CharacterStatus {
        self.engine
            .progress_snapshot(char_id(n))
            .expect("player")
            .status
    }

    /// The `SP_ASPD` (amotion) `n` got at login.
    fn amotion(&self, n: u32) -> u64 {
        let login = &self.login.iter().find(|(m, _)| *m == n).expect("login").1;
        login
            .iter()
            .rev()
            .find_map(|e| match e {
                ServerEvent::ParameterChanged { var_id, value } if *var_id == SP_ASPD => {
                    Some(u64::try_from(*value).expect("amotion"))
                }
                _ => None,
            })
            .expect("SP_ASPD at login")
    }

    /// The battle value (`base + bonus`) of stat `var` that `n` got at login.
    fn battle_stat(&self, n: u32, var: u32) -> i32 {
        let login = &self.login.iter().find(|(m, _)| *m == n).expect("login").1;
        login
            .iter()
            .rev()
            .find_map(|e| match e {
                ServerEvent::CoupleStatus {
                    var_id,
                    base,
                    bonus,
                } if *var_id == var => Some(base + bonus),
                _ => None,
            })
            .expect("0x0141 at login")
    }
}

fn casting(src: u32, dst: u32, skill_id: u16, element: u32, delay_ms: u32) -> ServerEvent {
    ServerEvent::SkillCasting {
        src,
        dst,
        x: 0,
        y: 0,
        skill_id,
        element,
        delay_ms,
    }
}

fn no_damage(skill_id: u16, value: i32, target: u32, src: u32) -> ServerEvent {
    ServerEvent::SkillNoDamage {
        skill_id,
        value,
        target,
        src,
        ok: true,
    }
}

fn postdelay(caster: u32, ms: u32) -> ServerEvent {
    ServerEvent::StatusIconStarted {
        efst: 46,
        gid: caster,
        state: 1,
        total_ms: ms,
        remain_ms: ms as i32,
        values: [0; 3],
    }
}

fn fail(skill_id: u16, cause: u8) -> ServerEvent {
    ServerEvent::SkillFail { skill_id, cause }
}

/// A Novice with First Aid at `hp`, next to an observer B.
fn first_aid_world(hp: u32) -> World {
    World::new(
        vec![
            Player::new(A, NOVICE, (40, 40), &[(NV_FIRSTAID, 1)]).status(|s| s.hp = hp),
            Player::new(B, NOVICE, (42, 40), &[]),
        ],
        PORING,
        mobs(1000, 1),
    )
}

/// A Swordman with Bash 10 next to a Poring with `hp`, B watching.
fn bash_world(hp: u32, agi: u16) -> World {
    World::new(
        vec![
            Player::new(A, SWORDMAN, NEXT_TO_MOB, &[(SM_BASH, 10)]),
            Player::new(B, NOVICE, (53, 53), &[]),
        ],
        PORING,
        mobs(hp, agi),
    )
}

/// The `0x01de` in `events`.
fn skill_damage(events: &[ServerEvent]) -> Option<ServerEvent> {
    events
        .iter()
        .find(|e| matches!(e, ServerEvent::SkillDamage { .. }))
        .cloned()
}

/// An Acolyte with Heal 10 (A, at `hp`) and a Novice B at `b_hp`, beside a mob of `mob_id`.
fn heal_world(hp: u32, b_hp: u32, mob_id: u32) -> World {
    World::new(
        vec![
            Player::new(A, ACOLYTE, NEXT_TO_MOB, &[(AL_HEAL, 10)]).status(|s| s.hp = hp),
            Player::new(B, NOVICE, (52, 50), &[]).status(|s| s.hp = b_hp),
        ],
        mob_id,
        mobs(1000, 1),
    )
}

/// Heal's amount for `n` at `level`, from its login stats (catalog `scripting/skills.md` §7).
fn heal_amount(w: &World, n: u32, level: i32) -> i32 {
    let (int, dex, luk) = (
        w.battle_stat(n, SP_INT),
        w.battle_stat(n, SP_DEX),
        w.battle_stat(n, SP_LUK),
    );
    let base_level = 99;
    let matk = int + int / 2 + dex / 5 + luk / 3 + base_level / 4;
    (base_level + int) / 5 * 30 * level / 10 + matk
}

fn use_item(index: u16) -> ClientIntent {
    ClientIntent::UseItem { index }
}

fn item_ack(index: u16, item: u32, n: u32, amount: i16, ok: bool) -> ServerEvent {
    ServerEvent::ItemUseAck {
        index,
        item_id: item,
        gid: gid(n),
        amount,
        ok,
    }
}

/// The `(item, amount)` in `n`'s inventory `slot`.
fn stack(w: &World, n: u32, slot: usize) -> Option<(u32, u16)> {
    let snapshot = w.engine.play_snapshot(char_id(n)).expect("player");
    snapshot
        .inventory
        .slots
        .get(slot)
        .and_then(Option::as_ref)
        .map(|item| (item.item_id.0, item.amount))
}

/// A Novice at `hp`/`sp` next to the mob, holding `items` from slot 0 (wire index 2); B observes.
fn potion_world(hp: u32, sp: u32, items: &[(u32, u16)]) -> World {
    let mut a = Player::new(A, NOVICE, NEXT_TO_MOB, &[]).status(|s| {
        s.hp = hp;
        s.sp = sp;
    });
    a.inventory = items
        .iter()
        .map(|&(id, amount)| Some(InventoryItem::new(ItemId(id), amount)))
        .collect();
    World::new(
        vec![a, Player::new(B, NOVICE, (52, 52), &[])],
        PORING,
        mobs(1_000_000, 1),
    )
}

impl World {
    /// `n` uses the item at wire `index` at `now`; what `n` and the observer B see.
    fn use_at(&mut self, n: u32, index: u16, now: u64) -> (Vec<ServerEvent>, Vec<ServerEvent>) {
        let out = self.at(now, &[(n, use_item(index))]);
        (seen_by(&out, n), seen_by(&out, B))
    }
}

/// `itemheal`'s scaling (spec §2.7) of a constant `value` at battle stat `stat`, no skills.
fn potion_heal(value: i32, stat: i32) -> i32 {
    let bonus = 100 + 2 * stat;
    let boosted = value * bonus / 100;
    if bonus != 100 && boosted > value {
        boosted
    } else {
        value
    }
}
