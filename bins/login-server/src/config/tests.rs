use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ro_core::ServerEvent;
use ro_persistence::PostgresRepo;

use super::*;
use crate::service::LoginService;

#[test]
fn test_dom_bin_01_login_config_resolution() {
    let env_map: HashMap<&str, &str> = [
        ("LOGIN_PORT", "6910"),
        ("LOGIN_HTTP_PORT", "6911"),
        ("CHAR_ADVERTISED_PORT", "6121"),
    ]
    .into_iter()
    .collect();

    let cfg = LoginConfig::from_args_and_env(Vec::<String>::new(), |key| {
        env_map.get(key).map(|v| v.to_string())
    });

    assert_eq!(cfg.client_port, 6910);
    assert_eq!(cfg.http_port, 6911);
    assert_eq!(cfg.advertised_char_port, 6121);
}

#[test]
fn test_dom_bin_04_login_precedence_cli_over_env_over_default() {
    let env_map: HashMap<&str, &str> = [
        ("LOGIN_PORT", "6920"),
        ("LOGIN_HTTP_PORT", "6921"),
        ("CHAR_ADVERTISED_PORT", "6125"),
    ]
    .into_iter()
    .collect();

    let args = vec![
        "login-server",
        "--port",
        "6950",
        "--http-port",
        "6951",
        "--advertised-port",
        "6121",
    ];
    let cfg = LoginConfig::from_args_and_env(args, |key| env_map.get(key).map(|v| v.to_string()));

    assert_eq!(cfg.client_port, 6950);
    assert_eq!(cfg.http_port, 6951);
    assert_eq!(cfg.advertised_char_port, 6121);
}

#[test]
fn test_bin_04_login_server_custom_ports_bind() {
    let env_map: HashMap<&str, &str> = [("LOGIN_PORT", "6910"), ("LOGIN_HTTP_PORT", "6911")]
        .into_iter()
        .collect();

    let cfg = LoginConfig::from_args_and_env(Vec::<String>::new(), |key| {
        env_map.get(key).map(|v| v.to_string())
    });

    assert_eq!(cfg.client_port, 6910);
    assert_eq!(cfg.http_port, 6911);
}

#[tokio::test]
async fn test_bin_05_e2e_proxy_redirection_flow() {
    let env_map: HashMap<&str, &str> = [
        ("LOGIN_PORT", "6910"),
        ("LOGIN_HTTP_PORT", "6911"),
        ("CHAR_ADVERTISED_PORT", "6121"),
    ]
    .into_iter()
    .collect();

    let cfg = LoginConfig::from_args_and_env(Vec::<String>::new(), |key| {
        env_map.get(key).map(|v| v.to_string())
    });

    assert_eq!(
        cfg.client_port, 6910,
        "login-server binds to 6910 for proxy"
    );
    assert_eq!(
        cfg.advertised_char_port, 6121,
        "advertises 6121 proxy port for char-server"
    );

    let test_db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string());
    let repo = PostgresRepo::connect(&test_db_url).await.unwrap();
    let _ = repo.seed_test_data().await;
    let service = LoginService::with_ttl_and_char_port(
        Arc::new(repo),
        Duration::from_secs(300),
        cfg.advertised_char_port,
    );
    let event = service
        .process_login("test", "password", 0x80000000, 0)
        .await;
    match event {
        ServerEvent::LoginAccepted { char_servers, .. } => {
            assert!(!char_servers.is_empty());
            assert_eq!(
                char_servers[0].port, 6121,
                "client is directed to char proxy port 6121"
            );
        }
        other => panic!("expected LoginAccepted, got {:?}", other),
    }
}

#[test]
fn test_bin_cfg_001_login_default_database_url() {
    let cfg = LoginConfig::from_args_and_env(Vec::<String>::new(), |_| None);
    assert_eq!(
        cfg.database_url,
        "postgres://ragnarok:ragnarok@localhost:5432/ragnarok"
    );
}

#[test]
fn test_bin_cfg_persist_001_login_database_url_cli_over_env_over_default() {
    let env_map: HashMap<&str, &str> = [(
        "DATABASE_URL",
        "postgres://env_user:env_pass@env-host:5432/env_db",
    )]
    .into_iter()
    .collect();

    let cfg_env = LoginConfig::from_args_and_env(Vec::<String>::new(), |key| {
        env_map.get(key).map(|v| v.to_string())
    });
    assert_eq!(
        cfg_env.database_url,
        "postgres://env_user:env_pass@env-host:5432/env_db"
    );

    let args = vec![
        "login-server",
        "--database-url",
        "postgres://cli_user:cli_pass@cli-host:5432/cli_db",
    ];
    let cfg_cli =
        LoginConfig::from_args_and_env(args, |key| env_map.get(key).map(|v| v.to_string()));
    assert_eq!(
        cfg_cli.database_url,
        "postgres://cli_user:cli_pass@cli-host:5432/cli_db"
    );
}
