use clap::{error::ErrorKind, Parser};

/// Configuration for `login-server` network ports and database connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginConfig {
    pub client_port: u16,
    pub http_port: u16,
    pub advertised_char_port: u16,
    pub database_url: String,
}

impl Default for LoginConfig {
    fn default() -> Self {
        Self {
            client_port: 6900,
            http_port: 6901,
            advertised_char_port: 6121,
            database_url: "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string(),
        }
    }
}

/// Command-line arguments for `login-server`.
#[derive(Parser, Debug, Default)]
#[command(name = "login-server", about = "Metaling login server")]
struct LoginCliArgs {
    /// Client TCP listening port.
    #[arg(short = 'p', long = "port")]
    port: Option<u16>,

    /// Internal HTTP API listening port.
    #[arg(long = "http-port")]
    http_port: Option<u16>,

    /// Port advertised to clients for character server redirection.
    #[arg(
        long = "advertised-port",
        visible_alias = "char-port",
        visible_alias = "advertised-char-port"
    )]
    advertised_char_port: Option<u16>,

    /// PostgreSQL database connection URL.
    #[arg(long = "database-url", visible_alias = "account-database-url")]
    database_url: Option<String>,

    /// Host advertised to clients for character server redirection.
    #[arg(long = "char-host")]
    char_host: Option<String>,
}

impl LoginConfig {
    pub fn from_args_and_env<I, S, F>(args: I, env: F) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        F: Fn(&str) -> Option<String>,
    {
        let mut args_vec: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
        if args_vec.is_empty() || args_vec[0].starts_with('-') {
            args_vec.insert(0, "login-server".to_string());
        }

        let parsed = match LoginCliArgs::try_parse_from(&args_vec) {
            Ok(args) => args,
            Err(err) => match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => err.exit(),
                _ => LoginCliArgs::default(),
            },
        };
        let defaults = Self::default();

        let client_port = parsed
            .port
            .or_else(|| env("LOGIN_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.client_port);

        let http_port = parsed
            .http_port
            .or_else(|| env("LOGIN_HTTP_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.http_port);

        let advertised_char_port = parsed
            .advertised_char_port
            .or_else(|| env("CHAR_ADVERTISED_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.advertised_char_port);

        let database_url = parsed
            .database_url
            .or_else(|| env("DATABASE_URL"))
            .unwrap_or(defaults.database_url);

        Self {
            client_port,
            http_port,
            advertised_char_port,
            database_url,
        }
    }

    pub fn from_env() -> Self {
        Self::from_args_and_env(std::env::args(), |key| std::env::var(key).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;
    use ro_contracts::packet::ServerEvent;
    use ro_persistence::PostgresRepo;
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

        let cfg = LoginConfig::from_args_and_env(
            Vec::<String>::new(),
            |key| env_map.get(key).map(|v| v.to_string()),
        );

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
        let cfg = LoginConfig::from_args_and_env(
            args,
            |key| env_map.get(key).map(|v| v.to_string()),
        );

        assert_eq!(cfg.client_port, 6950);
        assert_eq!(cfg.http_port, 6951);
        assert_eq!(cfg.advertised_char_port, 6121);
    }

    #[test]
    fn test_bin_04_login_server_custom_ports_bind() {
        let env_map: HashMap<&str, &str> = [
            ("LOGIN_PORT", "6910"),
            ("LOGIN_HTTP_PORT", "6911"),
        ]
        .into_iter()
        .collect();

        let cfg = LoginConfig::from_args_and_env(
            Vec::<String>::new(),
            |key| env_map.get(key).map(|v| v.to_string()),
        );

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

        let cfg = LoginConfig::from_args_and_env(
            Vec::<String>::new(),
            |key| env_map.get(key).map(|v| v.to_string()),
        );

        assert_eq!(cfg.client_port, 6910, "login-server binds to 6910 for proxy");
        assert_eq!(cfg.advertised_char_port, 6121, "advertises 6121 proxy port for char-server");

        let test_db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string());
        let repo = PostgresRepo::connect(&test_db_url).await.unwrap();
        let _ = repo.seed_test_data().await;
        let service = LoginService::with_ttl_and_char_port(
            Arc::new(repo),
            Duration::from_secs(300),
            cfg.advertised_char_port,
        );
        let event = service.process_login("test", "password", 0x80000000, 0).await;
        match event {
            ServerEvent::LoginAccepted { char_servers, .. } => {
                assert!(!char_servers.is_empty());
                assert_eq!(char_servers[0].port, 6121, "client is directed to char proxy port 6121");
            }
            other => panic!("expected LoginAccepted, got {:?}", other),
        }
    }

    #[test]
    fn test_bin_cfg_001_login_default_database_url() {
        let cfg = LoginConfig::from_args_and_env(
            Vec::<String>::new(),
            |_| None,
        );
        assert_eq!(
            cfg.database_url,
            "postgres://ragnarok:ragnarok@localhost:5432/ragnarok"
        );
    }

    #[test]
    fn test_bin_cfg_persist_001_login_database_url_cli_over_env_over_default() {
        let env_map: HashMap<&str, &str> = [
            ("DATABASE_URL", "postgres://env_user:env_pass@env-host:5432/env_db"),
        ]
        .into_iter()
        .collect();

        let cfg_env = LoginConfig::from_args_and_env(
            Vec::<String>::new(),
            |key| env_map.get(key).map(|v| v.to_string()),
        );
        assert_eq!(cfg_env.database_url, "postgres://env_user:env_pass@env-host:5432/env_db");

        let args = vec![
            "login-server",
            "--database-url",
            "postgres://cli_user:cli_pass@cli-host:5432/cli_db",
        ];
        let cfg_cli = LoginConfig::from_args_and_env(
            args,
            |key| env_map.get(key).map(|v| v.to_string()),
        );
        assert_eq!(cfg_cli.database_url, "postgres://cli_user:cli_pass@cli-host:5432/cli_db");
    }
}
