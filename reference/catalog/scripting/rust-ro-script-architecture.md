# rust-ro script and skill architecture (what to borrow, what to avoid)

Source: rust-ro (`$RUSTRO_PATH`, MIT, Copyright (c) 2023 Nicolas Meylan; keep that notice if any code is copied). Paths relative to the rust-ro root; line numbers spot-checked 2026-10-31. rust-ro is **pre-renewal oriented and old-packetver**: its packet set (`0x07fb`, `0x011a`, via `PacketZcUseSkill`, `PacketZcNotifySkill2`, `PacketZcUseskillAck2`, `PacketZcAckTouseskill`, `PacketZcActionFailure`, `PacketZcMsgStateChange2`) is not the `20260219` set; take the architecture, not the wire format. Related: [units.md](./units.md), [lua_notes.md](./lua_notes.md), [skills.md](./skills.md), [items.md](./items.md).

## 1. Skills

* `lib/skills/src/lib.rs`: traits `SkillBase` (line 18), `Skill: SkillBase` (214), `OffensiveSkillBase`/`OffensiveSkill` (400/431: damage ratio, element, hit count, status effects), plus empty marker traits for supportive, performance and passive skills (466-473). Per-skill structs (about 406) are generated from JSON by `tools/skills`; per-job base data in `lib/skills/src/base/*_base.rs`; lookup by `SkillEnum::from_id`.
* Buff skills return lists of `TemporaryStatusBonus` (`lib/models/src/status_bonus.rs:41`). Skills are pure over a `StatusSnapshot` argument; services own packet sending.
* Heal, ground, teleport and summon skills are unimplemented: there is no hook point for effects that mutate world state.

## 2. Cast flow

`handle_use_skill` (`server/src/server/request_handler/action/skill.rs:7`) -> `character_start_use_skill` (`server/src/server/service/server_service.rs:346`: range check, store pending skill and path toward the target) -> `SkillService::start_use_skill` (`service/skill_service.rs:56`: validators for sp, hp, ammo, weapon, zeny, item; cast-time reduction; cast-start packet; returns `SkillCasted`) -> per-tick `do_use_skill` (`:162`, after cast time) -> post-delay. SP is never actually deducted (tests for consume are empty, `tests/skill_service_test.rs:639-647`).

## 3. Item scripts

* Item `Script` text is rAthena syntax compiled by the interpreter at boot (`server/src/main.rs:205-235`: hash the script, recompile only on change, store the compiled binary in the DB).
* Static vs dynamic split: bonus-only scripts are evaluated once into `Vec<BonusType>` (`repository/model/item_model.rs:115`, `BonusScriptHandler` in `script/bonus.rs`, run at load via `item_service.rs:147`); scripts that depend on character state (e.g. refine, stats) are re-run per status snapshot by `DynamicItemScriptHandler` (`script/item_script_handler.rs:17`, called from `status_service.rs:157`). `BonusType` has one variant per `bonus bXxx` (`script/bonus.rs`).
* Usable items run in a VM with a process-global `static mut` handler (`script/mod.rs:138`); NPC dialogs block one OS thread per player waiting on a channel (`script/interaction.rs`, `block_recv` at `script/mod.rs:209`).

## 4. Interpreter host interface (facts only; interpreter is unlicensed, do not copy code)

* Host implements a trait `NativeMethodHandler` with `handle(&self, native, params: Vec<Value>, execution_thread, call_frame, source_line, class_name)` (`script/mod.rs:161`); dispatch is by comparing `native.name` strings.
* Natives are declared in a text file (`native_functions_list.txt`): `name, min_args, max_args[, return type]`; `Vm::new(path, debug_flag)` loads it.
* `Value` is an i32 or a string only. Results are pushed with `execution_thread.push_constant_on_stack`. Per-run constants (char id etc.) are passed via thread constants.
* No suspension: a native that waits blocks the calling OS thread.

## 5. Borrow / avoid

* Borrow: skills as data-returning functions with typed failures; pending-skill walk into range before casting; static/dynamic bonus split with compile-once caching by script hash; services separate from packets.
* Avoid: generated code bloat (406 structs); packet serialization inside services; skills with no hook point to mutate the world; `static mut` singletons; blocking one thread per NPC dialog; never deducting SP.
