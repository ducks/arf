//! HTML output for `arf export --format html`.
//!
//! Renders a single static `index.html` (plus `style.css`) that
//! groups records by commit and inlines a syntax-highlighted diff
//! pulled from `git show`. Templates are bundled into the binary via
//! `include_str!` so the tool stays a single drop-in file.
//!
//! Diff highlighting is intentionally simple: line-prefix matching
//! into add/del/hunk spans. We escape HTML before wrapping so a
//! literal `<` in a diff body can't break the page.

use crate::record::ArfRecord;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use tera::{Context as TeraContext, Tera};

const TEMPLATE_INDEX: &str = include_str!("html_templates/index.html");
const STYLE_CSS: &str = include_str!("html_templates/style.css");

pub fn render(records: &[&ArfRecord], out_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("creating output dir {}", out_dir.display()))?;

    let mut tera = Tera::default();
    tera.add_raw_template("index.html", TEMPLATE_INDEX)
        .context("loading index template")?;

    // Group by commit SHA, preserving a stable order. Records without
    // a linked commit land in an "(unlinked)" bucket so they're still
    // visible rather than silently dropped.
    let mut by_commit: BTreeMap<String, Vec<&ArfRecord>> = BTreeMap::new();
    for r in records {
        let key = r.commit.clone().unwrap_or_else(|| "(unlinked)".to_string());
        by_commit.entry(key).or_default().push(r);
    }

    let mut commits: Vec<serde_json::Value> = Vec::with_capacity(by_commit.len());
    for (sha, recs) in by_commit.iter() {
        let (subject, date, diff_html) = if sha == "(unlinked)" {
            (String::from("Records without a linked commit"), String::new(), String::new())
        } else {
            let (subj, dt) = git_meta(sha).unwrap_or_else(|| (String::new(), String::new()));
            let diff = git_diff(sha).unwrap_or_default();
            (subj, dt, highlight_diff(&diff))
        };

        let recs_json: Vec<serde_json::Value> = recs
            .iter()
            .map(|r| {
                let files: Vec<String> = r
                    .files
                    .as_ref()
                    .map(|fs| {
                        fs.iter()
                            .map(|f| match f.lines {
                                None => f.path.clone(),
                                Some([a, b]) if a == b => format!("{}:{}", f.path, a),
                                Some([a, b]) => format!("{}:{}-{}", f.path, a, b),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                serde_json::json!({
                    "what": r.what,
                    "why": r.why,
                    "how": r.how,
                    "backup": r.backup,
                    "outcome": r.outcome,
                    "timestamp": r.timestamp,
                    "agent": r.agent,
                    "files": files,
                })
            })
            .collect();

        let short = if sha.len() >= 8 { &sha[..8] } else { sha.as_str() };
        commits.push(serde_json::json!({
            "sha": sha,
            "short_sha": short,
            "subject": subject,
            "date": date,
            "records": recs_json,
            "diff_html": diff_html,
        }));
    }

    let mut ctx = TeraContext::new();
    ctx.insert("total", &records.len());
    ctx.insert("commits", &commits);
    ctx.insert("generated_at", &chrono::Utc::now().to_rfc3339());

    let html = tera.render("index.html", &ctx).context("rendering index.html")?;

    let index_path = out_dir.join("index.html");
    std::fs::write(&index_path, html)
        .with_context(|| format!("writing {}", index_path.display()))?;

    let css_path = out_dir.join("style.css");
    std::fs::write(&css_path, STYLE_CSS)
        .with_context(|| format!("writing {}", css_path.display()))?;

    Ok(())
}

fn git_meta(sha: &str) -> Option<(String, String)> {
    let out = Command::new("git")
        .args(["show", "-s", "--format=%s%n%cI", sha])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let mut lines = s.lines();
    let subject = lines.next().unwrap_or("").to_string();
    let date = lines.next().unwrap_or("").to_string();
    Some((subject, date))
}

fn git_diff(sha: &str) -> Option<String> {
    let out = Command::new("git")
        .args(["show", "--no-color", "--format=", sha])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn highlight_diff(diff: &str) -> String {
    let mut buf = String::with_capacity(diff.len());
    for line in diff.lines() {
        let escaped = escape_html(line);
        let class = if line.starts_with("+++") || line.starts_with("---") {
            None
        } else if line.starts_with('+') {
            Some("add")
        } else if line.starts_with('-') {
            Some("del")
        } else if line.starts_with("@@") {
            Some("hunk")
        } else {
            None
        };
        match class {
            Some(c) => buf.push_str(&format!("<span class=\"{}\">{}</span>\n", c, escaped)),
            None => {
                buf.push_str(&escaped);
                buf.push('\n');
            }
        }
    }
    buf
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}
