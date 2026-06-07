//! The on-disk shape of a single ARF reasoning record.
//!
//! Records are stored as TOML files keyed by short commit SHA on
//! the `arf` orphan branch (`.arf/records/<sha>/<agent>-<ts>.toml`).
//! The struct here is the canonical Rust representation; everything
//! else (loaders, formatters, exporters) operates on it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ArfRecord {
    pub what: String,
    pub why: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub how: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}
