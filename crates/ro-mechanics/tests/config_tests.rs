//! `BattleConfig` defaults and YAML overrides (spec MECH-06; catalog
//! `mechanics/battle_config.md`).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use ro_mechanics::{BattleConfig, ClassFlags};

fn parse(text: &str) -> BattleConfig {
    serde_saphyr::from_str(text).expect("parses")
}

/// MECH-06: the defaults are rAthena's shipped values, not its compiled ones.
#[test]
fn mech_06_defaults_are_the_shipped_values() {
    let c = BattleConfig::default();
    assert_eq!(c.enable_baseatk_renewal, 0x29F);
    assert_eq!(c.enable_critical, 0x11);
    assert_eq!(c.enable_perfect_flee, 0x1);
    assert_eq!((c.min_hitrate, c.max_hitrate), (5, 100));
    assert_eq!((c.max_aspd, c.max_third_aspd), (190, 193));
    assert_eq!((c.max_extended_aspd, c.max_summoner_aspd), (193, 193));
    assert_eq!((c.hp_rate, c.sp_rate), (100, 100));
    assert_eq!(
        (c.max_hp_lv99, c.max_hp_lv150, c.max_hp),
        (330_000, 660_000, 1_100_000)
    );
    assert_eq!(c.max_sp, 1_000_000, "shipped, not the compiled 32500");
    assert_eq!(c.max_ap, 1000);
    assert_eq!(c.natural_heal_weight_rate, 70);
    assert_eq!(c.major_overweight_rate, 90);
    assert_eq!(c.max_baby_third_parameter, 117);
    assert_eq!(c.max_summoner_parameter, 130);
    assert_eq!(c.max_extended_parameter, 130);
    assert_eq!(c.max_fourth_parameter, 130);
    assert_eq!(c.max_third_trans_parameter, 130);
    assert_eq!(c.max_third_parameter, 130);
    assert_eq!(c.max_trait_parameter, 110);
    assert_eq!(c.start_status_points, 48);
    assert!(c.use_statpoint_table);
    assert!(!c.multi_level_up);
}

/// MECH-06: a partial file overrides only its keys; an unknown key is an error.
#[test]
fn mech_06_partial_yaml_overrides_only_its_keys() {
    let c = parse("hp_rate: 200\nmax_aspd: 195\n");
    assert_eq!((c.hp_rate, c.max_aspd), (200, 195));
    assert_eq!(c.max_sp, BattleConfig::default().max_sp);
    assert!(serde_saphyr::from_str::<BattleConfig>("max_spp: 1\n").is_err());
    assert_eq!(parse("{}\n"), BattleConfig::default());
}

/// MECH-06: ASPD caps convert to an amotion floor, `2000 - aspd * 10`, picked by class.
#[test]
fn mech_06_aspd_caps_convert() {
    let c = BattleConfig::default();
    assert_eq!(c.amotion_floor(&ClassFlags::default()), 100);
    let third = ClassFlags {
        third: true,
        ..ClassFlags::default()
    };
    assert_eq!(c.amotion_floor(&third), 70);
    let extended = ClassFlags {
        extended: true,
        ..ClassFlags::default()
    };
    assert_eq!(c.amotion_floor(&extended), 70);
    let custom = parse("max_aspd: 195\n");
    assert_eq!(custom.amotion_floor(&ClassFlags::default()), 50);
}

#[test]
fn validation_rejects_out_of_range_values() {
    assert!(BattleConfig::default().validate().is_ok());
    for bad in [
        "max_aspd: 200\n",
        "max_third_aspd: 99\n",
        "hp_rate: 0\n",
        "min_hitrate: 101\n",
        "max_parameter: 5\n",
        // HP/SP caps: 0 would clamp everyone to dead, and the save columns are `int`.
        "max_hp: 0\n",
        "max_sp: 2147483648\n",
        "max_hp_lv99: 3000000000\n",
        "max_hp_lv150: 0\n",
    ] {
        assert!(parse(bad).validate().is_err(), "{bad}");
    }
}

/// `config/battle.yml` writes every default out and parses back to them.
#[test]
fn shipped_battle_yml_equals_the_defaults() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/battle.yml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is committed: {e}", path.display()));
    assert_eq!(parse(&text), BattleConfig::default());
    for key in [
        "max_sp:",
        "max_third_aspd:",
        "start_status_points:",
        "max_trait_parameter:",
    ] {
        assert!(text.contains(key), "config/battle.yml lists {key}");
    }
}
