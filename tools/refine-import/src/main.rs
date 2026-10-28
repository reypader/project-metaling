//! `refine-import --rathena $RATHENA_PATH --out crates/ro-db/data/refine`

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;

/// Converts rAthena's renewal refine table into REFINE_DATA YAML.
#[derive(Parser, Debug)]
#[command(name = "refine-import")]
struct Args {
    /// rAthena checkout (defaults to $RATHENA_PATH). Pass an expanded path, not one with `~`.
    #[arg(long, env = "RATHENA_PATH")]
    rathena: PathBuf,
    /// Output directory.
    #[arg(long, default_value = "crates/ro-db/data/refine")]
    out: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let commit = rathena_commit(&args.rathena);
    let converted = refine_import::import(&args.rathena)?;
    refine_import::write_output(&args.out, &converted.rows, &commit)?;
    println!("rAthena commit {commit}");
    for row in &converted.rows {
        println!(
            "  {:?} level {}: +10 Bonus {}",
            row.group,
            row.level,
            row.bonus.get(9).copied().unwrap_or(0)
        );
    }
    println!("warnings: {}", converted.warnings.len());
    for w in &converted.warnings {
        println!("  {w}");
    }
    println!(
        "wrote {} rows to {}",
        converted.rows.len(),
        args.out.display()
    );
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
