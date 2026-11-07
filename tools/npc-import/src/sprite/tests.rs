use super::*;

#[test]
fn tool_npc_05_c_numbers() {
    assert_eq!(parse_c_long("117"), Some(117));
    assert_eq!(parse_c_long("-1"), Some(-1));
    assert_eq!(parse_c_long("0x2D"), Some(45));
    assert_eq!(parse_c_long("010"), Some(8));
    assert_eq!(parse_c_long(""), Some(0));
    assert_eq!(parse_c_long("4_F_KAFRA1"), None);
    assert_eq!(parse_c_long("0"), Some(0));
}

#[test]
fn tool_npc_05_enum_counter_and_resets() {
    let hpp = "enum e_job_types\n{\n\tNPC_RANGE1_START = 44,\n\tJT_WARPNPC,\n\tJT_1_ETC_01, // c\n\t/* x, y */\n\tJT_HIDDEN_WARP_NPC = 139,\n\tJT_NEXT,\n\tJT_FAKENPC = -1\n};";
    let t = parse_job_enum(hpp).expect("parses");
    assert_eq!(t.get("warpnpc"), Some(&45));
    assert_eq!(t.get("1_etc_01"), Some(&46));
    assert_eq!(t.get("hidden_warp_npc"), Some(&139));
    assert_eq!(t.get("next"), Some(&140));
    assert_eq!(t.get("fakenpc"), Some(&-1));
    assert!(!t.contains_key("npc_range1_start"));
}
