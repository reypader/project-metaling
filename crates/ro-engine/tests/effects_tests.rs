//! Skill casts, potion use and their effects (spec `specs/effects/spec.md` §2.2-§2.7, §2.9;
//! ENG-EFF-01..17), with the shipped job and skill data from `crates/ro-db/data/`, hand-written
//! mobs and items, and the built-in definitions and Rust hooks. Expected orders are the captured
//! ones (catalog `scripting/skills.md` §14-§15, `mechanics/inventory.md` §7.5).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::{Arc, OnceLock};

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterInventory, CharacterSpawn, CharacterStatus,
    ClientIntent, InventoryItem, ItemId, LearnedSkill, MapPosition, ScopedServerEvent, ServerEvent,
    SessionId, SessionIntent, Sex, SkillFlag,
};
use ro_db::{
    Element, Gender, ItemData, ItemDb, ItemType, ItemUse, JobDb, MapGrid, MapMobSpawns, MobData,
    MobDb, MobId, MobSpawn, SkillDb, SkillTreeDb, WeaponType,
};
use ro_engine::{MapInstance, Registries, SkillScripts};

/// The caster and an observer.
const A: u32 = 1;
const B: u32 = 2;

const NOVICE: u16 = 0;
const SWORDMAN: u16 = 1;
const ACOLYTE: u16 = 4;
const MERCHANT: u16 = 5;

const NV_BASIC: u16 = 1;
const SM_BASH: u16 = 5;
const AL_HEAL: u16 = 28;
const MC_MAMMONITE: u16 = 42;
const NV_FIRSTAID: u16 = 142;

const SP_HP: u16 = 5;
const SP_SP: u16 = 7;
const SP_ZENY: u16 = 20;
const SP_ASPD: u16 = 53;
const SP_INT: u32 = 16;
const SP_DEX: u32 = 17;
const SP_LUK: u32 = 18;

const BOW: u32 = 1701;
/// An etc item heavy enough to pass 90 % of any test character's max weight.
const ANVIL: u32 = 7000;
const EQP_ARMS: u32 = 0x22;
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
const SP_WEIGHT: u16 = 24;
const SP_VIT: u32 = 15;

/// The mob's cell; A starts next to it.
const MOB: (u16, u16) = (50, 50);
const NEXT_TO_MOB: (u16, u16) = (51, 50);
const MOB_DMOTION: i32 = 480;

const PORING: u32 = 1002;
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

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

fn gid(n: u32) -> u32 {
    2_000_000 + n
}

fn name(n: u32) -> String {
    format!("Caster{n}")
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

fn param(var_id: u16, value: i32) -> ServerEvent {
    ServerEvent::ParameterChanged { var_id, value }
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

/// ENG-EFF-01: First Aid below max HP: `0x0b1a`, SP, `0x09cb` 5, `SP_HP`; the observer gets
/// `0x0b1a` and `0x09cb`. The client's target id is ignored for a self skill.
#[test]
fn eng_eff_01_first_aid_heals_five() {
    let mut w = first_aid_world(10);
    let sp = w.status(A).sp;
    let out = w.cast(A, NV_FIRSTAID, 1, 12345);
    assert_eq!(
        seen_by(&out, A),
        [
            casting(gid(A), gid(A), NV_FIRSTAID, 0, 0),
            param(SP_SP, sp as i32 - 3),
            no_damage(NV_FIRSTAID, 5, gid(A), gid(A)),
            param(SP_HP, 15),
        ]
    );
    assert_eq!(
        seen_by(&out, B),
        [
            casting(gid(A), gid(A), NV_FIRSTAID, 0, 0),
            no_damage(NV_FIRSTAID, 5, gid(A), gid(A)),
        ]
    );
    assert_eq!((w.status(A).hp, w.status(A).sp), (15, sp - 3));
}

/// ENG-EFF-02: First Aid at full HP: `0x09cb` still says 5, and no HP push follows.
#[test]
fn eng_eff_02_first_aid_at_full_hp() {
    let mut w = first_aid_world(9999);
    let sp = w.status(A).sp;
    let out = w.cast(A, NV_FIRSTAID, 1, gid(A));
    assert_eq!(
        seen_by(&out, A),
        [
            casting(gid(A), gid(A), NV_FIRSTAID, 0, 0),
            param(SP_SP, sp as i32 - 3),
            no_damage(NV_FIRSTAID, 5, gid(A), gid(A)),
        ]
    );
}

/// ENG-EFF-03: a cast before `canact` is dropped with no packet; one at `T + amotion + 100` is
/// accepted.
#[test]
fn eng_eff_03_cast_gate() {
    let mut w = first_aid_world(10);
    let amotion = w.amotion(A);
    let t = w.now;
    assert!(!seen_by(&w.cast(A, NV_FIRSTAID, 1, gid(A)), A).is_empty());
    let early = t + amotion + 90;
    let out = w.at(
        early,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: NV_FIRSTAID,
                target: gid(A),
            },
        )],
    );
    assert_eq!(seen_by(&out, A), [], "too early is silent");
    let out = w.at(
        t + amotion + 100,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: NV_FIRSTAID,
                target: gid(A),
            },
        )],
    );
    assert_eq!(
        seen_by(&out, A)[0],
        casting(gid(A), gid(A), NV_FIRSTAID, 0, 0)
    );
}

