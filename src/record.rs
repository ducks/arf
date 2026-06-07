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
    /// Files and (optionally) line ranges this record is *about*.
    /// Added in format v0.3. Older records simply omit it; consumers
    /// should treat None as "the record applies to the whole commit
    /// rather than specific lines."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileRef>>,
}

/// A single file-and-optional-line-range reference inside a record.
/// `lines` is an inclusive `[start, end]` pair when present; for a
/// single line set both ends equal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRef {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<[u32; 2]>,
}

impl FileRef {
    /// Parse `--file path[:start[-end]]` syntax into a FileRef.
    /// Three accepted forms:
    ///   `src/foo.rs`           -> file-only, lines = None
    ///   `src/foo.rs:42`        -> single-line, lines = Some([42, 42])
    ///   `src/foo.rs:42-76`     -> range, lines = Some([42, 76])
    pub fn parse(s: &str) -> Result<Self, String> {
        // The last colon splits path from line spec; this leaves any
        // earlier colons (drive letters, scheme-like prefixes) alone.
        match s.rsplit_once(':') {
            None => Ok(FileRef {
                path: s.to_string(),
                lines: None,
            }),
            Some((path, line_spec)) => {
                // If the segment after the last colon isn't a line
                // spec, treat the whole input as the path (e.g. a
                // Windows path "C:\foo" - we'd land here once we
                // strip the actual line argument elsewhere, but be
                // defensive).
                if line_spec.is_empty() || !line_spec.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    return Ok(FileRef {
                        path: s.to_string(),
                        lines: None,
                    });
                }
                let (start, end) = match line_spec.split_once('-') {
                    None => {
                        let n: u32 = line_spec
                            .parse()
                            .map_err(|_| format!("invalid line spec: {:?}", line_spec))?;
                        (n, n)
                    }
                    Some((a, b)) => {
                        let a: u32 = a
                            .parse()
                            .map_err(|_| format!("invalid start: {:?}", a))?;
                        let b: u32 = b
                            .parse()
                            .map_err(|_| format!("invalid end: {:?}", b))?;
                        if a > b {
                            return Err(format!(
                                "start ({}) > end ({}) in line range",
                                a, b
                            ));
                        }
                        (a, b)
                    }
                };
                Ok(FileRef {
                    path: path.to_string(),
                    lines: Some([start, end]),
                })
            }
        }
    }

    /// Whether this FileRef "covers" the given file path + line. A
    /// FileRef with lines = None covers any line in the file (the
    /// whole file is referenced). Path comparison is exact-string.
    pub fn covers(&self, path: &str, line: u32) -> bool {
        if self.path != path {
            return false;
        }
        match self.lines {
            None => true,
            Some([start, end]) => line >= start && line <= end,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_path_only() {
        let r = FileRef::parse("src/foo.rs").unwrap();
        assert_eq!(r.path, "src/foo.rs");
        assert_eq!(r.lines, None);
    }

    #[test]
    fn parse_single_line() {
        let r = FileRef::parse("src/foo.rs:42").unwrap();
        assert_eq!(r.path, "src/foo.rs");
        assert_eq!(r.lines, Some([42, 42]));
    }

    #[test]
    fn parse_line_range() {
        let r = FileRef::parse("src/foo.rs:10-30").unwrap();
        assert_eq!(r.path, "src/foo.rs");
        assert_eq!(r.lines, Some([10, 30]));
    }

    #[test]
    fn parse_rejects_reversed_range() {
        let err = FileRef::parse("src/foo.rs:30-10").unwrap_err();
        assert!(err.contains("start"));
    }

    #[test]
    fn covers_handles_path_only_ref() {
        let r = FileRef {
            path: "src/foo.rs".to_string(),
            lines: None,
        };
        assert!(r.covers("src/foo.rs", 1));
        assert!(r.covers("src/foo.rs", 99999));
        assert!(!r.covers("src/bar.rs", 1));
    }

    #[test]
    fn covers_handles_range() {
        let r = FileRef {
            path: "src/foo.rs".to_string(),
            lines: Some([10, 20]),
        };
        assert!(r.covers("src/foo.rs", 10));
        assert!(r.covers("src/foo.rs", 15));
        assert!(r.covers("src/foo.rs", 20));
        assert!(!r.covers("src/foo.rs", 9));
        assert!(!r.covers("src/foo.rs", 21));
        assert!(!r.covers("src/bar.rs", 15));
    }
}
