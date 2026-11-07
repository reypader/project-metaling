use std::path::Path;
use std::sync::Arc;

use ro_core::{
    AccountId, BroadcastScope, CharId, CharacterSpawn, CharacterStatus, MapPosition,
    ScopedServerEvent, Sex,
};
use ro_db::{JobDb, MapGrid, WeaponType};
use ro_mechanics::WeaponClass;

use super::*;
use crate::ecs::Entity;
use crate::map_instance::{MapInstance, Registries};
use crate::protocol_consts::{SP_HP, SP_MAXHP, SP_MAXSP, SP_SP};

fn char_id(n: u32) -> CharId {
    CharId(150_000 + n)
}

/// An instance with the shipped job data, and a Novice B11 J1 (MaxHP 90, MaxSP 21) at full
/// HP/SP.
fn novice_b11() -> (MapInstance, Entity) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ro-db/data/job");
    let grid = Arc::new(MapGrid::all_walkable(50, 50).expect("dims"));
    let mut engine = MapInstance::new("prontera.gat", grid);
    engine.set_registries(Registries {
        jobs: Arc::new(JobDb::load_dir(&dir).expect("shipped job data")),
        ..Registries::default()
    });
    let spawn = CharacterSpawn {
        account_id: AccountId(2_000_001),
        char_id: char_id(1),
        name: "C1".into(),
        sex: Sex::Male,
        job_class: 0,
        base_level: 11,
        hair_style: 1,
        hair_color: 1,
        pos: MapPosition {
            x: 10,
            y: 10,
            dir: 0,
        },
        speed: 150,
        status: CharacterStatus {
            hp: 1000,
            sp: 1000,
            ..CharacterStatus::default()
        },
        group_id: 0,
        skills: Vec::new(),
        hidden_skills: Vec::new(),
        change_level: [0; 3],
        inventory: Default::default(),
    };
    engine.spawn_character(spawn, 0).expect("spawn");
    let e = engine.world.entity_by_char(char_id(1)).expect("entity");
    (engine, e)
}

fn tick(engine: &mut MapInstance) -> Vec<ServerEvent> {
    let mut out: Vec<ScopedServerEvent> = Vec::new();
    engine.step_tick(100, &mut out).expect("tick");
    out.into_iter()
        .filter(|e| e.scope == BroadcastScope::SelfOnly(char_id(1)))
        .map(|e| e.event)
        .collect()
}

fn p(var_id: u16, value: i32) -> ServerEvent {
    ServerEvent::ParameterChanged { var_id, value }
}

fn set_level(engine: &mut MapInstance, e: Entity, level: u16) {
    engine.world.players.get_mut(e).expect("player").clevel = level;
}

#[test]
fn spawn_clamps_hp_and_sp_to_the_computed_max() {
    let (engine, e) = novice_b11();
    let status = engine.world.players.get(e).expect("player").status;
    assert_eq!(
        (status.hp, status.max_hp, status.sp, status.max_sp),
        (90, 90, 21, 21)
    );
}

/// ENG-STAT-04: a lower MaxHP clamps HP: `SP_HP`/`SP_SP` before the diff, then the diff's
/// MAXHP, MAXSP, HP, SP (captured double send, capture 1 lines 681-698).
#[test]
fn eng_stat_04_clamp_pushes_come_before_the_diff_and_again_in_it() {
    let (mut engine, e) = novice_b11();
    set_level(&mut engine, e, 1);
    engine.mark_dirty(e, false);
    let out = tick(&mut engine);
    assert_eq!(out[..2], [p(SP_HP, 40), p(SP_SP, 11)], "{out:?}");
    assert_eq!(
        out[out.len() - 4..],
        [p(SP_MAXHP, 40), p(SP_MAXSP, 11), p(SP_HP, 40), p(SP_SP, 11)],
        "{out:?}"
    );
    let status = engine.world.players.get(e).expect("player").status;
    assert_eq!((status.hp, status.max_hp), (40, 40));
}

/// ENG-STAT-10: two changes in one tick share one recalc; the follow-ups go out after the
/// diff, in the order queued; nothing is owed the next tick.
#[test]
fn eng_stat_10_one_recalc_then_the_followups() {
    let (mut engine, e) = novice_b11();
    engine.world.players.get_mut(e).expect("player").status.luk = 3;
    engine.mark_dirty(e, false);
    engine.queue_followup(e, FollowUp::ToSelf(p(9, 46)));
    set_level(&mut engine, e, 12);
    engine.mark_dirty(e, true);
    engine.queue_followup(e, FollowUp::HealToMax);
    engine.queue_followup(e, FollowUp::Save);
    assert_eq!(engine.stat_queue.len(), 1, "listed once");
    let out = tick(&mut engine);
    let luk_pushes = out
        .iter()
        .filter(|ev| matches!(ev, ServerEvent::CoupleStatus { var_id: 18, .. }))
        .count();
    assert_eq!(luk_pushes, 1, "one diff: {out:?}");
    let max_hp = engine.world.players.get(e).expect("player").status.max_hp;
    let max_sp = engine.world.players.get(e).expect("player").status.max_sp;
    let tail = &out[out.len() - 3..];
    assert_eq!(
        tail,
        [
            p(9, 46),
            p(SP_HP, i32::try_from(max_hp).expect("small")),
            p(SP_SP, i32::try_from(max_sp).expect("small")),
        ],
        "{out:?}"
    );
    let mut saves = Vec::new();
    engine.drain_save_requests(&mut saves);
    assert_eq!(saves, vec![char_id(1)]);
    assert!(tick(&mut engine).is_empty(), "the queue is drained");
}

