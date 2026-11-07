//! Definitions, resolve and the Rust hooks (spec `specs/effects/spec.md` SCR-EFF-01..05).

use std::collections::HashMap;

use ro_core::ItemId;
use ro_scripting::{
    CastSource, Definitions, Effect, EffectList, HealShow, HookBackend, HookCtx, HookError, HookId,
    ItemUse, MissingHook, Roll, RustHooks, UnitView, Who,
};

use crate::common::{Levels, SM_BASH, SM_RECOVERY};

/// Answers with queued values (each taken `% n`), then 0; records every `n` asked for.
#[derive(Default)]
struct FixedRoll {
    values: Vec<u32>,
    asked: Vec<u32>,
}

impl FixedRoll {
    fn new(values: &[u32]) -> Self {
        Self {
            values: values.iter().rev().copied().collect(),
            asked: Vec::new(),
        }
    }
}

impl Roll for FixedRoll {
    fn roll(&mut self, n: u32) -> u32 {
        self.asked.push(n);
        let value = self.values.pop().unwrap_or(0);
        if n == 0 {
            0
        } else {
            value % n
        }
    }
}

const AL_HEAL: u16 = 28;
const MC_MAMMONITE: u16 = 42;
const NV_FIRSTAID: u16 = 142;
const RED_POTION: ItemId = ItemId(501);

/// The capture's Acolyte (catalog `scripting/skills.md` §14.4): BaseLv 13, all stats 1, no
/// weapon, so status MATK = 1 + 1/2 + 1/5 + 1/3 + 13/4 = 4.
fn acolyte() -> UnitView {
    UnitView {
        base_level: 13,
        job: 4,
        str: 1,
        agi: 1,
        vit: 1,
        int: 1,
        dex: 1,
        luk: 1,
        hp: 89,
        max_hp: 137,
        sp: 75,
        max_sp: 75,
        matk_base: 4,
        ..UnitView::default()
    }
}

fn ctx<'a>(
    caster: UnitView,
    skill: u16,
    level: u8,
    source: CastSource,
    args: [i32; 4],
    skills: &'a Levels,
) -> HookCtx<'a> {
    HookCtx {
        caster,
        target: caster,
        skill,
        level,
        source,
        args,
        skills,
    }
}

fn setup() -> (RustHooks, Definitions) {
    let hooks = RustHooks::builtin();
    let defs = Definitions::builtin(&hooks).expect("the Rust backend has every built-in hook");
    (hooks, defs)
}

/// Resolves skill `id` at `level` for `caster`, with `rolls` queued.
fn cast(id: u16, level: u8, caster: UnitView, rolls: &[u32]) -> (EffectList, Vec<u32>) {
    let (hooks, defs) = setup();
    let levels = Levels::default();
    let mut roll = FixedRoll::new(rolls);
    let mut out = EffectList::new();
    let context = ctx(caster, id, level, CastSource::Learned, [0; 4], &levels);
    defs.skill(id)
        .expect("defined")
        .resolve(&context, &hooks, &mut roll, &mut out)
        .expect("resolves");
    (out, roll.asked)
}

fn heal(to: Who, hp: u32, sp: u32, show: HealShow) -> Effect {
    Effect::Heal { to, hp, sp, show }
}

/// SCR-EFF-01: First Aid heals the caster 5 HP and draws no roll.
#[test]
fn scr_eff_01_first_aid() {
    let (out, rolls) = cast(NV_FIRSTAID, 1, acolyte(), &[]);
    assert_eq!(*out, [heal(Who::Caster, 5, 0, HealShow::Skill)]);
    assert!(rolls.is_empty());
}

/// SCR-EFF-02: Bash lv 1..10 is ratio 130..400 and hit +5..+50 %; Mammonite is 150..600.
#[test]
fn scr_eff_02_weapon_skills() {
    for level in 1..=10u8 {
        let lv = i32::from(level);
        let (out, rolls) = cast(SM_BASH, level, acolyte(), &[]);
        assert_eq!(
            *out,
            [Effect::WeaponDamage {
                ratio: 100 + 30 * lv,
                hit_pct: 5 * lv,
                div: 1
            }]
        );
        assert!(rolls.is_empty(), "the hit rolls belong to the engine");
        let (out, _) = cast(MC_MAMMONITE, level, acolyte(), &[]);
        assert_eq!(
            *out,
            [Effect::WeaponDamage {
                ratio: 100 + 50 * lv,
                hit_pct: 0,
                div: 1
            }]
        );
    }
}

/// SCR-EFF-03: Heal = `(13 + 1) / 5 × 30 × lv / 10 + MATK 4`: 64 at lv 10 and 10 at lv 1, the
/// captured values (`scripting_1.txt` `#6711`, `#7059`). No weapon: no roll.
#[test]
fn scr_eff_03_heal() {
    let (out, rolls) = cast(AL_HEAL, 10, acolyte(), &[]);
    assert_eq!(*out, [heal(Who::Target, 64, 0, HealShow::Skill)]);
    assert!(rolls.is_empty());
    let (out, _) = cast(AL_HEAL, 1, acolyte(), &[]);
    assert_eq!(*out, [heal(Who::Target, 10, 0, HealShow::Skill)]);
}