/// ENG-EFF-04: not enough SP (cause 1), zeny (5), wrong weapon (6): `0x0110` alone, nothing
/// spent; SP is checked before zeny.
#[test]
fn eng_eff_04_requirement_failures() {
    let mut w = World::new(
        vec![Player::new(A, NOVICE, (40, 40), &[(NV_FIRSTAID, 1)]).status(|s| s.sp = 2)],
        PORING,
        mobs(1000, 1),
    );
    assert_eq!(
        seen_by(&w.cast(A, NV_FIRSTAID, 1, gid(A)), A),
        [fail(NV_FIRSTAID, 1)]
    );
    assert_eq!(w.status(A).sp, 2);

    // Merchant: no zeny, then no SP and no zeny.
    let mut w = World::new(
        vec![Player::new(A, MERCHANT, NEXT_TO_MOB, &[(MC_MAMMONITE, 10)])],
        PORING,
        mobs(100_000, 1),
    );
    let mob = w.mob_gid();
    let sp = w.status(A).sp;
    assert_eq!(
        seen_by(&w.cast(A, MC_MAMMONITE, 10, mob), A),
        [fail(MC_MAMMONITE, 5)]
    );
    assert_eq!((w.status(A).sp, w.status(A).zeny), (sp, 0));
    let mut w = World::new(
        vec![Player::new(A, MERCHANT, NEXT_TO_MOB, &[(MC_MAMMONITE, 10)]).status(|s| s.sp = 4)],
        PORING,
        mobs(100_000, 1),
    );
    let mob = w.mob_gid();
    assert_eq!(
        seen_by(&w.cast(A, MC_MAMMONITE, 10, mob), A),
        [fail(MC_MAMMONITE, 1)]
    );

    // Swordman with a bow: Bash refuses it.
    let mut bow = InventoryItem::new(ItemId(BOW), 1);
    bow.equip = EQP_ARMS;
    let mut swordman = Player::new(A, SWORDMAN, NEXT_TO_MOB, &[(SM_BASH, 10)]);
    swordman.inventory = vec![Some(bow)];
    let mut w = World::new(vec![swordman], PORING, mobs(100_000, 1));
    let mob = w.mob_gid();
    let sp = w.status(A).sp;
    assert_eq!(seen_by(&w.cast(A, SM_BASH, 10, mob), A), [fail(SM_BASH, 6)]);
    assert_eq!(w.status(A).sp, sp);

    // The requirements come before the range: out of range with 4 SP still gets cause 1.
    let mut w = World::new(
        vec![Player::new(A, SWORDMAN, (55, 50), &[(SM_BASH, 10)]).status(|s| s.sp = 4)],
        PORING,
        mobs(100_000, 1),
    );
    let mob = w.mob_gid();
    assert_eq!(seen_by(&w.cast(A, SM_BASH, 1, mob), A), [fail(SM_BASH, 1)]);

    // At 90 % weight or more: cause 9, before anything else.
    let mut novice = Player::new(A, NOVICE, (40, 40), &[(NV_FIRSTAID, 1)]);
    novice.inventory = vec![Some(InventoryItem::new(ItemId(ANVIL), 1))];
    let mut w = World::new(vec![novice], PORING, mobs(1000, 1));
    assert_eq!(
        seen_by(&w.cast(A, NV_FIRSTAID, 1, gid(A)), A),
        [fail(NV_FIRSTAID, 9)]
    );
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

/// ENG-EFF-05: a Bash hit: `0x0b1a` (element 0), SP, `0x01de` (sdelay amotion, ddelay dmotion,
/// type 8); the HP, `0x0977` and the kill land at `+amotion`.
#[test]
fn eng_eff_05_bash_hit_and_kill() {
    let mut w = bash_world(100_000, 1);
    let (mob, amotion, sp) = (w.mob_gid(), w.amotion(A), w.status(A).sp);
    let t = w.now;
    let to_a = seen_by(&w.cast(A, SM_BASH, 10, mob), A);
    assert_eq!(to_a.len(), 3, "{to_a:?}");
    assert_eq!(to_a[0], casting(gid(A), mob, SM_BASH, 0, 0));
    assert_eq!(to_a[1], param(SP_SP, sp as i32 - 15));
    let ServerEvent::SkillDamage {
        skill_id,
        src,
        dst,
        src_speed,
        dmg_speed,
        damage,
        level,
        div,
        damage_type,
        ..
    } = to_a[2].clone()
    else {
        panic!("third is 0x01de: {to_a:?}");
    };
    assert_eq!(
        (
            skill_id,
            src,
            dst,
            src_speed as u64,
            dmg_speed,
            level,
            div,
            damage_type
        ),
        (SM_BASH, gid(A), mob, amotion, MOB_DMOTION, 10, 1, 8)
    );
    assert!(damage > 0);
    assert_eq!(w.mob_hp(), 100_000, "the HP lands later");
    let later = w.run_to(A, t + amotion);
    assert_eq!(
        later,
        [ServerEvent::MobHpBar {
            gid: mob,
            hp: 100_000 - damage as u32,
            max_hp: 100_000,
        }]
    );

    // A kill: exp at +amotion, the death vanish 250 ms later.
    let mut w = bash_world(10, 1);
    let (mob, t) = (w.mob_gid(), w.now);
    drop(w.cast(A, SM_BASH, 10, mob));
    let later = w.run_to(A, t + amotion + 250);
    assert!(
        later
            .iter()
            .any(|e| matches!(e, ServerEvent::ExpGained { .. })),
        "{later:?}"
    );
    assert!(later.contains(&ServerEvent::UnitVanished {
        gid: mob,
        vanish_type: 1
    }));
}

/// ENG-EFF-06: a miss is `0x01de` damage 0 and ddelay 0, the SP is still spent, and no
/// `0x0977` follows. A Poring with very high FLEE makes misses common; the seeded roll decides.
#[test]
fn eng_eff_06_bash_miss() {
    let mut w = bash_world(100_000, 999);
    let (mob, amotion) = (w.mob_gid(), w.amotion(A));
    for _ in 0..20 {
        let sp = w.status(A).sp;
        let t = w.now;
        let to_a = seen_by(&w.cast(A, SM_BASH, 1, mob), A);
        let Some(ServerEvent::SkillDamage {
            damage, dmg_speed, ..
        }) = skill_damage(&to_a)
        else {
            panic!("no 0x01de: {to_a:?}");
        };
        let later = w.run_to(A, t + amotion + 200);
        if damage == 0 {
            assert_eq!(dmg_speed, 0);
            assert_eq!(w.status(A).sp, sp - 8);
            assert!(
                !later
                    .iter()
                    .any(|e| matches!(e, ServerEvent::MobHpBar { .. })),
                "{later:?}"
            );
            return;
        }
    }
    panic!("no miss in 20 casts at 5 % hit");
}

/// ENG-EFF-07: a Bash stops the auto-attack loop, which doesn't resume; a cast in the tick of a
/// due swing finds the swing's gate and is dropped.
#[test]
fn eng_eff_07_bash_during_attack_loop() {
    let mut w = bash_world(1_000_000, 1);
    let mob = w.mob_gid();
    let t0 = w.now;
    let out = w.at(
        t0,
        &[(
            A,
            ClientIntent::RequestAction {
                target_id: mob,
                action_type: 7,
            },
        )],
    );
    assert!(seen_by(&out, A)
        .iter()
        .any(|e| matches!(e, ServerEvent::Damage { .. })));
    let amotion = w.amotion(A);
    let adelay = 2 * amotion;
    // The second swing is due at t0 + adelay; a cast in that tick is dropped.
    let out = w.at(
        t0 + adelay,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: SM_BASH,
                target: mob,
            },
        )],
    );
    let to_a = seen_by(&out, A);
    assert!(
        to_a.iter().any(|e| matches!(e, ServerEvent::Damage { .. })),
        "the swing ran: {to_a:?}"
    );
    assert!(
        !to_a
            .iter()
            .any(|e| matches!(e, ServerEvent::SkillCasting { .. })),
        "the cast was dropped: {to_a:?}"
    );
    // Once the gate passes, the cast goes through and the loop stops for good.
    let t1 = t0 + adelay + amotion + 100;
    w.run_to(A, t1 - 10);
    let out = w.at(
        t1,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: SM_BASH,
                target: mob,
            },
        )],
    );
    assert!(skill_damage(&seen_by(&out, A)).is_some());
    let later = w.run_to(A, t1 + 5 * adelay);
    assert!(
        !later
            .iter()
            .any(|e| matches!(e, ServerEvent::Damage { .. })),
        "no swing after the cast: {later:?}"
    );
}

