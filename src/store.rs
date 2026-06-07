//! Disk layout for ARF records.
//!
//! Records live on the `arf` orphan git branch, checked out as a
//! worktree at `.arf/`. Within the worktree:
//!
//!   .arf/
//!   ├── README.md         # human-facing breadcrumb
//!   └── records/
//!       └── <short-sha>/  # 8-char commit SHA
//!           └── <agent>-<timestamp>.toml
//!
//! This module owns the loader. Mutating commands (record/init) call
//! out to git directly; only the read path needs to be shared
//! between commands.

use crate::record::ArfRecord;
use anyhow::{anyhow, Result};
use std::path::Path;

/// Name of the orphan git branch that stores ARF records.
pub const ARF_BRANCH: &str = "arf";

/// Walk `.arf/records` and return every record on disk, optionally
/// scoped to a single commit's directory. Records are returned
/// newest first by timestamp.
///
/// Returns:
///   - Vec<(filename, ArfRecord)> on success, possibly empty
///   - Err if `.arf` is not initialized
///
/// The Ok(empty) case includes "specific commit requested but no
/// records exist for it" - callers decide how to message that.
pub fn load_records(commit: Option<&str>) -> Result<Vec<(String, ArfRecord)>> {
    if !Path::new(".arf/records").exists() {
        return Err(anyhow!("ARF not initialized. Run 'arf init' first."));
    }

    let records_dir = Path::new(".arf/records");
    let mut all_records: Vec<(String, ArfRecord)> = Vec::new();

    let dirs_to_check: Vec<_> = if let Some(sha) = commit {
        let short = &sha[..8.min(sha.len())];
        let path = records_dir.join(short);
        if !path.exists() {
            return Ok(Vec::new());
        }
        vec![path]
    } else {
        std::fs::read_dir(records_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect()
    };

    for dir in dirs_to_check {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "toml") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(record) = toml::from_str::<ArfRecord>(&content) {
                            let filename = path.file_name().unwrap().to_string_lossy().to_string();
                            all_records.push((filename, record));
                        }
                    }
                }
            }
        }
    }

    all_records.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));
    Ok(all_records)
}
