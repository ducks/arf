//! `arf init` - set up the orphan branch and worktree.
//!
//! Four states to handle:
//!
//!   1. Local arf branch exists AND .arf worktree exists -> nothing
//!      to do, success.
//!   2. Local arf branch exists, no worktree -> mount it. This
//!      happens when a prior init dropped the worktree somehow.
//!   3. No local branch, but origin/arf exists -> fetch and check
//!      out the existing branch into a worktree. This is the
//!      "fresh clone of a repo someone else already arf-init'd"
//!      case, which earlier versions failed on.
//!   4. No branch anywhere -> create an orphan branch and seed it
//!      with the README. The original-init path.

use crate::store::ARF_BRANCH;
use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Command;

pub fn run() -> Result<()> {
    // Check if we're in a git repo
    let status = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()?;

    if !status.status.success() {
        return Err(anyhow!("Not a git repository. Run 'git init' first."));
    }

    let local_branch_exists = Command::new("git")
        .args(["rev-parse", "--verify", ARF_BRANCH])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let worktree_exists = Path::new(".arf").is_dir();

    // Case 1: everything already in place.
    if local_branch_exists && worktree_exists {
        println!("✓ ARF branch '{}' already initialized", ARF_BRANCH);
        return Ok(());
    }

    // Case 2: branch exists locally but no worktree. Mount it.
    if local_branch_exists && !worktree_exists {
        let mount = Command::new("git")
            .args(["worktree", "add", ".arf", ARF_BRANCH])
            .output()?;
        if !mount.status.success() {
            return Err(anyhow!(
                "Failed to mount existing ARF branch: {}",
                String::from_utf8_lossy(&mount.stderr)
            ));
        }
        println!("✓ Mounted existing ARF branch at .arf/");
        return Ok(());
    }

    // Case 3: branch exists on origin (e.g. fresh clone of a repo
    // someone already initialized). Fetch and check out into a
    // worktree, tracking the remote.
    let remote_ref = format!("origin/{}", ARF_BRANCH);
    let remote_exists = Command::new("git")
        .args(["rev-parse", "--verify", &remote_ref])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if remote_exists {
        let mount = Command::new("git")
            .args(["worktree", "add", ".arf", ARF_BRANCH])
            .output()?;
        if !mount.status.success() {
            return Err(anyhow!(
                "Failed to attach to remote ARF branch: {}",
                String::from_utf8_lossy(&mount.stderr)
            ));
        }
        println!("✓ Attached to existing ARF branch from origin");
        println!("✓ Mounted at .arf/");
        return Ok(());
    }

    // Case 4: no branch anywhere. Create the orphan.
    println!("Initializing ARF...");

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
