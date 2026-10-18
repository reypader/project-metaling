/// Configuration for `map-server` network ports and database connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapConfig {
    pub client_port: u16,
    pub http_port: u16,
    pub advertised_port: u16,
    pub database_url: String,
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            client_port: 5121,
            http_port: 5122,
            advertised_port: 5121,
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

impl MapConfig {
    pub fn from_args_and_env<I, S, F>(args: I, env: F) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        F: Fn(&str) -> Option<String>,
    {
        let args_vec: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
        let defaults = Self::default();

        let client_port = get_cli_arg(&args_vec, Some("-p"), "--port")
            .or_else(|| env("MAP_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.client_port);

        let http_port = get_cli_arg(&args_vec, None, "--http-port")
            .or_else(|| env("MAP_HTTP_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.http_port);

        let advertised_port = get_cli_arg(&args_vec, None, "--advertised-port")
            .or_else(|| env("MAP_ADVERTISED_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.advertised_port);

        let database_url = get_cli_arg_string(&args_vec, None, "--database-url")
            .or_else(|| env("DATABASE_URL"))
            .unwrap_or(defaults.database_url);

        Self {
            client_port,
            http_port,
            advertised_port,
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

    #[test]
    fn test_dom_bin_03_map_config_resolution() {
        let env_map: HashMap<&str, &str> = [
            ("MAP_PORT", "5131"),
            ("MAP_HTTP_PORT", "5132"),
            ("MAP_ADVERTISED_PORT", "5121"),
        ]
        .into_iter()
        .collect();

        let cfg = MapConfig::from_args_and_env(
            Vec::<String>::new(),
            |key| env_map.get(key).map(|v| v.to_string()),
        );

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
        let cfg = MapConfig::from_args_and_env(
            args,
            |key| env_map.get(key).map(|v| v.to_string()),
        );

        assert_eq!(cfg.client_port, 5150);
        assert_eq!(cfg.http_port, 5151);
        assert_eq!(cfg.advertised_port, 5121);
    }

    #[test]
    fn test_bin_03_map_advertised_port_decoupling() {
        let env_map: HashMap<&str, &str> = [
            ("MAP_PORT", "5131"),
            ("MAP_ADVERTISED_PORT", "5121"),
        ]
        .into_iter()
        .collect();

        let cfg = MapConfig::from_args_and_env(
            Vec::<String>::new(),
            |key| env_map.get(key).map(|v| v.to_string()),
        );

        assert_eq!(cfg.client_port, 5131, "bind port must be 5131");
        assert_eq!(cfg.advertised_port, 5121, "advertised port must be 5121");
    }

    #[test]
    fn test_bin_cfg_003_map_default_database_url() {
        let cfg = MapConfig::from_args_and_env(
            Vec::<String>::new(),
            |_| None,
        );
        assert_eq!(
            cfg.database_url,
            "postgres://ragnarok:ragnarok@localhost:5432/ragnarok"
        );
    }

    #[test]
    fn test_bin_cfg_persist_001_map_database_url_cli_over_env_over_default() {
        let env_map: HashMap<&str, &str> = [
            ("DATABASE_URL", "postgres://map_env_user:map_env_pass@env-host:5432/map_env_db"),
        ]
        .into_iter()
        .collect();

        let cfg_env = MapConfig::from_args_and_env(
            Vec::<String>::new(),
            |key| env_map.get(key).map(|v| v.to_string()),
        );
        assert_eq!(
            cfg_env.database_url,
            "postgres://map_env_user:map_env_pass@env-host:5432/map_env_db"
        );

        let args = vec![
            "map-server",
            "--database-url",
            "postgres://map_cli_user:map_cli_pass@cli-host:5432/map_cli_db",
        ];
        let cfg_cli = MapConfig::from_args_and_env(
            args,
            |key| env_map.get(key).map(|v| v.to_string()),
        );
        assert_eq!(
            cfg_cli.database_url,
            "postgres://map_cli_user:map_cli_pass@cli-host:5432/map_cli_db"
        );
    }
}
