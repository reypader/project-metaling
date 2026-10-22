//! `npc-import --rathena $RATHENA_PATH --out crates/ro-db/data/npc`

use std::path::PathBuf;
use std::process::Command;

use clap::Parser;

/// Converts rAthena's renewal NPC files into per-map NPC_SPAWN_DB YAML.
#[derive(Parser, Debug)]
#[command(name = "npc-import")]
struct Args {
    /// rAthena checkout (defaults to $RATHENA_PATH).
    #[arg(long, env = "RATHENA_PATH")]
    rathena: PathBuf,
    /// Output directory for `<map>.yml` files.
    #[arg(long, default_value = "crates/ro-db/data/npc")]
    out: PathBuf,
    /// Print every warning instead of the first 20.
    #[arg(long)]
    all_warnings: bool,
}

const SHOWN_WARNINGS: usize = 20;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let commit = rathena_commit(&args.rathena);
    let (maps, report) = npc_import::import(&args.rathena)?;
    let written = npc_import::write_output(&args.out, &maps, &commit)?;

    println!("rAthena commit {commit}: {} files read", report.files);
    println!("converted:");
    for (kind, n) in &report.converted {
        println!("  {kind:<12} {n:>6}");
    }
    println!(
        "  {:<12} {:>6}",
        "total",
        report.converted.values().sum::<usize>()
    );
    println!("skipped:");
    for (reason, n) in &report.skipped {
        println!("  {reason:<28} {n:>6}");
    }
    for err in &report.file_errors {
        println!("file stopped early: {err}");
    }
    let shown = if args.all_warnings {
        report.warnings.len()
    } else {
        SHOWN_WARNINGS
    };
    println!("warnings: {}", report.warnings.len());
    for w in report.warnings.iter().take(shown) {
        println!("  {w}");
    }
    println!("wrote {written} map files to {}", args.out.display());
    Ok(())
}

fn rathena_commit(root: &PathBuf) -> String {
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
