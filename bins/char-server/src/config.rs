use clap::{error::ErrorKind, Parser};

/// Configuration for `char-server` network ports and database connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharConfig {
    pub client_port: u16,
    pub http_port: u16,
    pub advertised_port: u16,
    pub default_map_port: u16,
    pub database_url: String,
}

impl Default for CharConfig {
    fn default() -> Self {
        Self {
            client_port: 6121,
            http_port: 6122,
            advertised_port: 6121,
            default_map_port: 5121,
            database_url: "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string(),
        }
    }
}

/// Command-line arguments for `char-server`.
#[derive(Parser, Debug, Default)]
#[command(name = "char-server", about = "Metaling character server")]
struct CharCliArgs {
    /// Client TCP listening port.
    #[arg(short = 'p', long = "port")]
    port: Option<u16>,

    /// Internal HTTP API listening port.
    #[arg(long = "http-port")]
    http_port: Option<u16>,

    /// Port advertised to clients for character server connections.
    #[arg(long = "advertised-port", visible_alias = "char-port")]
    advertised_port: Option<u16>,

    /// Port advertised for map server redirection.
    #[arg(long = "map-port", visible_alias = "advertised-map-port")]
    default_map_port: Option<u16>,

    /// PostgreSQL database connection URL.
    #[arg(
        long = "database-url",
        visible_alias = "char-database-url",
        visible_alias = "account-database-url"
    )]
    database_url: Option<String>,

    /// Character server client host.
    #[arg(long = "char-host")]
    char_host: Option<String>,

    /// Map server client host.
    #[arg(long = "map-host")]
    map_host: Option<String>,
}

impl CharConfig {
    pub fn from_args_and_env<I, S, F>(args: I, env: F) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        F: Fn(&str) -> Option<String>,
    {
        let mut args_vec: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
        if args_vec.is_empty() || args_vec[0].starts_with('-') {
            args_vec.insert(0, "char-server".to_string());
        }

        let parsed = match CharCliArgs::try_parse_from(&args_vec) {
            Ok(args) => args,
            Err(err) => match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => err.exit(),
                _ => CharCliArgs::default(),
            },
        };
        let defaults = Self::default();

        let client_port = parsed
            .port
            .or_else(|| env("CHAR_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.client_port);

        let http_port = parsed
            .http_port
            .or_else(|| env("CHAR_HTTP_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.http_port);

        let advertised_port = parsed
            .advertised_port
            .or_else(|| env("CHAR_ADVERTISED_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.advertised_port);

        let default_map_port = parsed
            .default_map_port
            .or_else(|| env("MAP_ADVERTISED_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.default_map_port);

        let database_url = parsed
            .database_url
            .or_else(|| env("DATABASE_URL"))
            .unwrap_or(defaults.database_url);

        Self {
            client_port,
            http_port,
            advertised_port,
            default_map_port,
            database_url,
        }
    }

    pub fn from_env() -> Self {
        Self::from_args_and_env(std::env::args(), |key| std::env::var(key).ok())
    }
}

#[cfg(test)]
mod tests;
