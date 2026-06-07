//! `arf log` - human-readable listing of ARF records, newest first.

use crate::store::load_records;
use anyhow::Result;

pub fn run(commit: Option<String>, limit: usize) -> Result<()> {
    let all_records = load_records(commit.as_deref())?;

    if all_records.is_empty() {
        if let Some(ref sha) = commit {
            let short = &sha[..8.min(sha.len())];
            println!("No records for commit {}", short);
        } else {
            println!("No ARF records found.");
        }
        return Ok(());
    }

    let records: Vec<_> = all_records.into_iter().take(limit).collect();

    if records.is_empty() {
        println!("No ARF records found.");
        return Ok(());
    }

    println!("ARF Records ({}):\n", records.len());

    for (_filename, record) in records {
        let commit_str = record
            .commit
            .as_ref()
            .map(|c| &c[..8.min(c.len())])
            .unwrap_or("none");

        println!("commit {}", commit_str);
        println!("what: {}", record.what);
        println!("why: {}", record.why);
        if let Some(ref how) = record.how {
            println!("how: {}", how);
        }
        if let Some(ref backup) = record.backup {
            println!("backup: {}", backup);
        }
        println!("time: {}", record.timestamp);
        println!();
    }

    Ok(())
}