/// ENG-EFF-08: out of range standing is a silent drop; sent mid-walk out of `range - 1` it
/// fires at the step end; a self cast while walking stops the walk (`0x0088`) first.
#[test]
fn eng_eff_08_range_and_walking() {
    let mut w = World::new(
        vec![Player::new(A, SWORDMAN, (55, 50), &[(SM_BASH, 10)])],
        PORING,
        mobs(1_000_000, 1),
    );
    let mob = w.mob_gid();
    assert_eq!(seen_by(&w.cast(A, SM_BASH, 1, mob), A), [], "out of range");
    // Walk toward the mob and ask mid-walk: stored, then cast once adjacent.
    let t = w.now;
    drop(w.at(
        t,
        &[(
            A,
            ClientIntent::RequestMove {
                dest_x: 51,
                dest_y: 50,
            },
        )],
    ));
    let out = w.at(
        t + 50,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: SM_BASH,
                target: mob,
            },
        )],
    );
    assert!(skill_damage(&seen_by(&out, A)).is_none(), "not yet");
    let later = w.run_to(A, t + 2000);
    let cast_at = later
        .iter()
        .position(|e| matches!(e, ServerEvent::SkillCasting { .. }))
        .expect("the step action casts");
    assert!(skill_damage(&later[cast_at..]).is_some());

    // Heal on self while walking: `0x0088` then `0x0b1a`. (NV_BASIC 9: a first job with fewer
    // points spent sees only the Novice tree.)
    let mut w = World::new(
        vec![Player::new(
            A,
            ACOLYTE,
            (40, 40),
            &[(NV_BASIC, 9), (AL_HEAL, 1)],
        )],
        PORING,
        mobs(1000, 1),
    );
    let t = w.now;
    drop(w.at(
        t,
        &[(
            A,
            ClientIntent::RequestMove {
                dest_x: 40,
                dest_y: 30,
            },
        )],
    ));
    let out = w.at(
        t + 200,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: AL_HEAL,
                target: gid(A),
            },
        )],
    );
    let to_a = seen_by(&out, A);
    assert!(
        matches!(to_a[0], ServerEvent::UnitStopped { gid: g, .. } if g == gid(A)),
        "{to_a:?}"
    );
    assert_eq!(to_a[1], casting(gid(A), gid(A), AL_HEAL, 6, 0));
    assert!(!w.engine.is_walking(char_id(A)));

    // Heal on a mob within `range - 1` mid-walk: `0x0088`, then the cast at once.
    let mut w = World::new(
        vec![Player::new(
            A,
            ACOLYTE,
            (45, 50),
            &[(NV_BASIC, 9), (AL_HEAL, 1)],
        )],
        PORING,
        mobs(1000, 1),
    );
    let (mob, t) = (w.mob_gid(), w.now);
    let mv = w.at(
        t,
        &[(
            A,
            ClientIntent::RequestMove {
                dest_x: 45,
                dest_y: 40,
            },
        )],
    );
    assert!(!seen_by(&mv, A).is_empty(), "the walk starts");
    let out = w.at(
        t + 200,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: AL_HEAL,
                target: mob,
            },
        )],
    );
    let to_a = seen_by(&out, A);
    assert!(
        matches!(to_a[0], ServerEvent::UnitStopped { gid: g, .. } if g == gid(A)),
        "{to_a:?}"
    );
    assert_eq!(to_a[1], casting(gid(A), mob, AL_HEAL, 6, 0));

    // A cast ends a pending attack step action. Cast just after a step end, while the step
    // action's timer is armed: without the clear it would fire (`0x0139` or a swing).
    let mut w = World::new(
        vec![Player::new(A, NOVICE, (58, 50), &[(NV_FIRSTAID, 1)])],
        PORING,
        mobs(1_000_000, 1),
    );
    let (mob, t) = (w.mob_gid(), w.now);
    drop(w.at(
        t,
        &[(
            A,
            ClientIntent::RequestMove {
                dest_x: 51,
                dest_y: 50,
            },
        )],
    ));
    drop(w.at(
        t + 50,
        &[(
            A,
            ClientIntent::RequestAction {
                target_id: mob,
                action_type: 7,
            },
        )],
    ));
    let out = w.at(
        t + 170,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: NV_FIRSTAID,
                target: gid(A),
            },
        )],
    );
    assert!(seen_by(&out, A).contains(&casting(gid(A), gid(A), NV_FIRSTAID, 0, 0)));
    let later = w.run_to(A, t + 3000);
    assert!(
        !later.iter().any(|e| matches!(
            e,
            ServerEvent::Damage { .. } | ServerEvent::AttackOutOfRange { .. }
        )),
        "{later:?}"
    );
}

