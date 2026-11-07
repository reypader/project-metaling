//! Stats pushed by gear changes (ENG-STAT-01..03).

use super::*;

/// ENG-STAT-02: a login wearing a Knife pushes `SP_ATK2` 17 and its `0x00bd` has
/// `atk_bonus` 17.
#[test]
fn eng_stat_02_login_push_and_window_carry_gear_values() {
    let mut engine = instance();
    let burst = engine
        .spawn_character(spawn_data(A, vec![worn(KNIFE, EQP_HAND_R)]), 0)
        .expect("spawn");
    let burst = self_events(burst, A);
    assert_eq!(param_of(&burst, SP_ATK2), Some(17));
    assert_eq!(attack_ranges(&burst), vec![1]);

    let init = self_events(actor_init(&mut engine, A, 10), A);
    let window = window(&init);
    assert_eq!(window.atk_bonus, 17);
    assert_eq!(window.hard_def, 0);
}

/// ENG-STAT-02 (MATK, capture 4): a Rod's `MagicAttack` is the weapon side, `SP_MATK1` and the
/// window's `matk_bonus`; the status side is the INT/DEX/LUK/level formula.
#[test]
fn eng_stat_02_weapon_matk_is_the_matk1_side() {
    let mut engine = instance();
    let burst = engine
        .spawn_character(spawn_data(A, vec![worn(ROD, EQP_HAND_R)]), 0)
        .expect("spawn");
    let burst = self_events(burst, A);
    assert_eq!(param_of(&burst, SP_MATK1), Some(30));
    assert_eq!(param_of(&burst, SP_MATK2), Some(STATUS_MATK));
    let window = window(&self_events(actor_init(&mut engine, A, 10), A));
    assert_eq!((window.matk_bonus, window.matk), (30, STATUS_MATK as i16));
}

/// ENG-STAT-01 in the flow: a Rod pushes ATK2, then both MATK vars (capture 4 line 1399).
#[test]
fn eng_stat_01_rod_pushes_the_matk_pair() {
    let mut engine = pair(vec![item(ROD)]);
    let out = seen_by(&equip(&mut engine, 2, EQP_HAND_R), A);
    assert_eq!(
        out,
        vec![
            ack(2, EQP_HAND_R, 0),
            hands(ROD, 0),
            param(SP_ATK2, 15),
            param(SP_MATK2, STATUS_MATK),
            param(SP_MATK1, 30),
        ]
    );
}

/// ENG-STAT-03: every equip change bumps the inventory revision, so autosave sees it.
#[test]
fn eng_stat_03_equip_changes_bump_the_revision() {
    let mut engine = pair(vec![item(KNIFE), worn(HAT, EQP_HEAD_TOP)]);
    let revision = |engine: &MapInstance| engine.inventory_revision(char_id(A)).expect("A");
    let start = revision(&engine);
    equip(&mut engine, 2, EQP_HAND_R);
    let after_equip = revision(&engine);
    assert!(after_equip > start);
    take_off(&mut engine, 2);
    let after_take_off = revision(&engine);
    assert!(after_take_off > after_equip);
    unequip_all(&mut engine, UNEQUIP_ALL_WINDOW);
    assert!(revision(&engine) > after_take_off);
    let refused = revision(&engine);
    equip(&mut engine, 9, EQP_HAND_R);
    assert_eq!(revision(&engine), refused, "a refusal changes nothing");
}
