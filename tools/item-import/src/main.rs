//! `item-import --rathena $RATHENA_PATH --out crates/ro-db/data/item`

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;

/// Converts rAthena's renewal item database into ITEM_DATA YAML.
#[derive(Parser, Debug)]
#[command(name = "item-import")]
struct Args {
    /// rAthena checkout (defaults to $RATHENA_PATH). Pass an expanded path, not one with `~`.
    #[arg(long, env = "RATHENA_PATH")]
    rathena: PathBuf,
    /// Output directory.
    #[arg(long, default_value = "crates/ro-db/data/item")]
    out: PathBuf,
    /// Print every warning instead of the first 20.
    #[arg(long)]
    all_warnings: bool,
}

const SHOWN_WARNINGS: usize = 20;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let commit = rathena_commit(&args.rathena);
    let converted = item_import::import(&args.rathena)?;
    let written = item_import::write_output(&args.out, &converted, &commit)?;
    let report = &converted.report;

    println!("rAthena commit {commit}");
    for ((source, file), (read, items)) in item_import::SOURCES
        .iter()
        .zip(report.read.iter().zip(&converted.files))
    {
        println!(
            "  {source}: {read} entries -> {file}: {} items",
            items.len()
        );
    }
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
    println!("wrote {written} items to {}", args.out.display());
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