/// SCR-EFF-03: `scripting_2.txt` (catalog `scripting/skills.md` §15): BaseLv 99, INT 99, DEX 1,
/// LUK 1, no weapon. Status MATK `99 + 49 + 0 + 0 + 24 = 172`; lv 10 = `39 × 30 + 172` = 1342,
/// lv 1 = `117 + 172` = 289.
#[test]
fn scr_eff_03_heal_high_level() {
    let priest = UnitView {
        base_level: 99,
        int: 99,
        matk_base: 172,
        ..acolyte()
    };
    let (out, _) = cast(AL_HEAL, 10, priest, &[]);
    assert_eq!(*out, [heal(Who::Target, 1342, 0, HealShow::Skill)]);
    let (out, _) = cast(AL_HEAL, 1, priest, &[]);
    assert_eq!(*out, [heal(Who::Target, 289, 0, HealShow::Skill)]);
}

/// SCR-EFF-03: a weapon MATK 100 at weapon level 2 gives variance 20, so the MATK term is
/// `4 + 80 + roll(40)`, within `[84, 124)`.
#[test]
fn scr_eff_03_heal_weapon_roll() {
    let staff = UnitView {
        weapon_matk: 100,
        weapon_level: 2,
        ..acolyte()
    };
    for (roll, expected) in [(0, 60 + 84), (39, 60 + 123), (40, 60 + 84)] {
        let (out, rolls) = cast(AL_HEAL, 10, staff, &[roll]);
        assert_eq!(*out, [heal(Who::Target, expected, 0, HealShow::Skill)]);
        assert_eq!(rolls, [40]);
    }
}

fn drink(
    caster: UnitView,
    kind: ItemUse,
    args: [i32; 4],
    levels: &Levels,
    rolls: &[u32],
) -> (EffectList, Vec<u32>) {
    let (hooks, defs) = setup();
    let mut roll = FixedRoll::new(rolls);
    let mut out = EffectList::new();
    let context = ctx(caster, 0, 0, CastSource::ItemUse(RED_POTION), args, levels);
    defs.item_use(kind)
        .resolve(&context, &hooks, &mut roll, &mut out)
        .expect("resolves");
    (out, roll.asked)
}

/// SCR-EFF-04: `itemheal`. Red Potion `[45, 65]` rolled 47 at VIT 51 is `47 × 202 / 100 = 94`
/// (captured, catalog `mechanics/inventory.md` §7.5). Blue Potion SP `[40, 60]` rolled 57 at
/// INT 1 is `57 × 102 / 100 = 58`. A constant side draws no roll.
#[test]
fn scr_eff_04_item_heal() {
    let none = Levels::default();
    let tough = UnitView {
        vit: 51,
        ..acolyte()
    };
    let (out, rolls) = drink(tough, ItemUse::ItemHeal, [45, 65, 0, 0], &none, &[2]);
    assert_eq!(*out, [heal(Who::Caster, 94, 0, HealShow::Silent)]);
    assert_eq!(rolls, [21], "HP only: the SP side is a constant 0");

    let (out, rolls) = drink(acolyte(), ItemUse::ItemHeal, [0, 0, 40, 60], &none, &[17]);
    assert_eq!(*out, [heal(Who::Caster, 0, 58, HealShow::Silent)]);
    assert_eq!(rolls, [21]);

    // HP first, then SP.
    let (_, rolls) = drink(acolyte(), ItemUse::ItemHeal, [10, 19, 5, 9], &none, &[]);
    assert_eq!(rolls, [10, 5]);
}

/// SCR-EFF-04: the bonus is skipped at exactly 100 %, and the recovery skills add to it.
#[test]
fn scr_eff_04_item_heal_bonus() {
    let none = Levels::default();
    let zero = UnitView {
        vit: 0,
        int: 0,
        ..acolyte()
    };
    let (out, _) = drink(zero, ItemUse::ItemHeal, [45, 65, 0, 0], &none, &[2]);
    assert_eq!(*out, [heal(Who::Caster, 47, 0, HealShow::Silent)]);
    // Increase HP Recovery 10: 100 + 100 % = ×2.
    let recovery = Levels(HashMap::from([(SM_RECOVERY, 10)]));
    let (out, _) = drink(zero, ItemUse::ItemHeal, [45, 65, 0, 0], &recovery, &[2]);
    assert_eq!(*out, [heal(Who::Caster, 94, 0, HealShow::Silent)]);
}

