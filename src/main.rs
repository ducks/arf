use anyhow::Result;
use arf_cli::commands;
use arf_cli::commands::export::ExportFormat;
use clap::{Parser, Subcommand};

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

    /// Dump records to stdout for piping into other tooling
    Export {
        /// Only export records linked to this commit
        #[arg(short, long)]
        commit: Option<String>,

        /// Only export records with timestamp >= this RFC-3339 date/time
        /// (e.g. "2026-06-01" or "2026-06-01T12:00:00Z")
        #[arg(long)]
        since: Option<String>,

        /// Output format: json (default), jsonl (one record per line), toml
        #[arg(short, long, default_value = "json")]
        format: ExportFormat,
    },
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => commands::init::run()?,
        Commands::Spec { command } => match command {
            SpecCommands::List => commands::spec::list()?,
            SpecCommands::Show { name } => commands::spec::show(&name)?,
        },
        Commands::Record {
            what,
            why,
            how,
            backup,
            commit,
        } => commands::record::run(what, why, how, backup, commit)?,
        Commands::Log { commit, limit } => commands::log::run(commit, limit)?,
        Commands::Sync { push, pull } => commands::sync::run(push, pull)?,
        Commands::Graph { limit } => commands::graph::run(limit)?,
        Commands::Diff { commit, full } => commands::diff::run(commit, full)?,
        Commands::Browse => commands::browse::run()?,
        Commands::Export {
            commit,
            since,
            format,
        } => commands::export::run(commit, since, format)?,
    }

    Ok(())
}
