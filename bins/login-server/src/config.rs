/// Configuration for `login-server` network ports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginConfig {
    pub client_port: u16,
    pub http_port: u16,
    pub advertised_char_port: u16,
}

impl Default for LoginConfig {
    fn default() -> Self {
        Self {
            client_port: 6900,
            http_port: 6901,
            advertised_char_port: 6121,
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

        Self {
            client_port,
            http_port,
            advertised_char_port,
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
    use ro_persistence::InMemoryStubRepo;
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

        let repo = Arc::new(InMemoryStubRepo::new());
        let service = LoginService::with_ttl_and_char_port(
            repo,
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
}
