//! `arf export` - machine-shaped output for downstream consumers.
//!
//! Selection mirrors `log` (--commit, optional --since), but the
//! output format is JSON / JSONL / TOML rather than human-readable.
//!
//! --since accepts either a bare date ("2026-06-01") or a full
//! RFC-3339 timestamp. Bare dates widen to start-of-day UTC so users
//! don't have to remember timezone formatting.

use crate::record::ArfRecord;
use crate::store::load_records;
use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum ExportFormat {
    Json,
    Jsonl,
    Toml,
}

pub fn run(
    commit: Option<String>,
    since: Option<String>,
    format: ExportFormat,
) -> Result<()> {
    let mut records = load_records(commit.as_deref())?;

    if let Some(since_str) = since {
        let normalized = if since_str.contains('T') {
            since_str
        } else {
            format!("{}T00:00:00Z", since_str)
        };
        let cutoff = chrono::DateTime::parse_from_rfc3339(&normalized)
            .map_err(|e| anyhow!("invalid --since value {:?}: {}", normalized, e))?;
        records.retain(|(_, r)| match chrono::DateTime::parse_from_rfc3339(&r.timestamp) {
            Ok(ts) => ts >= cutoff,
            // Records with unparseable timestamps are kept on the
            // theory that filtering them silently is worse than
            // leaking them; the consumer can decide what to do.
            Err(_) => true,
        });
    }

    let plain: Vec<&ArfRecord> = records.iter().map(|(_, r)| r).collect();

    match format {
        ExportFormat::Json => {
            let out = serde_json::to_string_pretty(&plain)?;
            println!("{}", out);
        }
        ExportFormat::Jsonl => {
            for record in &plain {
                let line = serde_json::to_string(record)?;
                println!("{}", line);
            }
        }
        ExportFormat::Toml => {
            // TOML doesn't natively support a top-level array, so wrap
            // in a `records` table-array key. Round-trips back through
            // toml::from_str cleanly with the wrapper.
            #[derive(Serialize)]
            struct Wrapper<'a> {
                records: Vec<&'a ArfRecord>,
            }
            let wrapper = Wrapper {
                records: plain.clone(),
            };
            let out = toml::to_string_pretty(&wrapper)?;
            print!("{}", out);
        }
    }

    Ok(())
}