/// ENG-EFF-09: Mammonite pays SP then zeny (`0x00b1` with the new total) before `0x01de`.
#[test]
fn eng_eff_09_mammonite_pays_zeny() {
    let mut w = World::new(
        vec![Player::new(A, MERCHANT, NEXT_TO_MOB, &[(MC_MAMMONITE, 10)]).status(|s| s.zeny = 5000)],
        PORING,
        mobs(1_000_000, 1),
    );
    let (mob, sp) = (w.mob_gid(), w.status(A).sp);
    let to_a = seen_by(&w.cast(A, MC_MAMMONITE, 10, mob), A);
    assert_eq!(to_a[0], casting(gid(A), mob, MC_MAMMONITE, 0, 0));
    assert_eq!(to_a[1], param(SP_SP, sp as i32 - 5));
    assert_eq!(
        to_a[2],
        ServerEvent::LongParameterChanged {
            var_id: SP_ZENY,
            value: 4000
        }
    );
    assert!(matches!(to_a[3], ServerEvent::SkillDamage { .. }));
    assert_eq!(w.status(A).zeny, 4000);
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

/// ENG-EFF-10: Heal on self, on B, on a mob and at full HP: `0x0983` (efst 46, AID = caster,
/// 500) before the unclamped `0x09cb`; the target's own HP push; the mob's bar to its
/// damage-log members; nothing at full HP.
#[test]
fn eng_eff_10_heal_targets() {
    let mut w = heal_world(10, 9999, PORING);
    let amount = heal_amount(&w, A, 10);
    let sp = w.status(A).sp;
    let max_hp = w.status(A).max_hp;
    let to_a = seen_by(&w.cast(A, AL_HEAL, 10, gid(A)), A);
    assert_eq!(
        to_a,
        [
            casting(gid(A), gid(A), AL_HEAL, 6, 0),
            param(SP_SP, sp as i32 - 40),
            postdelay(gid(A), 500),
            no_damage(AL_HEAL, amount, gid(A), gid(A)),
            param(SP_HP, (10 + amount).min(max_hp as i32)),
        ]
    );

    // On B at full HP (B's max is small): the full amount, no push to anyone.
    let amotion = w.amotion(A);
    let t = w.now + 500.max(amotion + 100);
    w.run_to(A, t - 10);
    let out = w.at(
        t,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 10,
                skill_id: AL_HEAL,
                target: gid(B),
            },
        )],
    );
    let (to_a, to_b) = (seen_by(&out, A), seen_by(&out, B));
    assert_eq!(to_a[3], no_damage(AL_HEAL, amount, gid(B), gid(A)));
    assert_eq!(to_a.len(), 4, "{to_a:?}");
    assert_eq!(
        to_b,
        [
            casting(gid(A), gid(B), AL_HEAL, 6, 0),
            postdelay(gid(A), 500),
            no_damage(AL_HEAL, amount, gid(B), gid(A)),
        ]
    );

    // On B below max: B gets its own HP push, A doesn't.
    let mut w = heal_world(9999, 1, PORING);
    let out = w.cast(A, AL_HEAL, 1, gid(B));
    let b_max = w.status(B).max_hp;
    let to_b = seen_by(&out, B);
    assert_eq!(
        to_b.last(),
        Some(&param(SP_HP, (1 + heal_amount(&w, A, 1)).min(b_max as i32)))
    );
    assert!(!seen_by(&out, A)
        .iter()
        .any(|e| matches!(e, ServerEvent::ParameterChanged { var_id, .. } if *var_id == SP_HP)));

    // On a mob A has hit: its HP rises and its bar goes to A.
    let mut w = heal_world(9999, 9999, PORING);
    let mob = w.mob_gid();
    let t = w.now;
    drop(w.at(
        t,
        &[(
            A,
            ClientIntent::RequestAction {
                target_id: mob,
                action_type: 0,
            },
        )],
    ));
    w.run_to(A, t + 2000);
    let hurt = w.mob_hp();
    assert!(hurt < 1000);
    let to_a = seen_by(&w.cast(A, AL_HEAL, 1, mob), A);
    let healed = (hurt + heal_amount(&w, A, 1) as u32).min(1000);
    assert_eq!(
        to_a.last(),
        Some(&ServerEvent::MobHpBar {
            gid: mob,
            hp: healed,
            max_hp: 1000
        })
    );
    assert_eq!(w.mob_hp(), healed);
}

