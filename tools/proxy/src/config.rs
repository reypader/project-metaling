use clap::Parser;

pub const DEFAULT_LOGIN_PORT: u16 = 6900;
pub const DEFAULT_CHAR_PORT: u16 = 6121;
pub const DEFAULT_MAP_PORT: u16 = 5121;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "proxy",
    about = "Ragnarok Online packet logging & forwarding proxy",
    version
)]
pub struct ProxyArgs {
    /// Configurable target IP/host where servers are running (used when target host is not explicitly specified)
    #[arg(short = 't', short_alias = 'i', long = "target-ip", alias = "ip", default_value = "127.0.0.1")]
    pub target_ip: String,

    /// Login server target in "host:port" format (defaults to <target-ip>:6900)
    #[arg(short = 'l', long = "login")]
    pub login: Option<String>,

    /// Char server target in "host:port" format (defaults to <target-ip>:6121)
    #[arg(short = 'c', long = "char")]
    pub char_server: Option<String>,

    /// Map server target in "host:port" format (defaults to <target-ip>:5121)
    #[arg(short = 'm', long = "map")]
    pub map_server: Option<String>,

    /// Address to bind proxy listeners
    #[arg(long = "bind", default_value = "0.0.0.0")]
    pub bind_addr: String,

    /// Optional override for proxy login listen port (defaults to 6900)
    #[arg(long = "listen-login-port", default_value_t = DEFAULT_LOGIN_PORT)]
    pub listen_login_port: u16,

    /// Optional override for proxy char listen port (defaults to 6121)
    #[arg(long = "listen-char-port", default_value_t = DEFAULT_CHAR_PORT)]
    pub listen_char_port: u16,

    /// Optional override for proxy map listen port (defaults to 5121)
    #[arg(long = "listen-map-port", default_value_t = DEFAULT_MAP_PORT)]
    pub listen_map_port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    pub bind_addr: String,
    pub target_ip: String,
    pub login_listen_port: u16,
    pub char_listen_port: u16,
    pub map_listen_port: u16,
    pub login_target: String,
    pub char_target: String,
    pub map_target: String,
}

pub fn resolve_target(
    input: Option<&str>,
    default_ip: &str,
    default_port: u16,
) -> Result<String, String> {
    match input {
        Some(s) if !s.trim().is_empty() => {
            let s = s.trim();
            if let Some((host, port_str)) = s.split_once(':') {
                let host = if host.is_empty() { default_ip } else { host };
                let port: u16 = port_str
                    .parse()
                    .map_err(|e| format!("Invalid port in '{}': {}", s, e))?;
                Ok(format!("{}:{}", host, port))
            } else if let Ok(port) = s.parse::<u16>() {
                Ok(format!("{}:{}", default_ip, port))
            } else {
                Ok(format!("{}:{}", s, default_port))
            }
        }
        _ => Ok(format!("{}:{}", default_ip, default_port)),
    }
}

impl ProxyArgs {
    pub fn build_config(&self) -> Result<ProxyConfig, String> {
        let login_target = resolve_target(self.login.as_deref(), &self.target_ip, DEFAULT_LOGIN_PORT)?;
        let char_target = resolve_target(self.char_server.as_deref(), &self.target_ip, DEFAULT_CHAR_PORT)?;
        let map_target = resolve_target(self.map_server.as_deref(), &self.target_ip, DEFAULT_MAP_PORT)?;

        Ok(ProxyConfig {
            bind_addr: self.bind_addr.clone(),
            target_ip: self.target_ip.clone(),
            login_listen_port: self.listen_login_port,
            char_listen_port: self.listen_char_port,
            map_listen_port: self.listen_map_port,
            login_target,
            char_target,
            map_target,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_target_defaults() {
        let res = resolve_target(None, "127.0.0.1", 6900).unwrap();
        assert_eq!(res, "127.0.0.1:6900");

        let res = resolve_target(None, "192.168.1.100", 6121).unwrap();
        assert_eq!(res, "192.168.1.100:6121");
    }

    #[test]
    fn test_resolve_target_host_and_port() {
        let res = resolve_target(Some("10.0.0.1:7000"), "127.0.0.1", 6900).unwrap();
        assert_eq!(res, "10.0.0.1:7000");

        let res = resolve_target(Some("rathena.local:6121"), "127.0.0.1", 6121).unwrap();
        assert_eq!(res, "rathena.local:6121");
    }

    #[test]
    fn test_resolve_target_port_only() {
        let res = resolve_target(Some(":7000"), "192.168.1.50", 6900).unwrap();
        assert_eq!(res, "192.168.1.50:7000");

        let res = resolve_target(Some("7000"), "192.168.1.50", 6900).unwrap();
        assert_eq!(res, "192.168.1.50:7000");
    }

    #[test]
    fn test_resolve_target_host_only() {
        let res = resolve_target(Some("remote-server"), "127.0.0.1", 5121).unwrap();
        assert_eq!(res, "remote-server:5121");
    }

    #[test]
    fn test_resolve_target_invalid_port() {
        let res = resolve_target(Some("host:abc"), "127.0.0.1", 5121);
        assert!(res.is_err());
    }

    #[test]
    fn test_build_config_from_args() {
        let args = ProxyArgs {
            target_ip: "192.168.1.200".to_string(),
            login: None,
            char_server: Some("10.0.0.2:6125".to_string()),
            map_server: Some("10.0.0.3:5125".to_string()),
            bind_addr: "0.0.0.0".to_string(),
            listen_login_port: 6900,
            listen_char_port: 6121,
            listen_map_port: 5121,
        };

        let config = args.build_config().unwrap();
        assert_eq!(config.login_target, "192.168.1.200:6900");
        assert_eq!(config.char_target, "10.0.0.2:6125");
        assert_eq!(config.map_target, "10.0.0.3:5125");
    }
}
