//! The Lua hook spike against the Rust backend (spec `specs/effects/spec.md` §2.11, SCR-LUA-01).
//! Built only with `--features lua`, which `verify.sh` doesn't pass:
//! `cargo test -p ro-scripting --features lua`.
#![cfg(feature = "lua")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::path::PathBuf;

use ro_core::ItemId;
use ro_scripting::{
    CastSource, CtxMode, Definitions, Effect, EffectList, HealShow, HookBackend, HookCtx,
    HookError, LuaHooks, Roll, RustHooks, SkillDef, SkillLevels, TargetRule, UnitView, Who,
};

const SM_BASH: u16 = 5;
const SM_RECOVERY: u16 = 4;
const MG_SRECOVERY: u16 = 9;
const AM_LEARNINGPOTION: u16 = 227;
const MODES: [CtxMode; 2] = [CtxMode::Table, CtxMode::Userdata];

/// A seeded xorshift, recording every `n` asked for and every value given.
struct SeededRoll {
    state: u64,
    draws: Vec<(u32, u32)>,
}

impl SeededRoll {
    fn new(seed: u64) -> Self {
        Self {
            state: seed | 1,
            draws: Vec::new(),
        }
    }
}

impl Roll for SeededRoll {
    fn roll(&mut self, n: u32) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        let value = if n == 0 {
            0
        } else {
            u32::try_from(self.state % u64::from(n)).expect("below n")
        };
        self.draws.push((n, value));
        value
    }
}

#[derive(Default)]
struct Levels(HashMap<u16, u8>);

impl SkillLevels for Levels {
    fn level(&self, skill: u16) -> u8 {
        self.0.get(&skill).copied().unwrap_or(0)
    }
}

fn script(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("lua")
        .join(name)
}

fn lua_hooks(mode: CtxMode) -> (LuaHooks, Vec<SkillDef>) {
    let mut lua = LuaHooks::new();
    lua.set_ctx_mode(mode);
    let mut defs = lua
        .load_file(&script("sm_bash.lua"))
        .expect("sm_bash.lua loads");
    defs.extend(
        lua.load_file(&script("item_heal.lua"))
            .expect("item_heal.lua loads"),
    );
    (lua, defs)
}

fn swordman() -> UnitView {
    UnitView {
        base_level: 50,
        job: 1,
        str: 40,
        agi: 20,
        vit: 33,
        int: 17,
        dex: 25,
        luk: 5,
        hp: 300,
        max_hp: 1200,
        sp: 40,
        max_sp: 150,
        matk_base: 30,
        ..UnitView::default()
    }
}

fn ctx<'a>(
    skill: u16,
    level: u8,
    source: CastSource,
    args: [i32; 4],
    skills: &'a Levels,
) -> HookCtx<'a> {
    HookCtx {
        caster: swordman(),
        target: swordman(),
        skill,
        level,
        source,
        args,
        skills,
    }
}

/// Resolves `def` against `hooks` with a fresh roll from `seed`.
fn resolve(
    def: &SkillDef,
    hooks: &dyn HookBackend,
    context: &HookCtx<'_>,
    seed: u64,
) -> (EffectList, Vec<(u32, u32)>) {
    let mut roll = SeededRoll::new(seed);
    let mut out = EffectList::new();
    def.resolve(context, hooks, &mut roll, &mut out)
        .expect("resolves");
    (out, roll.draws)
}

