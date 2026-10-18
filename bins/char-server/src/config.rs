/// Configuration for `char-server` network ports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharConfig {
    pub client_port: u16,
    pub http_port: u16,
    pub advertised_port: u16,
    pub default_map_port: u16,
}

impl Default for CharConfig {
    fn default() -> Self {
        Self {
            client_port: 6121,
            http_port: 6122,
            advertised_port: 6121,
            default_map_port: 5121,
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

impl CharConfig {
    pub fn from_args_and_env<I, S, F>(args: I, env: F) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        F: Fn(&str) -> Option<String>,
    {
        let args_vec: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
        let defaults = Self::default();

        let client_port = get_cli_arg(&args_vec, Some("-p"), "--port")
            .or_else(|| env("CHAR_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.client_port);

        let http_port = get_cli_arg(&args_vec, None, "--http-port")
            .or_else(|| env("CHAR_HTTP_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.http_port);

        let advertised_port = get_cli_arg(&args_vec, None, "--advertised-port")
            .or_else(|| env("CHAR_ADVERTISED_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.advertised_port);

        let default_map_port = get_cli_arg(&args_vec, None, "--map-port")
            .or_else(|| get_cli_arg(&args_vec, None, "--advertised-map-port"))
            .or_else(|| env("MAP_ADVERTISED_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.default_map_port);

        Self {
            client_port,
            http_port,
            advertised_port,
            default_map_port,
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
    fn test_dom_bin_02_char_config_resolution() {
        let env_map: HashMap<&str, &str> = [
            ("CHAR_PORT", "6131"),
            ("CHAR_HTTP_PORT", "6132"),
            ("CHAR_ADVERTISED_PORT", "6121"),
            ("MAP_ADVERTISED_PORT", "5121"),
        ]
        .into_iter()
        .collect();

        let cfg = CharConfig::from_args_and_env(
            Vec::<String>::new(),
            |key| env_map.get(key).map(|v| v.to_string()),
        );

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
        let cfg = CharConfig::from_args_and_env(
            args,
            |key| env_map.get(key).map(|v| v.to_string()),
        );

        assert_eq!(cfg.client_port, 6150);
        assert_eq!(cfg.http_port, 6151);
        assert_eq!(cfg.advertised_port, 6121);
    }

    #[test]
    fn test_bin_02_char_advertised_port_decoupling() {
        let env_map: HashMap<&str, &str> = [
            ("CHAR_PORT", "6131"),
            ("CHAR_ADVERTISED_PORT", "6121"),
        ]
        .into_iter()
        .collect();

        let cfg = CharConfig::from_args_and_env(
            Vec::<String>::new(),
            |key| env_map.get(key).map(|v| v.to_string()),
        );

        assert_eq!(cfg.client_port, 6131, "bind port must be 6131");
        assert_eq!(cfg.advertised_port, 6121, "advertised port must be 6121");
    }
}