/// ENG-EFF-11: Heal on an Undead mob is dropped silently (deviation: rAthena deals holy
/// damage, captured in `scripting_2.txt`).
#[test]
fn eng_eff_11_heal_on_undead_is_dropped() {
    let mut w = heal_world(10, 9999, ZOMBIE);
    let mob = w.mob_gid();
    let sp = w.status(A).sp;
    assert_eq!(seen_by(&w.cast(A, AL_HEAL, 10, mob), A), []);
    assert_eq!(w.status(A).sp, sp);
}

/// ENG-EFF-12: a cast with a cast time: `0x0b1a` carries it (1000 ms at DEX 1 / INT 1 = 924),
/// another request meanwhile is dropped, and the effect lands at `CastEnd`.
#[test]
fn eng_eff_12_cast_time() {
    let mut w = World::with_skills(
        vec![Player::new(A, SWORDMAN, NEXT_TO_MOB, &[(SM_BASH, 10)])],
        PORING,
        mobs(1_000_000, 1),
        skills_with_bash_cast_time(),
    );
    let (mob, t) = (w.mob_gid(), w.now);
    let dex = w.battle_stat(A, SP_DEX);
    let int = w.battle_stat(A, 16);
    assert_eq!((dex, int), (1, 1), "the fixture's stats");
    let to_a = seen_by(&w.cast(A, SM_BASH, 1, mob), A);
    assert_eq!(to_a, [casting(gid(A), mob, SM_BASH, 0, 924)]);
    let mid = w.at(
        t + 400,
        &[(
            A,
            ClientIntent::UseSkill {
                level: 1,
                skill_id: SM_BASH,
                target: mob,
            },
        )],
    );
    assert_eq!(seen_by(&mid, A), [], "a cast is in progress");
    let before = w.run_to(A, t + 920);
    assert!(skill_damage(&before).is_none(), "{before:?}");
    let at_end = w.run_to(A, t + 930);
    assert!(
        matches!(at_end[0], ServerEvent::ParameterChanged { var_id, .. } if var_id == SP_SP),
        "{at_end:?}"
    );
    assert!(skill_damage(&at_end).is_some());
}

