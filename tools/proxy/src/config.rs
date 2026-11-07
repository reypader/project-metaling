use std::net::Ipv6Addr;

use anyhow::{Context, Result};

pub const DEFAULT_LOGIN_PORT: u16 = 6900;
pub const DEFAULT_CHAR_PORT: u16 = 6121;
pub const DEFAULT_MAP_PORT: u16 = 5121;

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
    pub full_payload: bool,
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

#[cfg(test)]
mod tests;