/// SCR-LUA-01: Bash from `sm_bash.lua`, as the declarative definition and as the hook, gives
/// the Rust Bash effects at lv 1-10 with the same seed, in both context modes.
#[test]
fn scr_lua_01_bash_matches_rust() {
    let rust = RustHooks::builtin();
    let rust_defs = Definitions::builtin(&rust).expect("the Rust backend has every hook");
    let rust_bash = rust_defs.skill(SM_BASH).expect("Bash is defined");
    let levels = Levels::default();
    for mode in MODES {
        let (lua, defs) = lua_hooks(mode);
        let declared = defs
            .iter()
            .find(|def| def.id == SM_BASH)
            .expect("sm_bash.lua defines Bash");
        assert_eq!(declared.effects, rust_bash.effects);
        assert_eq!(declared.hook, None);
        let scripted = SkillDef {
            id: SM_BASH,
            effects: Vec::new(),
            hook: Some(lua.hook_id("sm_bash").expect("sm_bash.lua has the hook")),
            target: TargetRule::default(),
        };
        for level in 1..=10u8 {
            let context = ctx(SM_BASH, level, CastSource::Learned, [0; 4], &levels);
            let seed = 0x5eed + u64::from(level);
            let expected = resolve(rust_bash, &rust, &context, seed);
            assert_eq!(
                *expected.0,
                [Effect::WeaponDamage {
                    ratio: 100 + 30 * i32::from(level),
                    hit_pct: 5 * i32::from(level),
                    div: 1
                }]
            );
            assert_eq!(
                resolve(declared, &lua, &context, seed),
                expected,
                "{mode:?}"
            );
            assert_eq!(
                resolve(&scripted, &lua, &context, seed),
                expected,
                "{mode:?}"
            );
        }
    }
}

/// SCR-LUA-01 (rolls): the Lua `item_heal` draws the same rolls in the same order as the Rust
/// one and gives the same heal, including a constant side that draws nothing (ruling 9) and the
/// skill-level bonuses.
#[test]
fn scr_lua_01_item_heal_matches_rust() {
    let rust = RustHooks::builtin();
    let rust_defs = Definitions::builtin(&rust).expect("the Rust backend has every hook");
    let rust_def = rust_defs.item_use(ro_scripting::ItemUse::ItemHeal);
    let mut trained = Levels::default();
    trained.0.insert(SM_RECOVERY, 5);
    trained.0.insert(MG_SRECOVERY, 3);
    trained.0.insert(AM_LEARNINGPOTION, 4);
    let argsets = [
        [45, 65, 0, 0],
        [0, 0, 40, 60],
        [100, 100, 0, 0],
        [325, 405, 40, 60],
        [0, 0, 0, 0],
    ];
    for mode in MODES {
        let (lua, _) = lua_hooks(mode);
        let lua_def = ro_scripting::ItemUseDef {
            kind: ro_scripting::ItemUse::ItemHeal,
            effects: Vec::new(),
            hook: Some(
                lua.hook_id("item_heal")
                    .expect("item_heal.lua has the hook"),
            ),
        };
        for levels in [&Levels::default(), &trained] {
            for args in argsets {
                for seed in [1, 7, 0xdead_beef] {
                    let context = ctx(0, 0, CastSource::ItemUse(ItemId(501)), args, levels);
                    let run = |def: &ro_scripting::ItemUseDef, hooks: &dyn HookBackend| {
                        let mut roll = SeededRoll::new(seed);
                        let mut out = EffectList::new();
                        def.resolve(&context, hooks, &mut roll, &mut out)
                            .expect("resolves");
                        (out, roll.draws)
                    };
                    let expected = run(rust_def, &rust);
                    let ranged = usize::from(args[0] != args[1]) + usize::from(args[2] != args[3]);
                    assert_eq!(expected.1.len(), ranged, "{args:?}");
                    assert_eq!(run(&lua_def, &lua), expected, "{mode:?} {args:?} {seed}");
                }
            }
        }
    }
}