/// ENG-EFF-16: `@zeny` adds (or removes) and pushes `0x00b1`, refusing a result out of range;
/// `@heal` fills HP/SP or adds the given amounts, keeping HP at 1 or more.
#[test]
fn eng_eff_16_zeny_and_heal_commands() {
    let mut w = first_aid_world(10);
    let say = |w: &mut World, line: &str| {
        let text = format!("{} : @{line}", name(A)).into_bytes();
        let now = w.now + 10;
        seen_by(&w.at(now, &[(A, ClientIntent::Chat { text })]), A)
    };
    let got = say(&mut w, "zeny 5000");
    assert_eq!(
        got[0],
        ServerEvent::LongParameterChanged {
            var_id: SP_ZENY,
            value: 5000
        }
    );
    assert!(matches!(got[1], ServerEvent::SelfChat { .. }));
    assert_eq!(w.status(A).zeny, 5000);
    let got = say(&mut w, "zeny -6000");
    assert!(matches!(got[..], [ServerEvent::SelfChat { .. }]), "{got:?}");
    assert_eq!(w.status(A).zeny, 5000);
    let got = say(&mut w, "zeny -5000");
    assert_eq!(
        got[0],
        ServerEvent::LongParameterChanged {
            var_id: SP_ZENY,
            value: 0
        }
    );

    // SP is full already: only HP is pushed.
    let status = w.status(A);
    let got = say(&mut w, "heal");
    assert_eq!(got[0], param(SP_HP, status.max_hp as i32));
    assert!(matches!(got[1], ServerEvent::SelfChat { .. }), "{got:?}");
    let got = say(&mut w, "heal -100000 -3");
    assert_eq!(
        got[..2],
        [param(SP_HP, 1), param(SP_SP, status.max_sp as i32 - 3)]
    );
    let got = say(&mut w, "heal 0 0");
    assert!(matches!(got[..], [ServerEvent::SelfChat { .. }]), "{got:?}");
}

