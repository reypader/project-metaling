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

fn get_cli_arg(args: &[String], short: Option<&str>, long: &str) -> Option<u16> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if let Some(short_flag) = short {
            if arg == short_flag {
                if let Some(val) = iter.next() {
                    if let Ok(port) = val.parse::<u16>() {
                        return Some(port);
                    }
                }
            }
            let prefix = format!("{short_flag}=");
            if let Some(rest) = arg.strip_prefix(&prefix) {
                if let Ok(port) = rest.parse::<u16>() {
                    return Some(port);
                }
            }
        }
        if arg == long {
            if let Some(val) = iter.next() {
                if let Ok(port) = val.parse::<u16>() {
                    return Some(port);
                }
            }
        }
        let prefix = format!("{long}=");
        if let Some(rest) = arg.strip_prefix(&prefix) {
            if let Ok(port) = rest.parse::<u16>() {
                return Some(port);
            }
        }
    }
    None
}

fn get_cli_arg_string(args: &[String], short: Option<&str>, long: &str) -> Option<String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if let Some(short_flag) = short {
            if arg == short_flag {
                if let Some(val) = iter.next() {
                    return Some(val.clone());
                }
            }
            let prefix = format!("{short_flag}=");
            if let Some(rest) = arg.strip_prefix(&prefix) {
                return Some(rest.to_string());
            }
        }
        if arg == long {
            if let Some(val) = iter.next() {
                return Some(val.clone());
            }
        }
        let prefix = format!("{long}=");
        if let Some(rest) = arg.strip_prefix(&prefix) {
            return Some(rest.to_string());
        }
    }
    None
}

impl LoginConfig {
    pub fn from_args_and_env<I, S, F>(args: I, env: F) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        F: Fn(&str) -> Option<String>,
    {
        let args_vec: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
        let defaults = Self::default();

        let client_port = get_cli_arg(&args_vec, Some("-p"), "--port")
            .or_else(|| env("LOGIN_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.client_port);

        let http_port = get_cli_arg(&args_vec, None, "--http-port")
            .or_else(|| env("LOGIN_HTTP_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.http_port);

        let advertised_char_port = get_cli_arg(&args_vec, None, "--advertised-port")
            .or_else(|| get_cli_arg(&args_vec, None, "--char-port"))
            .or_else(|| get_cli_arg(&args_vec, None, "--advertised-char-port"))
            .or_else(|| env("CHAR_ADVERTISED_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.advertised_char_port);

        let database_url = get_cli_arg_string(&args_vec, None, "--database-url")
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
            std::time::Duration::from_secs(300),
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
