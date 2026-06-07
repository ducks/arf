//! `arf sync` - push/pull the orphan branch from origin.

use crate::store::ARF_BRANCH;
use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Command;

pub fn run(push: bool, pull: bool) -> Result<()> {
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
