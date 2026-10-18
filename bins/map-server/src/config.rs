/// Configuration for `map-server` network ports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapConfig {
    pub client_port: u16,
    pub http_port: u16,
    pub advertised_port: u16,
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            client_port: 5121,
            http_port: 5122,
            advertised_port: 5121,
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

        Self {
            client_port,
            http_port,
            advertised_port,
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
}
