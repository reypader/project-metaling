//! Map name helpers (spec CORE-MAP-01).

use ro_core::{bare_map_name, gat_map_name};

#[test]
fn core_map_01_gat_map_name_appends_once() {
    assert_eq!(gat_map_name("prontera"), "prontera.gat");
    assert_eq!(gat_map_name("prontera.gat"), "prontera.gat");
}

#[test]
fn core_map_01_bare_map_name_strips_once() {
    assert_eq!(bare_map_name("prontera.gat"), "prontera");
    assert_eq!(bare_map_name("prontera"), "prontera");
    assert_eq!(bare_map_name("x.gat.gat"), "x.gat");
}

#[test]
fn core_map_01_case_sensitive() {
    assert_eq!(gat_map_name("prontera.GAT"), "prontera.GAT.gat");
    assert_eq!(bare_map_name("prontera.GAT"), "prontera.GAT");
}

#[test]
fn core_map_01_round_trip() {
    for bare in ["prontera", "prt_fild08", "1@tower", "a"] {
        assert_eq!(bare_map_name(&gat_map_name(bare)), bare);
    }
}
