//! `arf init` - create the orphan branch and mount it as a worktree.

use crate::store::ARF_BRANCH;
use anyhow::{anyhow, Result};
use std::process::Command;

pub fn run() -> Result<()> {
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
