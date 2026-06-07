//! `arf diff` - git show with ARF reasoning prepended.

use crate::record::ArfRecord;
use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Command;

pub fn run(commit: Option<String>, full: bool) -> Result<()> {
    let sha = match commit {
        Some(c) => c,
        None => {
            let output = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
            if !output.status.success() {
                return Err(anyhow!("Failed to get HEAD"));
            }
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
    };

    let output = Command::new("git")
        .args(["log", "-1", "--oneline", &sha])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("Commit not found: {}", sha));
    }

    let commit_line = String::from_utf8_lossy(&output.stdout);
    let commit_line = commit_line.trim();

    let records_dir = Path::new(".arf/records");
    let short_sha = &sha[..8.min(sha.len())];

    println!("═══════════════════════════════════════════════════════════════");
    println!("Commit: {}", commit_line);
    println!("═══════════════════════════════════════════════════════════════");

    if records_dir.exists() {
        // Match either direction so a short-sha argument finds a dir
        // and vice versa.
        let commit_records_dir = std::fs::read_dir(records_dir).ok().and_then(|entries| {
            entries
                .filter_map(|e| e.ok())
                .find(|e| {
                    let dir_name = e.file_name().to_string_lossy().to_string();
                    dir_name.starts_with(short_sha) || short_sha.starts_with(&dir_name)
                })
                .map(|e| e.path())
        });

        if let Some(dir) = commit_records_dir {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                let mut records: Vec<ArfRecord> = Vec::new();

                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "toml") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(record) = toml::from_str::<ArfRecord>(&content) {
                                records.push(record);
                            }
                        }
                    }
                }

                records.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

                if !records.is_empty() {
                    println!();
                    println!("REASONING:");
                    for record in &records {
                        println!("  what: {}", record.what);
                        println!("  why:  {}", record.why);
                        if let Some(ref how) = record.how {
                            println!("  how:  {}", how);
                        }
                        println!();
                    }
                }
            }
        } else {
            println!();
            println!("(no ARF record for this commit)");
            println!();
        }
    }

    println!("───────────────────────────────────────────────────────────────");
    println!("CHANGES:");
    println!();

    let diff_args = if full {
        vec!["show", "--format=", &sha]
    } else {
        vec!["show", "--stat", "--format=", &sha]
    };

    let diff_output = Command::new("git").args(&diff_args).output()?;

    if diff_output.status.success() {
        print!("{}", String::from_utf8_lossy(&diff_output.stdout));
    }

    Ok(())
}
