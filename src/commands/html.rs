//! HTML output for `arf export --format html`.
//!
//! Step 1 scaffolding: writes a stub index.html into the output
//! directory. Confirms the pipeline (validate output, create dir,
//! write file). Step 2 adds real Tera templates + per-commit pages.

use crate::record::ArfRecord;
use anyhow::{Context, Result};
use std::path::Path;

/// Render the given records into a static HTML site at `out_dir`.
///
/// Creates the output directory if it doesn't exist. Will not
/// overwrite an existing index.html silently - this is the
/// scaffolding stub; the real implementation will be more
/// considerate of pre-existing files.
pub fn render(records: &[&ArfRecord], out_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("creating output dir {}", out_dir.display()))?;

    // Minimal placeholder so the pipeline is end-to-end. Step 2
    // replaces this with a real Tera template that renders the
    // timeline, per-commit views, etc.
    let body = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <title>ARF trail</title>\n</head>\n<body>\n  <h1>ARF trail</h1>\n  <p>{} record(s). Real rendering ships in the next iteration.</p>\n</body>\n</html>\n",
        records.len()
    );

    let index = out_dir.join("index.html");
    std::fs::write(&index, body)
        .with_context(|| format!("writing {}", index.display()))?;

    Ok(())
}
