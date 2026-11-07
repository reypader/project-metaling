use clap::{error::ErrorKind, Parser};

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

/// Command-line arguments for `login-server`.
#[derive(Parser, Debug, Default)]
#[command(name = "login-server", about = "Metaling login server")]
struct LoginCliArgs {
    /// Client TCP listening port.
    #[arg(short = 'p', long = "port")]
    port: Option<u16>,

    /// Internal HTTP API listening port.
    #[arg(long = "http-port")]
    http_port: Option<u16>,

    /// Port advertised to clients for character server redirection.
    #[arg(
        long = "advertised-port",
        visible_alias = "char-port",
        visible_alias = "advertised-char-port"
    )]
    advertised_char_port: Option<u16>,

    /// PostgreSQL database connection URL.
    #[arg(long = "database-url", visible_alias = "account-database-url")]
    database_url: Option<String>,

    /// Host advertised to clients for character server redirection.
    #[arg(long = "char-host")]
    char_host: Option<String>,
}

impl LoginConfig {
    pub fn from_args_and_env<I, S, F>(args: I, env: F) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        F: Fn(&str) -> Option<String>,
    {
        let mut args_vec: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
        if args_vec.is_empty() || args_vec[0].starts_with('-') {
            args_vec.insert(0, "login-server".to_string());
        }

        let parsed = match LoginCliArgs::try_parse_from(&args_vec) {
            Ok(args) => args,
            Err(err) => match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => err.exit(),
                _ => LoginCliArgs::default(),
            },
        };
        let defaults = Self::default();

        let client_port = parsed
            .port
            .or_else(|| env("LOGIN_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.client_port);

        let http_port = parsed
            .http_port
            .or_else(|| env("LOGIN_HTTP_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.http_port);

        let advertised_char_port = parsed
            .advertised_char_port
            .or_else(|| env("CHAR_ADVERTISED_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.advertised_char_port);

        let database_url = parsed
            .database_url
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
mod tests;
