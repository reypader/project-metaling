use std::path::PathBuf;

use clap::{error::ErrorKind, Parser};

use crate::RegistryPaths;

/// Configuration for `map-server` network ports and database connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapConfig {
    pub client_port: u16,
    pub http_port: u16,
    pub advertised_port: u16,
    pub database_url: String,
    /// Directory holding `<map>.gat` files (`MAP_DATA_DIR`). Unset: every map uses the
    /// all-walkable fallback grid.
    pub map_data_dir: Option<PathBuf>,
    /// Directory holding `<map>.yml` NPC spawn lists (`NPC_DATA_DIR`), normally
    /// `crates/ro-db/data/npc`. Unset: no map has NPCs.
    pub npc_data_dir: Option<PathBuf>,
    /// Directory holding `<map>.yml` mob spawn lists (`MOB_SPAWN_DATA_DIR`), normally
    /// `crates/ro-db/data/mob_spawn`. Unset: no map has mobs.
    pub mob_spawn_data_dir: Option<PathBuf>,
    /// Directory holding the item registry (`ITEM_DATA_DIR`), normally `crates/ro-db/data/item`.
    /// Unset: an empty registry.
    pub item_data_dir: Option<PathBuf>,
    /// Directory holding the mob registry (`MOB_DATA_DIR`), normally `crates/ro-db/data/mob`.
    /// Unset: an empty registry.
    pub mob_data_dir: Option<PathBuf>,
    /// Directory holding the job registry (`JOB_DATA_DIR`), normally `crates/ro-db/data/job`.
    /// Required: the server refuses to start without it.
    pub job_data_dir: Option<PathBuf>,
    /// Directory holding the refine table (`REFINE_DATA_DIR`), normally
    /// `crates/ro-db/data/refine`. Unset: no refine bonuses.
    pub refine_data_dir: Option<PathBuf>,
    /// Directory holding the skill registry and trees (`SKILL_DATA_DIR`), normally
    /// `crates/ro-db/data/skill`. Required: the server refuses to start without it.
    pub skill_data_dir: Option<PathBuf>,
    /// Our battle config YAML (`BATTLE_CONFIG`), normally `config/battle.yml`. Unset: rAthena's
    /// shipped defaults.
    pub battle_config: Option<PathBuf>,
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            client_port: 5121,
            http_port: 5122,
            advertised_port: 5121,
            database_url: "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string(),
            map_data_dir: None,
            npc_data_dir: None,
            mob_spawn_data_dir: None,
            item_data_dir: None,
            mob_data_dir: None,
            job_data_dir: None,
            refine_data_dir: None,
            skill_data_dir: None,
            battle_config: None,
        }
    }
}

/// Command-line arguments for `map-server`.
#[derive(Parser, Debug, Default)]
#[command(name = "map-server", about = "Metaling map server")]
struct MapCliArgs {
    /// Client TCP listening port.
    #[arg(short = 'p', long = "port")]
    port: Option<u16>,

    /// Internal HTTP API listening port.
    #[arg(long = "http-port")]
    http_port: Option<u16>,

    /// Port advertised to clients for map server connections.
    #[arg(long = "advertised-port", visible_alias = "map-port")]
    advertised_port: Option<u16>,

    /// PostgreSQL database connection URL.
    #[arg(long = "database-url", visible_alias = "map-database-url")]
    database_url: Option<String>,

    /// Map server client host.
    #[arg(long = "map-host")]
    map_host: Option<String>,

    /// Directory holding `<map>.gat` walkability files.
    #[arg(long = "map-data-dir")]
    map_data_dir: Option<PathBuf>,

    /// Directory holding `<map>.yml` NPC spawn lists.
    #[arg(long = "npc-data-dir")]
    npc_data_dir: Option<PathBuf>,

    /// Directory holding `<map>.yml` mob spawn lists.
    #[arg(long = "mob-spawn-data-dir")]
    mob_spawn_data_dir: Option<PathBuf>,

    /// Directory holding the item registry (`item_*.yml`).
    #[arg(long = "item-data-dir")]
    item_data_dir: Option<PathBuf>,

    /// Directory holding the mob registry (`mob_db.yml`).
    #[arg(long = "mob-data-dir")]
    mob_data_dir: Option<PathBuf>,

    /// Directory holding the job registry (`job_db.yml`, `statpoint.yml`).
    #[arg(long = "job-data-dir")]
    job_data_dir: Option<PathBuf>,

    /// Directory holding the refine table (`refine.yml`).
    #[arg(long = "refine-data-dir")]
    refine_data_dir: Option<PathBuf>,

    /// Directory holding the skill registry and trees (`skill_db.yml`, `skill_tree.yml`).
    #[arg(long = "skill-data-dir")]
    skill_data_dir: Option<PathBuf>,

    /// Battle config YAML file.
    #[arg(long = "battle-config")]
    battle_config: Option<PathBuf>,
}

impl MapConfig {
    pub fn from_args_and_env<I, S, F>(args: I, env: F) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        F: Fn(&str) -> Option<String>,
    {
        let mut args_vec: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
        if args_vec.is_empty() || args_vec[0].starts_with('-') {
            args_vec.insert(0, "map-server".to_string());
        }

        let parsed = match MapCliArgs::try_parse_from(&args_vec) {
            Ok(args) => args,
            Err(err) => match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => err.exit(),
                _ => MapCliArgs::default(),
            },
        };
        let defaults = Self::default();

