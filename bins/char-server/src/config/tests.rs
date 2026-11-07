use std::collections::HashMap;

use super::*;

#[test]
fn test_dom_bin_02_char_config_resolution() {
    let env_map: HashMap<&str, &str> = [
        ("CHAR_PORT", "6131"),
        ("CHAR_HTTP_PORT", "6132"),
        ("CHAR_ADVERTISED_PORT", "6121"),
        ("MAP_ADVERTISED_PORT", "5121"),
    ]
    .into_iter()
    .collect();

    let cfg = CharConfig::from_args_and_env(Vec::<String>::new(), |key| {
        env_map.get(key).map(|v| v.to_string())
    });

    assert_eq!(cfg.client_port, 6131);
    assert_eq!(cfg.http_port, 6132);
    assert_eq!(cfg.advertised_port, 6121);
    assert_eq!(cfg.default_map_port, 5121);
}

#[test]
fn test_dom_bin_04_char_precedence_cli_over_env_over_default() {
    let env_map: HashMap<&str, &str> = [
        ("CHAR_PORT", "6140"),
        ("CHAR_HTTP_PORT", "6141"),
        ("CHAR_ADVERTISED_PORT", "6142"),
    ]
    .into_iter()
    .collect();

    let args = vec![
        "char-server",
        "--port",
        "6150",
        "--http-port",
        "6151",
        "--advertised-port",
        "6121",
    ];
    let cfg = CharConfig::from_args_and_env(args, |key| env_map.get(key).map(|v| v.to_string()));

    assert_eq!(cfg.client_port, 6150);
    assert_eq!(cfg.http_port, 6151);
    assert_eq!(cfg.advertised_port, 6121);
}

#[test]
fn test_bin_02_char_advertised_port_decoupling() {
    let env_map: HashMap<&str, &str> = [("CHAR_PORT", "6131"), ("CHAR_ADVERTISED_PORT", "6121")]
        .into_iter()
        .collect();

    let cfg = CharConfig::from_args_and_env(Vec::<String>::new(), |key| {
        env_map.get(key).map(|v| v.to_string())
    });

    assert_eq!(cfg.client_port, 6131, "bind port must be 6131");
    assert_eq!(cfg.advertised_port, 6121, "advertised port must be 6121");
}

#[test]
fn test_bin_cfg_002_char_default_database_url() {
    let cfg = CharConfig::from_args_and_env(Vec::<String>::new(), |_| None);
    assert_eq!(
        cfg.database_url,
        "postgres://ragnarok:ragnarok@localhost:5432/ragnarok"
    );
}

#[test]
fn test_bin_cfg_persist_001_char_database_url_cli_over_env_over_default() {
    let env_map: HashMap<&str, &str> = [(
        "DATABASE_URL",
        "postgres://char_env_user:char_env_pass@env-host:5432/char_env_db",
    )]
    .into_iter()
    .collect();

    let cfg_env = CharConfig::from_args_and_env(Vec::<String>::new(), |key| {
        env_map.get(key).map(|v| v.to_string())
    });
    assert_eq!(
        cfg_env.database_url,
        "postgres://char_env_user:char_env_pass@env-host:5432/char_env_db"
    );

    let args = vec![
        "char-server",
        "--database-url",
        "postgres://char_cli_user:char_cli_pass@cli-host:5432/char_cli_db",
    ];
    let cfg_cli =
        CharConfig::from_args_and_env(args, |key| env_map.get(key).map(|v| v.to_string()));
    assert_eq!(
        cfg_cli.database_url,
        "postgres://char_cli_user:char_cli_pass@cli-host:5432/char_cli_db"
    );
}
