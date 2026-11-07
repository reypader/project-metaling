//! Command-line arguments of the proxy.

use std::env;

use anyhow::{Context, Result};
use clap::Parser;

use crate::config::{
    resolve_target, ProxyConfig, DEFAULT_CHAR_PORT, DEFAULT_LOGIN_PORT, DEFAULT_MAP_PORT,
};

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

    /// Log every payload byte instead of the first 32 (or set PROXY_FULL_PAYLOAD=1)
    #[arg(
        long = "full-payload",
        env = "PROXY_FULL_PAYLOAD",
        action = clap::ArgAction::SetTrue,
        value_parser = clap::builder::BoolishValueParser::new()
    )]
    pub full_payload: bool,
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
            full_payload: self.full_payload,
        })
    }
}