/// SCR-EFF-04: `percentheal` is a percent of max with no scaling and no roll; a non-zero rate
/// heals at least 1, and 100 % or more is the full max.
#[test]
fn scr_eff_04_percent_heal() {
    let none = Levels::default();
    let (out, rolls) = drink(acolyte(), ItemUse::PercentHeal, [10, 5, 0, 0], &none, &[]);
    // 137 × 10 / 100 = 13; 75 × 5 / 100 = 3.
    assert_eq!(*out, [heal(Who::Caster, 13, 3, HealShow::Silent)]);
    assert!(rolls.is_empty());
    let (out, _) = drink(acolyte(), ItemUse::PercentHeal, [1, 0, 0, 0], &none, &[]);
    assert_eq!(*out, [heal(Who::Caster, 1, 0, HealShow::Silent)]);
    let (out, _) = drink(
        acolyte(),
        ItemUse::PercentHeal,
        [100, 150, 0, 0],
        &none,
        &[],
    );
    assert_eq!(*out, [heal(Who::Caster, 137, 75, HealShow::Silent)]);
}

/// A backend whose every hook fails.
struct Failing;

impl HookBackend for Failing {
    fn hook_id(&self, _name: &str) -> Option<HookId> {
        Some(HookId(0))
    }

    fn call(
        &self,
        _hook: HookId,
        _ctx: &HookCtx<'_>,
        _roll: &mut dyn Roll,
        _out: &mut EffectList,
    ) -> Result<(), HookError> {
        Err(HookError::Failed("boom".into()))
    }
}

/// SCR-EFF-05: a hook error clears the list (the engine's costs too) and is returned.
#[test]
fn scr_eff_05_hook_error_clears() {
    let defs = Definitions::builtin(&Failing).expect("Failing names every hook");
    let levels = Levels::default();
    let mut out = EffectList::new();
    out.push(Effect::SpendSp(40));
    let context = ctx(acolyte(), AL_HEAL, 10, CastSource::Learned, [0; 4], &levels);
    let result = defs.skill(AL_HEAL).expect("heal").resolve(
        &context,
        &Failing,
        &mut FixedRoll::default(),
        &mut out,
    );
    assert_eq!(result, Err(HookError::Failed("boom".into())));
    assert!(out.is_empty());
}

/// SCR-EFF-05: unsupported sources and bad hook input fail without panicking.
#[test]
fn scr_eff_05_rejections() {
    let (hooks, defs) = setup();
    let levels = Levels::default();
    let mut roll = FixedRoll::default();
    let mut out = EffectList::new();
    let first_aid = defs.skill(NV_FIRSTAID).expect("first aid");
    for source in [
        CastSource::ItemSkill(ItemId(601)),
        CastSource::Mob,
        CastSource::ItemUse(RED_POTION),
    ] {
        out.push(Effect::SpendSp(3));
        let context = ctx(acolyte(), NV_FIRSTAID, 1, source, [0; 4], &levels);
        let result = first_aid.resolve(&context, &hooks, &mut roll, &mut out);
        assert_eq!(result, Err(HookError::Unsupported(source)));
        assert!(out.is_empty());
    }
    // An item use needs an `ItemUse` source.
    let context = ctx(
        acolyte(),
        0,
        0,
        CastSource::Learned,
        [45, 65, 0, 0],
        &levels,
    );
    let result = defs
        .item_use(ItemUse::ItemHeal)
        .resolve(&context, &hooks, &mut roll, &mut out);
    assert_eq!(result, Err(HookError::Unsupported(CastSource::Learned)));
    // A reversed range is a hook error.
    let context = ctx(
        acolyte(),
        0,
        0,
        CastSource::ItemUse(RED_POTION),
        [65, 45, 0, 0],
        &levels,
    );
    let result = defs
        .item_use(ItemUse::ItemHeal)
        .resolve(&context, &hooks, &mut roll, &mut out);
    assert!(matches!(result, Err(HookError::Failed(_))));
    assert!(out.is_empty());
    // An id the backend doesn't have.
    let result = hooks.call(HookId(99), &context, &mut roll, &mut out);
    assert_eq!(result, Err(HookError::UnknownHook(HookId(99))));
}

/// Hook names are bound once; a backend missing one is refused. Only the four slice skills are
/// defined, and only Heal refuses Undead targets.
#[test]
fn definitions_bind_hooks_and_rules() {
    struct Empty;
    impl HookBackend for Empty {
        fn hook_id(&self, _name: &str) -> Option<HookId> {
            None
        }
        fn call(
            &self,
            hook: HookId,
            _ctx: &HookCtx<'_>,
            _roll: &mut dyn Roll,
            _out: &mut EffectList,
        ) -> Result<(), HookError> {
            Err(HookError::UnknownHook(hook))
        }
    }
    assert_eq!(Definitions::builtin(&Empty), Err(MissingHook("al_heal")));
    let (hooks, defs) = setup();
    let ids: Vec<u16> = defs.skills().map(|def| def.id).collect();
    assert_eq!(ids, [SM_BASH, AL_HEAL, MC_MAMMONITE, NV_FIRSTAID]);
    assert_eq!(
        defs.skill(AL_HEAL).expect("heal").hook,
        hooks.hook_id("al_heal")
    );
    assert!(defs.skill(AL_HEAL).expect("heal").target.reject_undead);
    assert!(!defs.skill(SM_BASH).expect("bash").target.reject_undead);
    assert!(defs.skill(6).is_none(), "Provoke has no definition yet");
}