/// Review I-3: a stat reset whose battle value stays the same (the new job bonus makes up
/// for it) still sends that stat's `0x0141`, so the window shows the new allocated base.
#[test]
fn a_reset_with_an_unchanged_battle_stat_still_sends_its_couple() {
    let (mut engine, e) = novice_b11();
    {
        let info = engine.world.players.get_mut(e).expect("player");
        info.status.str = 2;
    }
    engine.mark_dirty(e, false);
    tick(&mut engine);
    {
        // Novice job 10 adds +1 to every stat: STR 1 + 1 = the old battle 2.
        let info = engine.world.players.get_mut(e).expect("player");
        info.status.str = 1;
        info.status.job_level = 10;
    }
    engine.mark_dirty(e, false);
    let out = tick(&mut engine);
    assert!(
        out.contains(&ServerEvent::CoupleStatus {
            var_id: 13,
            base: 1,
            bonus: 1,
        }),
        "{out:?}"
    );
}

/// Review M-2 (catalog `mechanics/atcommands.md` "Heal pushes after the level-up"): the heal
/// pushes only what it raised, and nothing for a dead player.
#[test]
fn heal_to_max_pushes_only_what_it_raised() {
    let (mut engine, e) = novice_b11();
    engine.world.players.get_mut(e).expect("player").status.hp = 50;
    engine.queue_followup(e, FollowUp::HealToMax);
    assert_eq!(tick(&mut engine), vec![p(SP_HP, 90)], "SP was full");
    engine.world.players.get_mut(e).expect("player").status.hp = 0;
    engine.queue_followup(e, FollowUp::HealToMax);
    assert!(tick(&mut engine).is_empty(), "dead: no heal");
    let hp = engine.world.players.get(e).expect("player").status.hp;
    assert_eq!(hp, 0);
}

/// Spec §2.1 / plan M3: a logout in the same tick drops what was owed.
#[test]
fn logout_drops_the_pending_recalc() {
    let (mut engine, e) = novice_b11();
    set_level(&mut engine, e, 1);
    engine.mark_dirty(e, false);
    engine.queue_followup(e, FollowUp::Save);
    engine.remove_character(char_id(1)).expect("removed");
    assert!(tick(&mut engine).is_empty());
    let mut saves = Vec::new();
    engine.drain_save_requests(&mut saves);
    assert!(saves.is_empty());
}

/// A cross-map warp flushes what the tick owes before the player leaves: the diff goes out
/// first and the transfer carries the clamped HP.
#[test]
fn a_cross_map_warp_flushes_the_pending_recalc() {
    let (mut engine, e) = novice_b11();
    let warp = ro_db::NpcSpawn {
        name: "w".into(),
        unique_name: "w".into(),
        x: 10,
        y: 10,
        dir: 0,
        sprite: 45,
        sprite_is_mob: false,
        state: ro_db::NpcState::Normal,
        trigger: Some((0, 0)),
        duplicate_of: None,
        kind: ro_db::NpcKind::Warp(ro_db::WarpDest {
            map: "geffen".into(),
            x: 5,
            y: 5,
            trigger_while_hidden: false,
        }),
        source: "test:1".into(),
    };
    engine.spawn_npcs(&ro_db::MapNpcs { npcs: vec![warp] });
    set_level(&mut engine, e, 1);
    engine.mark_dirty(e, false);
    engine.push_intent(ro_core::SessionIntent::new(
        ro_core::SessionId(1),
        Some(AccountId(2_000_001)),
        Some(char_id(1)),
        ro_core::ClientIntent::NotifyActorInit,
    ));
    let out = tick(&mut engine);
    assert!(out.contains(&p(SP_MAXHP, 40)), "the diff went out: {out:?}");
    let mut transfers = Vec::new();
    engine.drain_transfers(&mut transfers);
    assert_eq!(transfers.len(), 1, "warped");
    assert_eq!(transfers[0].spawn.status.hp, 40);
}

#[test]
fn weapon_classes_follow_the_plan_mapping() {
    assert_eq!(weapon_class(WeaponType::Fist), WeaponClass::Fist);
    assert_eq!(weapon_class(WeaponType::Dagger), WeaponClass::Melee);
    assert_eq!(weapon_class(WeaponType::TwoHandStaff), WeaponClass::Melee);
    assert_eq!(weapon_class(WeaponType::Katar), WeaponClass::Katar);
    assert_eq!(weapon_class(WeaponType::Bow), WeaponClass::Bow);
    for ranged in [
        WeaponType::Musical,
        WeaponType::Whip,
        WeaponType::Revolver,
        WeaponType::Rifle,
        WeaponType::Gatling,
        WeaponType::Shotgun,
        WeaponType::Grenade,
    ] {
        assert_eq!(weapon_class(ranged), WeaponClass::Ranged, "{ranged:?}");
    }
}
