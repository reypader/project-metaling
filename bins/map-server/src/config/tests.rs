use std::collections::HashMap;

use super::*;

#[test]
fn test_dom_bin_03_map_config_resolution() {
    let env_map: HashMap<&str, &str> = [
        ("MAP_PORT", "5131"),
        ("MAP_HTTP_PORT", "5132"),
        ("MAP_ADVERTISED_PORT", "5121"),
    ]
    .into_iter()
    .collect();

    let cfg = MapConfig::from_args_and_env(Vec::<String>::new(), |key| {
        env_map.get(key).map(|v| v.to_string())
    });

    assert_eq!(cfg.client_port, 5131);
    assert_eq!(cfg.http_port, 5132);
    assert_eq!(cfg.advertised_port, 5121);
}

#[test]
fn test_dom_bin_04_map_precedence_cli_over_env_over_default() {
    let env_map: HashMap<&str, &str> = [
        ("MAP_PORT", "5140"),
        ("MAP_HTTP_PORT", "5141"),
        ("MAP_ADVERTISED_PORT", "5142"),
    ]
    .into_iter()
    .collect();

    let args = vec![
        "map-server",
        "--port",
        "5150",
        "--http-port",
        "5151",
        "--advertised-port",
        "5121",
    ];
    let cfg = MapConfig::from_args_and_env(args, |key| env_map.get(key).map(|v| v.to_string()));

    assert_eq!(cfg.client_port, 5150);
    assert_eq!(cfg.http_port, 5151);
    assert_eq!(cfg.advertised_port, 5121);
}

#[test]
fn test_bin_03_map_advertised_port_decoupling() {
    let env_map: HashMap<&str, &str> = [("MAP_PORT", "5131"), ("MAP_ADVERTISED_PORT", "5121")]
        .into_iter()
        .collect();

    let cfg = MapConfig::from_args_and_env(Vec::<String>::new(), |key| {
        env_map.get(key).map(|v| v.to_string())
    });

    assert_eq!(cfg.client_port, 5131, "bind port must be 5131");
    assert_eq!(cfg.advertised_port, 5121, "advertised port must be 5121");
}

#[test]
fn test_bin_cfg_003_map_default_database_url() {
    let cfg = MapConfig::from_args_and_env(Vec::<String>::new(), |_| None);
    assert_eq!(
        cfg.database_url,
        "postgres://ragnarok:ragnarok@localhost:5432/ragnarok"
    );
}

#[test]
fn test_bin_cfg_persist_001_map_database_url_cli_over_env_over_default() {
    let env_map: HashMap<&str, &str> = [(
        "DATABASE_URL",
        "postgres://map_env_user:map_env_pass@env-host:5432/map_env_db",
    )]
    .into_iter()
    .collect();

    let cfg_env = MapConfig::from_args_and_env(Vec::<String>::new(), |key| {
        env_map.get(key).map(|v| v.to_string())
    });
    assert_eq!(
        cfg_env.database_url,
        "postgres://map_env_user:map_env_pass@env-host:5432/map_env_db"
    );

    let args = vec![
        "map-server",
        "--database-url",
        "postgres://map_cli_user:map_cli_pass@cli-host:5432/map_cli_db",
    ];
    let cfg_cli = MapConfig::from_args_and_env(args, |key| env_map.get(key).map(|v| v.to_string()));
    assert_eq!(
        cfg_cli.database_url,
        "postgres://map_cli_user:map_cli_pass@cli-host:5432/map_cli_db"
    );
}
