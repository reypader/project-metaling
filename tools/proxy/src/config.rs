use std::{env, net::Ipv6Addr};

use anyhow::{Context, Result};
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
    #[arg(
        short = 't',
        short_alias = 'i',
        long = "target-ip",
        alias = "ip",
        default_value = "127.0.0.1"
    )]
    pub target_ip: String,

    /// Login server target in "host:port" format (defaults to PROXY_LOGIN_TARGET or <target-ip>:6900)
    #[arg(short = 'l', long = "login", env = "PROXY_LOGIN_TARGET")]
    pub login: Option<String>,

    /// Char server target in "host:port" format (defaults to PROXY_CHAR_TARGET or <target-ip>:6121)
    #[arg(short = 'c', long = "char", env = "PROXY_CHAR_TARGET")]
    pub char_server: Option<String>,

    /// Map server target in "host:port" format (defaults to PROXY_MAP_TARGET or <target-ip>:5121)
    #[arg(short = 'm', long = "map", env = "PROXY_MAP_TARGET")]
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

    /// Optional output log file path (defaults to packet_sequence-<date>.log)
    #[arg(short = 'o', long = "log-file")]
    pub log_file: Option<String>,
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
    pub log_file: Option<String>,
}

/// Formats host and port into a valid network address string.
/// IPv6 addresses are enclosed in brackets if not already bracketed.
pub fn format_host_port(host: &str, port: u16) -> String {
    let host = host.trim();
    if host.is_empty() {
        return format!("127.0.0.1:{}", port);
    }
    if host.starts_with('[') && host.ends_with(']') {
        let inner = host[1..host.len() - 1].trim();
        if inner.is_empty() {
            return format!("127.0.0.1:{}", port);
        }
        return format!("{}:{}", host, port);
    }
    if host.parse::<Ipv6Addr>().is_ok() || (host.contains(':') && !host.contains('.')) {
        format!("[{}]:{}", host, port)
    } else {
        format!("{}:{}", host, port)
    }
}

pub fn resolve_target(input: Option<&str>, default_ip: &str, default_port: u16) -> Result<String> {
    match input {
        Some(s) if !s.trim().is_empty() => {
            let s = s.trim();

            if s.starts_with('[') {
                if let Some(bracket_end) = s.find(']') {
                    let host = s[1..bracket_end].trim();
                    if host.is_empty() {
                        anyhow::bail!("Empty host inside IPv6 brackets in '{}'", s);
                    }
                    let after = &s[bracket_end + 1..];
                    if after.is_empty() {
                        return Ok(format!("[{}]:{}", host, default_port));
                    }
                    if let Some(port_str) = after.strip_prefix(':') {
                        let port: u16 = port_str
                            .parse()
                            .with_context(|| format!("Invalid port in '{}'", s))?;
                        return Ok(format!("[{}]:{}", host, port));
                    }
                    anyhow::bail!("Invalid characters after IPv6 bracket in '{}'", s);
                } else {
                    anyhow::bail!("Unclosed bracket in '{}'", s);
                }
            }

            if let Ok(ipv6) = s.parse::<Ipv6Addr>() {
                return Ok(format!("[{}]:{}", ipv6, default_port));
            }

            if let Some(port_str) = s.strip_prefix(':') {
                let port: u16 = port_str
                    .parse()
                    .with_context(|| format!("Invalid port in '{}'", s))?;
                return Ok(format_host_port(default_ip, port));
            }

            if let Ok(port) = s.parse::<u16>() {
                return Ok(format_host_port(default_ip, port));
            }

            if let Some((host, port_str)) = s.rsplit_once(':') {
                let host = if host.is_empty() { default_ip } else { host };
                let port: u16 = port_str
                    .parse()
                    .with_context(|| format!("Invalid port in '{}'", s))?;
                return Ok(format_host_port(host, port));
            }

            Ok(format_host_port(s, default_port))
        }
        _ => Ok(format_host_port(default_ip, default_port)),
    }
}

impl ProxyArgs {
    pub fn build_config(&self) -> Result<ProxyConfig> {
        let login_env = env::var("PROXY_LOGIN_TARGET").ok();
        let login_input = self
            .login
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| login_env.as_deref().filter(|s| !s.trim().is_empty()));
        let login_target = resolve_target(login_input, &self.target_ip, DEFAULT_LOGIN_PORT)
            .context("Failed to resolve Login server target address")?;

