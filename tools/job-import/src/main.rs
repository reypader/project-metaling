//! `job-import --rathena $RATHENA_PATH --out crates/ro-db/data/job`

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;

/// Converts rAthena's renewal job files into JOB_DATA and STATPOINT_DATA YAML.
#[derive(Parser, Debug)]
#[command(name = "job-import")]
struct Args {
    /// rAthena checkout (defaults to $RATHENA_PATH). Pass an expanded path, not one with `~`.
    #[arg(long, env = "RATHENA_PATH")]
    rathena: PathBuf,
    /// Output directory.
    #[arg(long, default_value = "crates/ro-db/data/job")]
    out: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let commit = rathena_commit(&args.rathena);
    let converted = job_import::import(&args.rathena)?;
    job_import::write_output(&args.out, &converted, &commit)?;
    let report = &converted.report;

    println!("rAthena commit {commit}");
    for (source, read) in job_import::JOB_SOURCES.iter().zip(&report.read) {
        println!("  {source}: {read} entries");
    }
    println!("jobs: {}", converted.jobs.len());
    println!(
        "jobs with an AP table: {}",
        converted
            .jobs
            .iter()
            .filter(|job| !job.base_ap.is_empty())
            .count()
    );
    println!("status point rows: {}", converted.points.len());
    println!(
        "skipped names without an id ({}): {}",
        report.unknown_jobs.len(),
        report
            .unknown_jobs
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("fallback-filled tables ({}):", report.fallback.len());
    for (name, hp, sp) in &report.fallback {
        println!("  {name}: {hp} HP slots, {sp} SP slots");
    }
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
