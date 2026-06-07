//! `arf graph` - git log with ARF reasoning interleaved per commit.

use crate::record::ArfRecord;
use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Command;

pub fn run(limit: usize) -> Result<()> {
    let output = Command::new("git")
        .args(["log", "--oneline", "--no-decorate", &format!("-{}", limit)])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("Failed to get git log"));
    }

    let log = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<&str> = log.lines().collect();

    if commits.is_empty() {
        println!("No commits found.");
        return Ok(());
    }

    let records_dir = Path::new(".arf/records");
    let has_arf = records_dir.exists();

    println!("Git + ARF History:\n");

    for (i, line) in commits.iter().enumerate() {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        let (sha, msg) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            (parts[0], "")
        };

        let is_last = i == commits.len() - 1;
        let connector = if is_last { "└" } else { "├" };
        let continuation = if is_last { " " } else { "│" };

        println!("{}─● {} {}", connector, sha, msg);

        if has_arf {
            // git log shows 7-char shas; records dirs use 8 chars, so
            // do a starts_with match rather than equality.
            let commit_records_dir = if let Ok(entries) = std::fs::read_dir(records_dir) {
                entries
                    .filter_map(|e| e.ok())
                    .find(|e| e.file_name().to_string_lossy().starts_with(sha))
                    .map(|e| e.path())
            } else {
                None
            };

            if let Some(commit_records_dir) = commit_records_dir {
                if let Ok(entries) = std::fs::read_dir(&commit_records_dir) {
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

                    for (j, record) in records.iter().enumerate() {
                        let is_last_record = j == records.len() - 1;
                        let rec_connector = if is_last_record { "└" } else { "├" };

                        println!("{}  {}─ what: {}", continuation, rec_connector, record.what);
                        println!(
                            "{}  {}   why: {}",
                            continuation,
                            if is_last_record { " " } else { "│" },
                            record.why
                        );

                        if let Some(ref how) = record.how {
                            println!(
                                "{}  {}   how: {}",
                                continuation,
                                if is_last_record { " " } else { "│" },
                                how
                            );
                        }
                    }
                }
            }
        }
    }

    if !has_arf {
        println!("\n(ARF not initialized - run 'arf init' for reasoning context)");
    }

    Ok(())
}