        let char_env = env::var("PROXY_CHAR_TARGET").ok();
        let char_input = self
            .char_server
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| char_env.as_deref().filter(|s| !s.trim().is_empty()));
        let char_target = resolve_target(char_input, &self.target_ip, DEFAULT_CHAR_PORT)
            .context("Failed to resolve Char server target address")?;

        let map_env = env::var("PROXY_MAP_TARGET").ok();
        let map_input = self
            .map_server
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| map_env.as_deref().filter(|s| !s.trim().is_empty()));
        let map_target = resolve_target(map_input, &self.target_ip, DEFAULT_MAP_PORT)
            .context("Failed to resolve Map server target address")?;

        let log_file = self
            .log_file
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);

        Ok(ProxyConfig {
            bind_addr: self.bind_addr.clone(),
            target_ip: self.target_ip.clone(),
            login_listen_port: self.listen_login_port,
            char_listen_port: self.listen_char_port,
            map_listen_port: self.listen_map_port,
            login_target,
            char_target,
            map_target,
            log_file,
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
            log_file: None,
        };

        let config = args.build_config().unwrap();
        assert_eq!(config.login_target, "192.168.1.200:6900");
        assert_eq!(config.char_target, "10.0.0.2:6125");
        assert_eq!(config.map_target, "10.0.0.3:5125");
        assert_eq!(config.log_file, None);
    }

    #[test]
    fn test_build_config_with_custom_log_file() {
        let args = ProxyArgs {
            target_ip: "127.0.0.1".to_string(),
            login: None,
            char_server: None,
            map_server: None,
            bind_addr: "127.0.0.1".to_string(),
            listen_login_port: 6900,
            listen_char_port: 6121,
            listen_map_port: 5121,
            log_file: Some("/tmp/custom_proxy.log".to_string()),
        };

        let config = args.build_config().unwrap();
        assert_eq!(config.log_file.as_deref(), Some("/tmp/custom_proxy.log"));
    }

    #[test]
    fn test_build_config_with_empty_log_file() {
        let args = ProxyArgs {
            target_ip: "127.0.0.1".to_string(),
            login: None,
            char_server: None,
            map_server: None,
            bind_addr: "127.0.0.1".to_string(),
            listen_login_port: 6900,
            listen_char_port: 6121,
            listen_map_port: 5121,
            log_file: Some("   ".to_string()),
        };

        let config = args.build_config().unwrap();
        assert_eq!(config.log_file, None);
    }

    #[test]
    fn test_format_host_port_ipv4_and_ipv6() {
        assert_eq!(format_host_port("127.0.0.1", 6900), "127.0.0.1:6900");
        assert_eq!(format_host_port("localhost", 6900), "localhost:6900");
        assert_eq!(format_host_port("::1", 6900), "[::1]:6900");
        assert_eq!(format_host_port("[::1]", 6900), "[::1]:6900");
        assert_eq!(format_host_port("2001:db8::1", 5121), "[2001:db8::1]:5121");
        assert_eq!(format_host_port("", 6900), "127.0.0.1:6900");
    }

    #[test]
    fn test_resolve_target_ipv6_bracketed_with_port() {
        let res = resolve_target(Some("[::1]:7000"), "127.0.0.1", 6900).unwrap();
        assert_eq!(res, "[::1]:7000");

        let res = resolve_target(Some("[2001:db8::1]:8000"), "127.0.0.1", 6900).unwrap();
        assert_eq!(res, "[2001:db8::1]:8000");
    }

    #[test]
    fn test_resolve_target_ipv6_bracketed_without_port() {
        let res = resolve_target(Some("[::1]"), "127.0.0.1", 6900).unwrap();
        assert_eq!(res, "[::1]:6900");

        let res = resolve_target(Some("[2001:db8::1]"), "127.0.0.1", 5121).unwrap();
        assert_eq!(res, "[2001:db8::1]:5121");
    }

    #[test]
    fn test_resolve_target_ipv6_unbracketed_without_port() {
        let res = resolve_target(Some("::1"), "127.0.0.1", 6900).unwrap();
        assert_eq!(res, "[::1]:6900");

        let res = resolve_target(Some("2001:db8::1"), "127.0.0.1", 5121).unwrap();
        assert_eq!(res, "[2001:db8::1]:5121");
    }

    #[test]
    fn test_resolve_target_ipv6_default_ip() {
        let res = resolve_target(None, "::1", 6900).unwrap();
        assert_eq!(res, "[::1]:6900");

        let res = resolve_target(Some("7000"), "::1", 6900).unwrap();
        assert_eq!(res, "[::1]:7000");

        let res = resolve_target(Some(":7000"), "::1", 6900).unwrap();
        assert_eq!(res, "[::1]:7000");
    }

    #[test]
    fn test_resolve_target_ipv6_invalid_formats() {
        let res = resolve_target(Some("[::1]:abc"), "127.0.0.1", 6900);
        assert!(res.is_err());

        let res = resolve_target(Some("[::1"), "127.0.0.1", 6900);
        assert!(res.is_err());

        let res = resolve_target(Some("[::1]extra"), "127.0.0.1", 6900);
        assert!(res.is_err());

        let res = resolve_target(Some("[]"), "127.0.0.1", 6900);
        assert!(res.is_err());

        let res = resolve_target(Some("[]:6900"), "127.0.0.1", 6900);
        assert!(res.is_err());
    }

    #[test]
    fn test_build_config_with_ipv6_target() {
        let args = ProxyArgs {
            target_ip: "::1".to_string(),
            login: None,
            char_server: Some("[::1]:6125".to_string()),
            map_server: Some("::1".to_string()),
            bind_addr: "[::]".to_string(),
            listen_login_port: 6900,
            listen_char_port: 6121,
            listen_map_port: 5121,
            log_file: None,
        };

        let config = args.build_config().unwrap();
        assert_eq!(config.login_target, "[::1]:6900");
        assert_eq!(config.char_target, "[::1]:6125");
        assert_eq!(config.map_target, "[::1]:5121");
    }
}
