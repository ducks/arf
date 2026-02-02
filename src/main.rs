use anyhow::{anyhow, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

const ARF_BRANCH: &str = "arf";

#[derive(Parser)]
#[command(name = "arf")]
#[command(about = "Agent Reasoning Format - track AI reasoning alongside git")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize ARF tracking (creates orphan branch)
    Init,

    /// Record a reasoning entry
    Record {
        /// What action is being taken (required)
        #[arg(long)]
        what: String,

        /// Why this approach (required)
        #[arg(long)]
        why: String,

        /// How it will be implemented (optional)
        #[arg(long)]
        how: Option<String>,

        /// Backup/rollback plan (optional)
        #[arg(short, long)]
        backup: Option<String>,

        /// Link to specific commit (defaults to HEAD)
        #[arg(short, long)]
        commit: Option<String>,
    },

    /// Show reasoning records
    Log {
        /// Show records for specific commit
        #[arg(short, long)]
        commit: Option<String>,

        /// Limit number of records
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Sync ARF branch with remote
    Sync {
        /// Push local records to remote
        #[arg(long)]
        push: bool,

        /// Pull remote records
        #[arg(long)]
        pull: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct ArfRecord {
    what: String,
    why: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    how: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    backup: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outcome: Option<String>,
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => cmd_init()?,
        Commands::Record {
            what,
            why,
            how,
            backup,
            commit,
        } => cmd_record(what, why, how, backup, commit)?,
        Commands::Log { commit, limit } => cmd_log(commit, limit)?,
        Commands::Sync { push, pull } => cmd_sync(push, pull)?,
    }

    Ok(())
}

fn cmd_init() -> Result<()> {
    // Check if we're in a git repo
    let status = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()?;

    if !status.status.success() {
        return Err(anyhow!("Not a git repository. Run 'git init' first."));
    }

    // Check if arf branch already exists
    let branch_check = Command::new("git")
        .args(["rev-parse", "--verify", ARF_BRANCH])
        .output()?;

    if branch_check.status.success() {
        println!("✓ ARF branch '{}' already exists", ARF_BRANCH);
        return Ok(());
    }

    println!("Initializing ARF...");

    // Create orphan branch using worktree
    let output = Command::new("git")
        .args(["worktree", "add", "--orphan", "-b", ARF_BRANCH, ".arf"])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!(
            "Failed to create ARF branch: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Create initial structure
    std::fs::create_dir_all(".arf/records")?;

    // Create README in arf branch
    let readme = r#"# ARF Records

This branch contains Agent Reasoning Format records.

Records are organized by commit SHA:
```
records/
  <commit-sha>/
    <agent>-<timestamp>.toml
```

See https://github.com/ducks/arf for the ARF specification.
"#;
    std::fs::write(".arf/README.md", readme)?;

    // Commit initial structure
    let add = Command::new("git")
        .args(["add", "."])
        .current_dir(".arf")
        .output()?;

    if !add.status.success() {
        return Err(anyhow!("Failed to stage files"));
    }

    let commit = Command::new("git")
        .args(["commit", "-m", "Initialize ARF"])
        .current_dir(".arf")
        .output()?;

    if !commit.status.success() {
        // Might be empty, that's ok
        let stderr = String::from_utf8_lossy(&commit.stderr);
        if !stderr.contains("nothing to commit") {
            return Err(anyhow!("Failed to commit: {}", stderr));
        }
    }

    println!("✓ Created ARF branch '{}'", ARF_BRANCH);
    println!("✓ Mounted at .arf/");
    println!();
    println!("Next: arf record --what 'action' --why 'reason'");

    Ok(())
}

fn cmd_record(
    what: String,
    why: String,
    how: Option<String>,
    backup: Option<String>,
    commit: Option<String>,
) -> Result<()> {
    // Check if arf is initialized
    if !Path::new(".arf").exists() {
        return Err(anyhow!("ARF not initialized. Run 'arf init' first."));
    }

    // Get commit SHA (default to HEAD)
    let commit_sha = match commit {
        Some(c) => c,
        None => {
            let output = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()?;
            if !output.status.success() {
                return Err(anyhow!("Failed to get HEAD commit"));
            }
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
    };

    let short_sha = &commit_sha[..8.min(commit_sha.len())];

    // Create record
    let record = ArfRecord {
        what,
        why,
        how,
        backup,
        outcome: None,
        timestamp: Utc::now().to_rfc3339(),
        commit: Some(commit_sha.clone()),
        agent: std::env::var("ARF_AGENT").ok(),
    };

    // Create directory for this commit
    let record_dir = format!(".arf/records/{}", short_sha);
    std::fs::create_dir_all(&record_dir)?;

    // Generate filename
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let agent = record.agent.as_deref().unwrap_or("unknown");
    let filename = format!("{}/{}-{}.toml", record_dir, agent, timestamp);

    // Write record
    let content = toml::to_string_pretty(&record)?;
    std::fs::write(&filename, &content)?;

    // Commit to arf branch
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

fn cmd_log(commit: Option<String>, limit: usize) -> Result<()> {
    if !Path::new(".arf/records").exists() {
        return Err(anyhow!("ARF not initialized. Run 'arf init' first."));
    }

    let records_dir = Path::new(".arf/records");
    let mut all_records: Vec<(String, ArfRecord)> = Vec::new();

    // If specific commit requested, only look there
    let dirs_to_check: Vec<_> = if let Some(ref sha) = commit {
        let short = &sha[..8.min(sha.len())];
        let path = records_dir.join(short);
        if path.exists() {
            vec![path]
        } else {
            println!("No records for commit {}", short);
            return Ok(());
        }
    } else {
        std::fs::read_dir(records_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect()
    };

    // Read all records
    for dir in dirs_to_check {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "toml") {
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

    // Sort by timestamp (newest first)
    all_records.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));

    // Limit
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

fn cmd_sync(push: bool, pull: bool) -> Result<()> {
    if !Path::new(".arf").exists() {
        return Err(anyhow!("ARF not initialized. Run 'arf init' first."));
    }

    // Default to both if neither specified
    let (do_pull, do_push) = if !push && !pull {
        (true, true)
    } else {
        (pull, push)
    };

    if do_pull {
        println!("Pulling ARF records...");
        let output = Command::new("git")
            .args(["pull", "origin", ARF_BRANCH])
            .current_dir(".arf")
            .output()?;

        if output.status.success() {
            println!("✓ Pulled");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("couldn't find remote ref") {
                println!("  No remote ARF branch yet");
            } else {
                println!("  Pull failed: {}", stderr.trim());
            }
        }
    }

    if do_push {
        println!("Pushing ARF records...");
        let output = Command::new("git")
            .args(["push", "-u", "origin", ARF_BRANCH])
            .current_dir(".arf")
            .output()?;

        if output.status.success() {
            println!("✓ Pushed");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("  Push failed: {}", stderr.trim());
        }
    }

    Ok(())
}
