//! `arf record` - persist a single ARF record keyed to a commit SHA.

use crate::record::{ArfRecord, FileRef};
use anyhow::{anyhow, Result};
use chrono::Utc;
use std::path::Path;
use std::process::Command;

pub fn run(
    what: String,
    why: String,
    how: Option<String>,
    backup: Option<String>,
    commit: Option<String>,
    files: Vec<String>,
) -> Result<()> {
    // Check if arf is initialized
    if !Path::new(".arf").exists() {
        return Err(anyhow!("ARF not initialized. Run 'arf init' first."));
    }

    // Get commit SHA (default to HEAD)
    let commit_sha = match commit {
        Some(c) => c,
        None => {
            let output = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
            if !output.status.success() {
                return Err(anyhow!("Failed to get HEAD commit"));
            }
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
    };

    let short_sha = &commit_sha[..8.min(commit_sha.len())];

    // Parse --file values into structured FileRefs. Any parse error
    // aborts the whole record - we'd rather fail loud than silently
    // drop file annotations the user asked for.
    let parsed_files: Vec<FileRef> = files
        .iter()
        .map(|s| FileRef::parse(s).map_err(|e| anyhow!("--file {:?}: {}", s, e)))
        .collect::<Result<Vec<_>>>()?;

    let record = ArfRecord {
        what,
        why,
        how,
        backup,
        outcome: None,
        timestamp: Utc::now().to_rfc3339(),
        commit: Some(commit_sha.clone()),
        agent: std::env::var("ARF_AGENT").ok(),
        files: if parsed_files.is_empty() {
            None
        } else {
            Some(parsed_files)
        },
    };

    let record_dir = format!(".arf/records/{}", short_sha);
    std::fs::create_dir_all(&record_dir)?;

    // Nanosecond precision so multiple records created back-to-back in
    // the same second don't collide on disk - earlier versions used
    // second precision and silently overwrote each other when an
    // agent emitted several records in quick succession (caught by
    // integration tests).
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S%9f");
    let agent = record.agent.as_deref().unwrap_or("unknown");
    let filename = format!("{}/{}-{}.toml", record_dir, agent, timestamp);

    let content = toml::to_string_pretty(&record)?;
    std::fs::write(&filename, &content)?;

    let add = Command::new("git")
        .args(["add", "."])
        .current_dir(".arf")
        .output()?;

    if !add.status.success() {
        return Err(anyhow!("Failed to stage record"));
    }

    let commit_msg = format!("Record: {}", record.what);
    let commit_result = Command::new("git")
        .args(["commit", "-m", &commit_msg])
        .current_dir(".arf")
        .output()?;

    if !commit_result.status.success() {
        let stderr = String::from_utf8_lossy(&commit_result.stderr);
        if !stderr.contains("nothing to commit") {
            return Err(anyhow!("Failed to commit record: {}", stderr));
        }
    }

    println!("✓ Recorded: {}", record.what);
    println!("  Commit: {}", short_sha);

    Ok(())
}
