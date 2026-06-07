//! `arf spec list` / `arf spec show <name>` - manage the .arf/specs/
//! task definitions that lok/finna and others drop into the orphan
//! branch.

use anyhow::{anyhow, Result};
use std::path::Path;

pub fn list() -> Result<()> {
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

pub fn show(name: &str) -> Result<()> {
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
