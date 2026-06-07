//! `arf why <file>:<line>` - the headline feature.
//!
//! Resolves a file:line back to whichever commit last touched that
//! line (via `git blame`), then looks up ARF records for that
//! commit and prints the ones whose `files` field covers the
//! queried location. Falls back to "show all records for the
//! commit" when no record specifies file-level scope, since older
//! records (pre-v0.3) don't have the `files` field at all.

use crate::store::load_records;
use anyhow::{anyhow, Result};
use std::process::Command;

pub fn run(target: String) -> Result<()> {
    let (path, line) = parse_target(&target)?;

    // `git blame --porcelain -L <n>,<n> -- <path>` produces a
    // header line whose first whitespace-separated token is the
    // commit SHA the line last belongs to. Porcelain mode is the
    // stable machine-readable format.
    let blame = Command::new("git")
        .args([
            "blame",
            "--porcelain",
            "-L",
            &format!("{},{}", line, line),
            "--",
            &path,
        ])
        .output()?;

    if !blame.status.success() {
        return Err(anyhow!(
            "git blame failed for {}:{} - {}",
            path,
            line,
            String::from_utf8_lossy(&blame.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&blame.stdout);
    let sha = stdout
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().next())
        .ok_or_else(|| anyhow!("could not parse blame output"))?
        .to_string();

    let records = load_records(Some(&sha))?;

    if records.is_empty() {
        println!("No ARF records for {}:{} (commit {}).", path, line, &sha[..8.min(sha.len())]);
        println!("The line exists, but the agent didn't leave reasoning at that commit.");
        return Ok(());
    }

    // Prefer records whose `files` field specifically covers this
    // path+line. If none match, fall back to all records for the
    // commit (older records have no files field; they apply
    // commit-wide).
    let mut matched: Vec<_> = records
        .iter()
        .filter(|(_, r)| match &r.files {
            Some(refs) => refs.iter().any(|fr| fr.covers(&path, line)),
            None => false,
        })
        .collect();

    if matched.is_empty() {
        matched = records.iter().collect();
        println!(
            "No file-specific records for {}:{}; showing all reasoning attached to commit {}.",
            path,
            line,
            &sha[..8.min(sha.len())]
        );
        println!();
    } else {
        println!(
            "Reasoning for {}:{} (last touched in commit {}):",
            path,
            line,
            &sha[..8.min(sha.len())]
        );
        println!();
    }

    for (_filename, record) in matched {
        println!("what: {}", record.what);
        println!("why:  {}", record.why);
        if let Some(ref how) = record.how {
            println!("how:  {}", how);
        }
        if let Some(ref backup) = record.backup {
            println!("backup: {}", backup);
        }
        println!("time: {}", record.timestamp);
        println!();
    }

    Ok(())
}

/// Parse `path:line` into (path, line). Rejects missing-line and
/// non-numeric line specs - the subcommand requires a specific
/// line, since "what reasoning applies to this whole file" is
/// better answered by `arf log`.
fn parse_target(s: &str) -> Result<(String, u32)> {
    let (path, line_str) = s
        .rsplit_once(':')
        .ok_or_else(|| anyhow!("expected <file>:<line>, got {:?}", s))?;
    let line: u32 = line_str
        .parse()
        .map_err(|_| anyhow!("line must be a positive integer, got {:?}", line_str))?;
    if path.is_empty() {
        return Err(anyhow!("file path is empty in {:?}", s));
    }
    Ok((path.to_string(), line))
}