        let client_port = parsed
            .port
            .or_else(|| env("MAP_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.client_port);

        let http_port = parsed
            .http_port
            .or_else(|| env("MAP_HTTP_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.http_port);

        let advertised_port = parsed
            .advertised_port
            .or_else(|| env("MAP_ADVERTISED_PORT").and_then(|v| v.parse().ok()))
            .unwrap_or(defaults.advertised_port);

        let database_url = parsed
            .database_url
            .or_else(|| env("DATABASE_URL"))
            .unwrap_or(defaults.database_url);

        let map_data_dir = parsed
            .map_data_dir
            .or_else(|| env("MAP_DATA_DIR").map(PathBuf::from));

        let npc_data_dir = parsed
            .npc_data_dir
            .or_else(|| env("NPC_DATA_DIR").map(PathBuf::from));

        let mob_spawn_data_dir = parsed
            .mob_spawn_data_dir
            .or_else(|| env("MOB_SPAWN_DATA_DIR").map(PathBuf::from));

        let item_data_dir = parsed
            .item_data_dir
            .or_else(|| env("ITEM_DATA_DIR").map(PathBuf::from));

        let mob_data_dir = parsed
            .mob_data_dir
            .or_else(|| env("MOB_DATA_DIR").map(PathBuf::from));

        let job_data_dir = parsed
            .job_data_dir
            .or_else(|| env("JOB_DATA_DIR").map(PathBuf::from));

        let refine_data_dir = parsed
            .refine_data_dir
            .or_else(|| env("REFINE_DATA_DIR").map(PathBuf::from));

        let skill_data_dir = parsed
            .skill_data_dir
            .or_else(|| env("SKILL_DATA_DIR").map(PathBuf::from));

        let battle_config = parsed
            .battle_config
            .or_else(|| env("BATTLE_CONFIG").map(PathBuf::from));

        Self {
            client_port,
            http_port,
            advertised_port,
            database_url,
            map_data_dir,
            npc_data_dir,
            mob_spawn_data_dir,
            item_data_dir,
            mob_data_dir,
            job_data_dir,
            refine_data_dir,
            skill_data_dir,
            battle_config,
        }
    }

    pub fn from_env() -> Self {
        Self::from_args_and_env(std::env::args(), |key| std::env::var(key).ok())
    }

    /// The registry and battle-config sources for `load_registries`.
    pub fn registry_paths(&self) -> RegistryPaths<'_> {
        RegistryPaths {
            items: self.item_data_dir.as_deref(),
            mobs: self.mob_data_dir.as_deref(),
            jobs: self.job_data_dir.as_deref(),
            refine: self.refine_data_dir.as_deref(),
            skills: self.skill_data_dir.as_deref(),
            battle: self.battle_config.as_deref(),
        }
    }

    /// Refuses a config without the data directories the server can't run without: the job
    /// data (stats) and the skill data (the skill list).
    pub fn check_required(&self) -> anyhow::Result<()> {
        if self.job_data_dir.is_none() {
            anyhow::bail!(
                "JOB_DATA_DIR is not set; the job data (crates/ro-db/data/job) is required for stats"
            );
        }
        if self.skill_data_dir.is_none() {
            anyhow::bail!(
                "SKILL_DATA_DIR is not set; the skill data (crates/ro-db/data/skill) is required \
                 for the skill list"
            );
        }
        Ok(())
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

        let cfg = MapConfig::from_args_and_env(Vec::<String>::new(), |key| {
            env_map.get(key).map(|v| v.to_string())
        });

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
        let cfg = MapConfig::from_args_and_env(args, |key| env_map.get(key).map(|v| v.to_string()));

        assert_eq!(cfg.client_port, 5150);
        assert_eq!(cfg.http_port, 5151);
        assert_eq!(cfg.advertised_port, 5121);
    }

    #[test]
    fn test_bin_03_map_advertised_port_decoupling() {
        let env_map: HashMap<&str, &str> = [("MAP_PORT", "5131"), ("MAP_ADVERTISED_PORT", "5121")]
            .into_iter()
            .collect();

        let cfg = MapConfig::from_args_and_env(Vec::<String>::new(), |key| {
            env_map.get(key).map(|v| v.to_string())
        });

        assert_eq!(cfg.client_port, 5131, "bind port must be 5131");
        assert_eq!(cfg.advertised_port, 5121, "advertised port must be 5121");
    }

    #[test]
    fn test_bin_cfg_003_map_default_database_url() {
        let cfg = MapConfig::from_args_and_env(Vec::<String>::new(), |_| None);
        assert_eq!(
            cfg.database_url,
            "postgres://ragnarok:ragnarok@localhost:5432/ragnarok"
        );
    }

    #[test]
    fn test_bin_cfg_persist_001_map_database_url_cli_over_env_over_default() {
        let env_map: HashMap<&str, &str> = [(
            "DATABASE_URL",
            "postgres://map_env_user:map_env_pass@env-host:5432/map_env_db",
        )]
        .into_iter()
        .collect();

        let cfg_env = MapConfig::from_args_and_env(Vec::<String>::new(), |key| {
            env_map.get(key).map(|v| v.to_string())
        });
        assert_eq!(
            cfg_env.database_url,
            "postgres://map_env_user:map_env_pass@env-host:5432/map_env_db"
        );

        let args = vec![
            "map-server",
            "--database-url",
            "postgres://map_cli_user:map_cli_pass@cli-host:5432/map_cli_db",
        ];
        let cfg_cli =
            MapConfig::from_args_and_env(args, |key| env_map.get(key).map(|v| v.to_string()));
        assert_eq!(
            cfg_cli.database_url,
            "postgres://map_cli_user:map_cli_pass@cli-host:5432/map_cli_db"
        );
    }
}
