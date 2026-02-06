use anyhow::{anyhow, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use serde::{Deserialize, Serialize};
use std::io::stdout;
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

    /// Manage specs (task definitions)
    Spec {
        #[command(subcommand)]
        command: SpecCommands,
    },

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

    /// Show git commits with ARF reasoning
    Graph {
        /// Number of commits to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Show diff with ARF reasoning context
    Diff {
        /// Commit to diff (defaults to HEAD)
        #[arg(short, long)]
        commit: Option<String>,

        /// Show full diff instead of stat summary
        #[arg(long)]
        full: bool,
    },

    /// Interactive TUI browser
    Browse,
}

#[derive(Subcommand)]
enum SpecCommands {
    /// List all specs
    List,

    /// Show a specific spec
    Show {
        /// Spec name (without .arf extension)
        name: String,
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
        Commands::Spec { command } => match command {
            SpecCommands::List => cmd_spec_list()?,
            SpecCommands::Show { name } => cmd_spec_show(&name)?,
        },
        Commands::Record {
            what,
            why,
            how,
            backup,
            commit,
        } => cmd_record(what, why, how, backup, commit)?,
        Commands::Log { commit, limit } => cmd_log(commit, limit)?,
        Commands::Sync { push, pull } => cmd_sync(push, pull)?,
        Commands::Graph { limit } => cmd_graph(limit)?,
        Commands::Diff { commit, full } => cmd_diff(commit, full)?,
        Commands::Browse => cmd_browse()?,
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
    std::fs::create_dir_all(".arf/specs")?;

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

fn cmd_spec_list() -> Result<()> {
    let specs_dir = Path::new(".arf/specs");

    if !specs_dir.exists() {
        return Err(anyhow!(
            "ARF not initialized or no specs directory. Run 'arf init' first."
        ));
    }

    let mut specs: Vec<String> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(specs_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "arf") {
                if let Some(name) = path.file_stem() {
                    specs.push(name.to_string_lossy().to_string());
                }
            }
        }
    }

    if specs.is_empty() {
        println!("No specs found in .arf/specs/");
        println!();
        println!("Generate specs with: lok spec \"your task description\"");
        return Ok(());
    }

    specs.sort();

    println!("Specs ({}):\n", specs.len());
    for name in &specs {
        println!("  {}", name);
    }
    println!();
    println!("Show details: arf spec show <name>");

    Ok(())
}

fn cmd_spec_show(name: &str) -> Result<()> {
    let specs_dir = Path::new(".arf/specs");
    let spec_path = specs_dir.join(format!("{}.arf", name));

    if !spec_path.exists() {
        return Err(anyhow!("Spec not found: {}", name));
    }

    let content = std::fs::read_to_string(&spec_path)?;

    println!("═══════════════════════════════════════════════════════════════");
    println!("Spec: {}", name);
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    print!("{}", content);

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
            let output = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
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

fn cmd_graph(limit: usize) -> Result<()> {
    // Get git log
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

        // Graph connector
        let is_last = i == commits.len() - 1;
        let connector = if is_last { "└" } else { "├" };
        let continuation = if is_last { " " } else { "│" };

        // Print commit line
        println!("{}─● {} {}", connector, sha, msg);

        // Check for ARF records for this commit
        if has_arf {
            // Try to find matching records dir (git log shows 7 chars, records use 8)
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

                    // Sort by timestamp
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

fn cmd_diff(commit: Option<String>, full: bool) -> Result<()> {
    // Get the commit SHA (default to HEAD)
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

    // Get commit info
    let output = Command::new("git")
        .args(["log", "-1", "--oneline", &sha])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("Commit not found: {}", sha));
    }

    let commit_line = String::from_utf8_lossy(&output.stdout);
    let commit_line = commit_line.trim();

    // Print ARF context first
    let records_dir = Path::new(".arf/records");
    let short_sha = &sha[..8.min(sha.len())];

    println!("═══════════════════════════════════════════════════════════════");
    println!("Commit: {}", commit_line);
    println!("═══════════════════════════════════════════════════════════════");

    if records_dir.exists() {
        // Find matching records dir (match either direction for flexibility)
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

    // Show diff
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

// TUI types and implementation

#[derive(Debug)]
struct CommitInfo {
    sha: String,
    short_sha: String,
    message: String,
    records: Vec<ArfRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DiffMode {
    Hidden,
    Stat,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Focus {
    Commits,
    Diff,
}

struct App {
    commits: Vec<CommitInfo>,
    list_state: ListState,
    diff_mode: DiffMode,
    diff_lines: Vec<DiffLine>,
    diff_scroll: usize,
    focus: Focus,
    should_quit: bool,
}

#[derive(Debug, Clone)]
struct DiffLine {
    content: String,
    style: Style,
}

impl App {
    fn new(commits: Vec<CommitInfo>) -> Self {
        let mut list_state = ListState::default();
        if !commits.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            commits,
            list_state,
            diff_mode: DiffMode::Stat,
            diff_lines: Vec::new(),
            diff_scroll: 0,
            focus: Focus::Commits,
            should_quit: false,
        }
    }

    fn selected_commit(&self) -> Option<&CommitInfo> {
        self.list_state.selected().and_then(|i| self.commits.get(i))
    }

    fn next(&mut self) {
        match self.focus {
            Focus::Commits => {
                if self.commits.is_empty() {
                    return;
                }
                let i = match self.list_state.selected() {
                    Some(i) => (i + 1) % self.commits.len(),
                    None => 0,
                };
                self.list_state.select(Some(i));
                self.diff_scroll = 0;
                self.update_diff();
            }
            Focus::Diff => {
                if self.diff_scroll < self.diff_lines.len().saturating_sub(1) {
                    self.diff_scroll += 1;
                }
            }
        }
    }

    fn previous(&mut self) {
        match self.focus {
            Focus::Commits => {
                if self.commits.is_empty() {
                    return;
                }
                let i = match self.list_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.commits.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.list_state.select(Some(i));
                self.diff_scroll = 0;
                self.update_diff();
            }
            Focus::Diff => {
                self.diff_scroll = self.diff_scroll.saturating_sub(1);
            }
        }
    }

    fn toggle_focus(&mut self) {
        if self.diff_mode != DiffMode::Hidden {
            self.focus = match self.focus {
                Focus::Commits => Focus::Diff,
                Focus::Diff => Focus::Commits,
            };
        }
    }

    fn toggle_diff(&mut self) {
        self.diff_mode = match self.diff_mode {
            DiffMode::Hidden => DiffMode::Stat,
            DiffMode::Stat => DiffMode::Full,
            DiffMode::Full => DiffMode::Hidden,
        };
        if self.diff_mode == DiffMode::Hidden {
            self.focus = Focus::Commits;
        }
        self.diff_scroll = 0;
        self.update_diff();
    }

    fn page_down(&mut self) {
        if self.focus == Focus::Diff {
            self.diff_scroll = (self.diff_scroll + 10).min(self.diff_lines.len().saturating_sub(1));
        }
    }

    fn page_up(&mut self) {
        if self.focus == Focus::Diff {
            self.diff_scroll = self.diff_scroll.saturating_sub(10);
        }
    }

    fn update_diff(&mut self) {
        self.diff_lines.clear();

        if self.diff_mode == DiffMode::Hidden {
            return;
        }

        let Some(commit) = self.selected_commit() else {
            return;
        };

        let args = if self.diff_mode == DiffMode::Full {
            vec!["show", "--format=", &commit.sha]
        } else {
            vec!["show", "--stat", "--format=", &commit.sha]
        };

        let output = Command::new("git").args(&args).output();
        let content = match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => "Failed to get diff".to_string(),
        };

        // Parse lines with syntax highlighting
        for line in content.lines() {
            let (style, display) = if line.starts_with('+') && !line.starts_with("+++") {
                (Style::default().fg(Color::Green), line.to_string())
            } else if line.starts_with('-') && !line.starts_with("---") {
                (Style::default().fg(Color::Red), line.to_string())
            } else if line.starts_with("@@") {
                (Style::default().fg(Color::Cyan), line.to_string())
            } else if line.starts_with("diff ") || line.starts_with("index ") {
                (Style::default().fg(Color::Yellow).bold(), line.to_string())
            } else if line.starts_with("+++") || line.starts_with("---") {
                (Style::default().fg(Color::Yellow), line.to_string())
            } else {
                (Style::default(), line.to_string())
            };

            self.diff_lines.push(DiffLine {
                content: display,
                style,
            });
        }
    }
}

fn cmd_browse() -> Result<()> {
    // Get commits
    let output = Command::new("git")
        .args(["log", "--oneline", "--no-decorate", "-50"])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("Failed to get git log"));
    }

    let log = String::from_utf8_lossy(&output.stdout);
    let records_dir = Path::new(".arf/records");

    let mut commits: Vec<CommitInfo> = Vec::new();

    for line in log.lines() {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        let (sha, msg) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            (parts[0], "")
        };

        let short_sha = sha.to_string();

        // Get full SHA for diff
        let full_sha = Command::new("git")
            .args(["rev-parse", sha])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| sha.to_string());

        // Find ARF records
        let mut records = Vec::new();
        if records_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(records_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let dir_name = entry.file_name().to_string_lossy().to_string();
                    if dir_name.starts_with(&short_sha) || short_sha.starts_with(&dir_name) {
                        if let Ok(record_entries) = std::fs::read_dir(entry.path()) {
                            for record_entry in record_entries.filter_map(|e| e.ok()) {
                                let path = record_entry.path();
                                if path.extension().is_some_and(|e| e == "toml") {
                                    if let Ok(content) = std::fs::read_to_string(&path) {
                                        if let Ok(record) = toml::from_str::<ArfRecord>(&content) {
                                            records.push(record);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        commits.push(CommitInfo {
            sha: full_sha,
            short_sha,
            message: msg.to_string(),
            records,
        });
    }

    if commits.is_empty() {
        println!("No commits found.");
        return Ok(());
    }

    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut app = App::new(commits);
    app.update_diff();

    // Main loop
    loop {
        terminal.draw(|frame| ui(frame, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Char('d') => app.toggle_diff(),
                    KeyCode::Tab | KeyCode::Enter => app.toggle_focus(),
                    KeyCode::PageDown | KeyCode::Char('f') => app.page_down(),
                    KeyCode::PageUp | KeyCode::Char('b') => app.page_up(),
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

fn ui(frame: &mut Frame, app: &mut App) {
    let has_diff = app.diff_mode != DiffMode::Hidden;

    // Border styles based on focus
    let focused_border = Style::default().fg(Color::Cyan);
    let unfocused_border = Style::default();

    // Main layout
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if has_diff {
            vec![Constraint::Percentage(50), Constraint::Percentage(50)]
        } else {
            vec![Constraint::Percentage(100)]
        })
        .split(frame.area());

    // Top section: commits + reasoning
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(main_chunks[0]);

    // Commits list
    let items: Vec<ListItem> = app
        .commits
        .iter()
        .map(|c| {
            let has_arf = if c.records.is_empty() { " " } else { "●" };
            ListItem::new(format!("{} {} {}", has_arf, c.short_sha, c.message))
        })
        .collect();

    let commits_border = if app.focus == Focus::Commits {
        focused_border
    } else {
        unfocused_border
    };

    let commits_list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(commits_border)
                .title(" Commits "),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).bold())
        .highlight_symbol("→ ");

    frame.render_stateful_widget(commits_list, top_chunks[0], &mut app.list_state);

    // Reasoning panel
    let reasoning_text = if let Some(commit) = app.selected_commit() {
        if commit.records.is_empty() {
            "(no ARF record for this commit)".to_string()
        } else {
            commit
                .records
                .iter()
                .map(|r| {
                    let mut s = format!("what: {}\nwhy:  {}", r.what, r.why);
                    if let Some(ref how) = r.how {
                        s.push_str(&format!("\nhow:  {}", how));
                    }
                    if let Some(ref backup) = r.backup {
                        s.push_str(&format!("\nback: {}", backup));
                    }
                    s
                })
                .collect::<Vec<_>>()
                .join("\n\n---\n\n")
        }
    } else {
        "No commit selected".to_string()
    };

    let reasoning = Paragraph::new(reasoning_text)
        .block(Block::default().borders(Borders::ALL).title(" Reasoning "))
        .wrap(Wrap { trim: false });

    frame.render_widget(reasoning, top_chunks[1]);

    // Diff panel (if visible)
    if has_diff {
        let diff_border = if app.focus == Focus::Diff {
            focused_border
        } else {
            unfocused_border
        };

        let diff_title = match app.diff_mode {
            DiffMode::Stat => " Diff (stat) ",
            DiffMode::Full => " Diff (full) ",
            DiffMode::Hidden => "",
        };

        // Build styled lines from diff_lines
        let lines: Vec<Line> = app
            .diff_lines
            .iter()
            .skip(app.diff_scroll)
            .map(|dl| Line::from(Span::styled(dl.content.clone(), dl.style)))
            .collect();

        let scroll_info = if !app.diff_lines.is_empty() {
            format!(" [{}/{}] ", app.diff_scroll + 1, app.diff_lines.len())
        } else {
            String::new()
        };

        let diff = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(diff_border)
                .title(format!("{}{}", diff_title, scroll_info)),
        );

        frame.render_widget(diff, main_chunks[1]);
    }

    // Help bar at bottom
    let help = " q: quit | j/k: scroll | Tab: focus | d: toggle diff | f/b: page ";
    let help_area = Rect {
        x: 0,
        y: frame.area().height - 1,
        width: frame.area().width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(help).style(Style::default().bg(Color::DarkGray)),
        help_area,
    );
}
