//! `skill-import --rathena $RATHENA_PATH --out crates/ro-db/data/skill`

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;

/// Converts rAthena's renewal skill files into SKILL_DATA and SKILL_TREE_DATA YAML.
#[derive(Parser, Debug)]
#[command(name = "skill-import")]
struct Args {
    /// rAthena checkout (defaults to $RATHENA_PATH). Pass an expanded path, not one with `~`.
    #[arg(long, env = "RATHENA_PATH")]
    rathena: PathBuf,
    /// Output directory.
    #[arg(long, default_value = "crates/ro-db/data/skill")]
    out: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let commit = rathena_commit(&args.rathena);
    let converted = skill_import::import(&args.rathena)?;
    skill_import::write_output(&args.out, &converted, &commit)?;
    let report = &converted.report;

    println!("rAthena commit {commit}");
    println!(
        "  {}: {} entries",
        skill_import::SKILL_DB_SOURCE,
        report.skills_read
    );
    println!(
        "  {}: {} entries",
        skill_import::SKILL_TREE_SOURCE,
        report.jobs_read
    );
    println!("skills written: {}", converted.skills.len());
    println!(
        "job trees written: {} ({} entries)",
        converted.trees.len(),
        converted
            .trees
            .iter()
            .map(|tree| tree.tree.len())
            .sum::<usize>()
    );
    println!(
        "skipped jobs without an id ({}): {}",
        report.skipped_jobs.len(),
        report
            .skipped_jobs
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("warnings: {}", report.warnings.len());
    for w in &report.warnings {
        println!("  {w}");
    }
    println!("wrote {}", args.out.display());
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