/// ENG-EFF-17: Bash on a player begins (`0x0b1a` to the area) and fails at castend with
/// `0x0110` cause 0; no SP, no damage; `canact` keeps cast begin's `+ amotion + 200`.
#[test]
fn eng_eff_17_attack_skill_on_player() {
    let mut w = World::new(
        vec![
            Player::new(A, SWORDMAN, (40, 40), &[(SM_BASH, 10)]),
            Player::new(B, NOVICE, (41, 40), &[]),
        ],
        PORING,
        mobs(1000, 1),
    );
    let (sp, amotion, t) = (w.status(A).sp, w.amotion(A), w.now);
    let out = w.cast(A, SM_BASH, 10, gid(B));
    assert_eq!(
        seen_by(&out, A),
        [casting(gid(A), gid(B), SM_BASH, 0, 0), fail(SM_BASH, 0)]
    );
    assert_eq!(seen_by(&out, B), [casting(gid(A), gid(B), SM_BASH, 0, 0)]);
    assert_eq!(w.status(A).sp, sp);
    let retry = |w: &mut World, now: u64| {
        seen_by(
            &w.at(
                now,
                &[(
                    A,
                    ClientIntent::UseSkill {
                        level: 10,
                        skill_id: SM_BASH,
                        target: gid(B),
                    },
                )],
            ),
            A,
        )
    };
    assert_eq!(retry(&mut w, t + amotion + 190), []);
    assert_eq!(retry(&mut w, t + amotion + 200)[1], fail(SM_BASH, 0));
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

/// ENG-EFF-13: a potion at reduced HP: `0x01c8` to the area with the remaining amount, then
/// `SP_WEIGHT` and `SP_HP` to self. The observer gets only the ack.
#[test]
fn eng_eff_13_potion_at_reduced_hp() {
    let mut w = potion_world(10, 9999, &[(HP_POTION, 5)]);
    let heal = potion_heal(40, w.battle_stat(A, SP_VIT));
    let now = w.now;
    let (to_a, to_b) = w.use_at(A, 2, now);
    let ack = item_ack(2, HP_POTION, A, 4, true);
    assert_eq!(
        to_a,
        [ack.clone(), param(SP_WEIGHT, 280), param(SP_HP, 10 + heal)]
    );
    assert_eq!(to_b, [ack]);
    assert_eq!(w.status(A).hp, 10 + heal as u32);
    assert_eq!(stack(&w, A, 0), Some((HP_POTION, 4)));
}

/// ENG-EFF-13 (spec §2.7): walking, sitting and an auto-attack loop are not gates, and a potion
/// stops neither the walk (no `0x0088`) nor the swings.
#[test]
fn eng_eff_13_potion_keeps_walk_sit_and_attack() {
    let mut w = potion_world(10, 9999, &[(HP_POTION, 5)]);
    let t = w.now;
    drop(w.at(
        t,
        &[(
            A,
            ClientIntent::RequestMove {
                dest_x: 51,
                dest_y: 58,
            },
        )],
    ));
    let (to_a, to_b) = w.use_at(A, 2, t + 50);
    assert_eq!(to_a.len(), 3, "ack, weight, HP only: {to_a:?}");
    assert!(matches!(to_a[0], ServerEvent::ItemUseAck { ok: true, .. }));
    assert_eq!(to_b, [item_ack(2, HP_POTION, A, 4, true)], "no stop");
    w.run_to(A, t + 3000);

    let t = w.now + 10;
    let sat = w.at(
        t,
        &[(
            A,
            ClientIntent::RequestAction {
                target_id: 0,
                action_type: 2,
            },
        )],
    );
    assert!(
        seen_by(&sat, B)
            .iter()
            .any(|e| matches!(e, ServerEvent::EntityAction { .. })),
        "A sat down"
    );
    let (to_a, _) = w.use_at(A, 2, t + 200);
    assert_eq!(to_a[0], item_ack(2, HP_POTION, A, 3, true), "sitting");
    drop(w.at(
        t + 400,
        &[(
            A,
            ClientIntent::RequestAction {
                target_id: 0,
                action_type: 3,
            },
        )],
    ));

    // Back next to the mob and swinging.
    let t = t + 500;
    drop(w.at(
        t,
        &[(
            A,
            ClientIntent::RequestMove {
                dest_x: NEXT_TO_MOB.0,
                dest_y: NEXT_TO_MOB.1,
            },
        )],
    ));
    w.run_to(A, t + 3000);
    let mob = w.mob_gid();
    let t = w.now + 10;
    let out = w.at(
        t,
        &[(
            A,
            ClientIntent::RequestAction {
                target_id: mob,
                action_type: 7,
            },
        )],
    );
    assert!(seen_by(&out, A)
        .iter()
        .any(|e| matches!(e, ServerEvent::Damage { .. })));
    let (to_a, _) = w.use_at(A, 2, t + 10);
    assert_eq!(to_a[0], item_ack(2, HP_POTION, A, 2, true), "attacking");
    let later = w.run_to(A, t + 10 + 4 * w.amotion(A));
    assert!(
        later
            .iter()
            .any(|e| matches!(e, ServerEvent::Damage { .. })),
        "the loop goes on: {later:?}"
    );
}

/// ENG-EFF-14: at full HP the potion is still used, with only the ack and `SP_WEIGHT`; an SP
/// potion pushes `SP_SP`; the last potion of a stack (a percent heal here) empties the slot.
#[test]
fn eng_eff_14_full_hp_sp_and_last_potion() {
    let mut w = potion_world(
        9999,
        0,
        &[(HP_POTION, 5), (SP_POTION, 5), (PERCENT_POTION, 1)],
    );
    let mut weight = 5 * 70 + 5 * 150 + 10;
    let t = w.now;

    let (to_a, to_b) = w.use_at(A, 2, t);
    weight -= 70;
    let ack = item_ack(2, HP_POTION, A, 4, true);
    assert_eq!(to_a, [ack.clone(), param(SP_WEIGHT, weight)]);
    assert_eq!(to_b, [ack]);
    assert_eq!(stack(&w, A, 0), Some((HP_POTION, 4)));

    let sp = potion_heal(20, w.battle_stat(A, SP_INT));
    let (to_a, _) = w.use_at(A, 3, t + 1000);
    weight -= 150;
    assert_eq!(
        to_a,
        [
            item_ack(3, SP_POTION, A, 4, true),
            param(SP_WEIGHT, weight),
            param(SP_SP, sp),
        ]
    );

    let _ = w.at(
        t + 1100,
        &[(
            A,
            ClientIntent::Chat {
                text: format!("{} : @heal -100000", name(A)).into_bytes(),
            },
        )],
    );
    let max_hp = w.status(A).max_hp as i32;
    let (to_a, to_b) = w.use_at(A, 4, t + 2000);
    weight -= 10;
    let ack = item_ack(4, PERCENT_POTION, A, 0, true);
    assert_eq!(
        to_a,
        [
            ack.clone(),
            param(SP_WEIGHT, weight),
            param(SP_HP, 1 + (max_hp * 10 / 100).max(1)),
        ]
    );
    assert_eq!(to_b, [ack]);
    assert_eq!(stack(&w, A, 2), None, "the last potion empties the slot");
}

/// ENG-EFF-15: refusals send the failure `0x01c8` (amount 0, result 0) to self only and consume
/// nothing: too soon after the last use, an item type that can't be used, no lifted `Use`, a
/// level, gender, job or class restriction. An empty slot or an index out of range is silent.
#[test]
fn eng_eff_15_potion_refused() {
    let mut w = potion_world(
        10,
        9999,
        &[
            (HP_POTION, 5),
            (NOT_USABLE, 1),
            (NO_USE, 1),
            (LEVEL_LOCKED, 1),
            (GENDER_LOCKED, 1),
            (JOB_LOCKED, 1),
            (CLASS_LOCKED, 1),
        ],
    );
    let t = w.now;
    let (to_a, _) = w.use_at(A, 2, t);
    assert_eq!(to_a[0], item_ack(2, HP_POTION, A, 4, true));
    // `item_use_interval` is 100 ms.
    let (to_a, to_b) = w.use_at(A, 2, t + 90);
    assert_eq!(to_a, [item_ack(2, HP_POTION, A, 0, false)], "too soon");
    assert_eq!(to_b, []);
    assert_eq!(stack(&w, A, 0), Some((HP_POTION, 4)));
    let (to_a, _) = w.use_at(A, 2, t + 100);
    assert_eq!(to_a[0], item_ack(2, HP_POTION, A, 3, true));

    for (slot, item) in [
        (1, NOT_USABLE),
        (2, NO_USE),
        (3, LEVEL_LOCKED),
        (4, GENDER_LOCKED),
        (5, JOB_LOCKED),
        (6, CLASS_LOCKED),
    ] {
        let now = w.now + 1000;
        let index = slot as u16 + 2;
        let (to_a, to_b) = w.use_at(A, index, now);
        assert_eq!(to_a, [item_ack(index, item, A, 0, false)], "item {item}");
        assert_eq!(to_b, []);
        assert_eq!(stack(&w, A, slot), Some((item, 1)));
    }

    for index in [0, 1, 9, u16::MAX] {
        let now = w.now + 1000;
        assert_eq!(w.use_at(A, index, now), (vec![], vec![]), "index {index}");
    }
}

/// ENG-EFF-15 (catalog `mechanics/inventory.md` §7.2): a loaded stack of amount 0 is silent like
/// an empty slot: no ack, no heal, nothing removed (final review).
#[test]
fn eng_eff_15_zero_amount_stack_is_silent() {
    let mut w = potion_world(10, 9999, &[(HP_POTION, 0)]);
    let now = w.now;
    assert_eq!(w.use_at(A, 2, now), (vec![], vec![]));
    assert_eq!(w.status(A).hp, 10);
    assert_eq!(stack(&w, A, 0), Some((HP_POTION, 0)));
}