/// A Lua error (a bad range) is a `HookError::Failed` that names the hook, and the resolve
/// leaves nothing behind.
#[test]
fn scr_lua_01_script_error_fails_the_hook() {
    let levels = Levels::default();
    for mode in MODES {
        let (lua, _) = lua_hooks(mode);
        let def = ro_scripting::ItemUseDef {
            kind: ro_scripting::ItemUse::ItemHeal,
            effects: Vec::new(),
            hook: lua.hook_id("item_heal"),
        };
        let context = ctx(
            0,
            0,
            CastSource::ItemUse(ItemId(501)),
            [10, 5, 0, 0],
            &levels,
        );
        let mut out = EffectList::new();
        out.push(Effect::SpendSp(3));
        let error = def
            .resolve(&context, &lua, &mut SeededRoll::new(1), &mut out)
            .expect_err("a bad range fails");
        match error {
            HookError::Failed(message) => {
                assert!(message.contains("item_heal: bad range 10..=5"), "{message}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(out.is_empty());
    }
}

/// The loader rejects a skill naming a missing hook and a duplicate hook name, and keeps
/// nothing from a failed load; a hook returning an unknown effect kind fails the call and adds
/// nothing.
#[test]
fn scr_lua_01_loader_checks_the_table() {
    let mut lua = LuaHooks::new();
    let missing = lua
        .load_source(
            "missing",
            "return { skills = { { id = 5, hook = 'nope' } } }",
        )
        .expect_err("the hook doesn't exist");
    assert!(missing.0.contains("no hook named nope"), "{missing}");

    lua.load_source(
        "first",
        "return { hooks = { odd = function() return { { kind = 'spend_sp', amount = 1 }, { kind = 'teleport' } } end } }",
    )
    .expect("loads");
    let duplicate = lua
        .load_source(
            "second",
            "return { hooks = { odd = function() end, other = function() end } }",
        )
        .expect_err("odd is already loaded");
    assert!(
        duplicate.0.contains("hook odd is already loaded"),
        "{duplicate}"
    );
    assert_eq!(lua.hook_id("other"), None);

    let levels = Levels::default();
    let context = ctx(SM_BASH, 1, CastSource::Learned, [0; 4], &levels);
    let mut out = EffectList::new();
    out.push(Effect::SpendSp(3));
    let error = lua
        .call(
            lua.hook_id("odd").expect("loaded"),
            &context,
            &mut SeededRoll::new(1),
            &mut out,
        )
        .expect_err("teleport isn't an effect yet");
    assert!(
        matches!(&error, HookError::Failed(m) if m.contains("unknown effect kind \"teleport\"")),
        "{error:?}"
    );
    // A hook that fails partway leaves `out` as it was, like a Rust hook.
    assert_eq!(*out, [Effect::SpendSp(3)]);
}

/// A heal spec with a fixed number and the context's fields reach Lua in both modes.
#[test]
fn scr_lua_01_context_fields() {
    let source = r#"
        return {
          skills = { { id = 142, effects = { { kind = "heal", to = "caster", hp = 5, sp = 0, show = "skill" } } } },
          hooks = {
            echo = function(ctx)
              return { {
                kind = "weapon_damage",
                ratio = ctx.caster.vit * 1000 + ctx.target.max_sp,
                hit_pct = ctx.skill * 100 + ctx.level,
                div = (ctx.source == "item_use" and ctx.item or 0) + ctx.args[4],
              } }
            end,
          },
        }
    "#;
    let levels = Levels::default();
    for mode in MODES {
        let mut lua = LuaHooks::new();
        lua.set_ctx_mode(mode);
        let defs = lua.load_source("echo", source).expect("loads");
        let mut out = EffectList::new();
        defs[0]
            .resolve(
                &ctx(142, 1, CastSource::Learned, [0; 4], &levels),
                &lua,
                &mut SeededRoll::new(1),
                &mut out,
            )
            .expect("resolves");
        assert_eq!(
            *out,
            [Effect::Heal {
                to: Who::Caster,
                hp: 5,
                sp: 0,
                show: HealShow::Skill
            }]
        );
        out.clear();
        lua.call(
            lua.hook_id("echo").expect("loaded"),
            &ctx(7, 3, CastSource::ItemUse(ItemId(12)), [0, 0, 0, 9], &levels),
            &mut SeededRoll::new(1),
            &mut out,
        )
        .expect("calls");
        assert_eq!(
            *out,
            [Effect::WeaponDamage {
                ratio: 33_150,
                hit_pct: 703,
                div: 21
            }],
            "{mode:?}"
        );
    }
}
