//! `mob-import --rathena $RATHENA_PATH --items crates/ro-db/data/item --out crates/ro-db/data/mob`
//!
//! Run `item-import` first: drop names are resolved through the generated item data.

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use ro_db::ItemDb;

/// Converts rAthena's renewal mob database into MOB_DATA YAML.
#[derive(Parser, Debug)]
#[command(name = "mob-import")]
struct Args {
    /// rAthena checkout (defaults to $RATHENA_PATH). Pass an expanded path, not one with `~`.
    #[arg(long, env = "RATHENA_PATH")]
    rathena: PathBuf,
    /// Generated item data directory (from `item-import`).
    #[arg(long, default_value = "crates/ro-db/data/item")]
    items: PathBuf,
    /// Output directory.
    #[arg(long, default_value = "crates/ro-db/data/mob")]
    out: PathBuf,
    /// Print every warning instead of the first 20.
    #[arg(long)]
    all_warnings: bool,
}

const SHOWN_WARNINGS: usize = 20;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let commit = rathena_commit(&args.rathena);
    let items = ItemDb::load_dir(&args.items)?;
    let converted = mob_import::import(&args.rathena, &items)?;
    mob_import::write_output(&args.out, &converted, &commit)?;
    let report = &converted.report;

    println!("rAthena commit {commit}");
    println!(
        "  {}: {} entries -> {} mobs",
        mob_import::SOURCE,
        report.read,
        converted.mobs.len()
    );
    println!("merged repeated ids: {}", report.merged);
    let shown = if args.all_warnings {
        report.warnings.len()
    } else {
        SHOWN_WARNINGS
    };
    println!("warnings: {}", report.warnings.len());
    for w in report.warnings.iter().take(shown) {
        println!("  {w}");
    }
    println!("wrote {}", args.out.join(ro_db::MOB_DATA_FILE).display());
    Ok(())
}

fn rathena_commit(root: &Path) -> String {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map_or_else(|| "unknown".to_string(), |s| s.trim().to_string())
}
