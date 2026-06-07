//! End-to-end tests against the built arf binary.
//!
//! Each test spins up an isolated tempdir, runs `git init` + (often)
//! `arf init`, makes some commits, and then exercises one subcommand.
//! Assertions are on stdout / filesystem layout, not on internal Rust
//! types - the goal is "the CLI does what its users see," not "this
//! private function returns the right value."
//!
//! Slow but trustworthy: each test is one process spawn per command,
//! ~50ms overhead. At a couple dozen tests, the whole suite runs in
//! 1-3 seconds.

use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::str;

/// Build a fresh tempdir with `git init` already done plus a single
/// commit on `main`. Returns the temp directory handle so the caller
/// can run further commands against it; the directory is cleaned up
/// when the handle drops.
fn git_repo() -> assert_fs::TempDir {
    let dir = assert_fs::TempDir::new().unwrap();

    // git init + identity (CI envs often have no global user config).
    run(&dir, "git", &["init", "-q", "-b", "main"]);
    run(&dir, "git", &["config", "user.email", "test@example.com"]);
    run(&dir, "git", &["config", "user.name", "test"]);

    // Empty commit so HEAD exists - `arf record` resolves HEAD to
    // attach records to a SHA.
    run(
        &dir,
        "git",
        &["commit", "--allow-empty", "-q", "-m", "initial"],
    );

    dir
}

/// Same as `git_repo` but also runs `arf init`. Most tests want this.
fn arf_repo() -> assert_fs::TempDir {
    let dir = git_repo();
    arf(&dir).args(["init"]).assert().success();
    dir
}

fn arf(dir: &assert_fs::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("arf").unwrap();
    cmd.current_dir(dir.path());
    cmd
}

fn run(dir: &assert_fs::TempDir, bin: &str, args: &[&str]) {
    std::process::Command::new(bin)
        .args(args)
        .current_dir(dir.path())
        .output()
        .expect("command spawn failed");
}

#[test]
fn init_succeeds_in_fresh_git_repo() {
    let dir = git_repo();
    arf(&dir).args(["init"]).assert().success();
    dir.child(".arf").assert(predicates::path::is_dir());
    dir.child(".arf/records").assert(predicates::path::is_dir());
}

#[test]
fn init_errors_outside_git_repo() {
    let dir = assert_fs::TempDir::new().unwrap();
    arf(&dir)
        .args(["init"])
        .assert()
        .failure()
        .stderr(str::contains("Not a git repository"));
}

#[test]
fn init_is_idempotent() {
    let dir = git_repo();
    arf(&dir).args(["init"]).assert().success();
    // Second call shouldn't error - it should just no-op.
    arf(&dir).args(["init"]).assert().success();
}

#[test]
fn record_writes_a_file_under_records_dir() {
    let dir = arf_repo();
    arf(&dir)
        .args([
            "record",
            "--what",
            "Add retry logic",
            "--why",
            "Transient API failures",
        ])
        .assert()
        .success();

    // .arf/records/<8-char-sha>/<timestamp>.toml should exist.
    let records = std::fs::read_dir(dir.path().join(".arf/records"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .count();
    assert_eq!(records, 1, "expected one commit-keyed directory");
}

#[test]
fn record_errors_when_arf_not_initialized() {
    let dir = git_repo();
    arf(&dir)
        .args(["record", "--what", "x", "--why", "y"])
        .assert()
        .failure()
        .stderr(str::contains("ARF not initialized"));
}

#[test]
fn log_shows_recorded_data() {
    let dir = arf_repo();
    arf(&dir)
        .args([
            "record",
            "--what",
            "Add caching layer",
            "--why",
            "Hot path is read-heavy",
        ])
        .assert()
        .success();

    arf(&dir)
        .args(["log"])
        .assert()
        .success()
        .stdout(str::contains("Add caching layer"))
        .stdout(str::contains("Hot path is read-heavy"));
}

#[test]
fn log_reports_empty_when_no_records() {
    let dir = arf_repo();
    arf(&dir)
        .args(["log"])
        .assert()
        .success()
        .stdout(str::contains("No ARF records found"));
}

#[test]
fn export_json_contains_recorded_record() {
    let dir = arf_repo();
    arf(&dir)
        .args([
            "record",
            "--what",
            "Refactor user model",
            "--why",
            "Decouple from session store",
        ])
        .assert()
        .success();

    let out = arf(&dir).args(["export"]).assert().success();
    let stdout = std::str::from_utf8(&out.get_output().stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(stdout).expect("export should produce valid JSON");
    let records = parsed.as_array().expect("export should emit an array");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["what"], "Refactor user model");
    assert_eq!(records[0]["why"], "Decouple from session store");
}

#[test]
fn export_jsonl_emits_one_record_per_line() {
    let dir = arf_repo();
    for i in 0..3 {
        arf(&dir)
            .args([
                "record",
                "--what",
                &format!("Step {}", i),
                "--why",
                "Testing",
            ])
            .assert()
            .success();
    }

    let out = arf(&dir)
        .args(["export", "--format", "jsonl"])
        .assert()
        .success();
    let stdout = std::str::from_utf8(&out.get_output().stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 3);
    for line in lines {
        serde_json::from_str::<serde_json::Value>(line).expect("each jsonl line should be valid JSON");
    }
}

#[test]
fn export_toml_round_trips() {
    let dir = arf_repo();
    arf(&dir)
        .args([
            "record",
            "--what",
            "Compress backups",
            "--why",
            "S3 storage costs",
        ])
        .assert()
        .success();

    let out = arf(&dir)
        .args(["export", "--format", "toml"])
        .assert()
        .success();
    let stdout = std::str::from_utf8(&out.get_output().stdout).unwrap();
    assert!(stdout.contains("[[records]]"));
    assert!(stdout.contains("Compress backups"));
    // Round-trip through toml parsing to ensure it's well-formed.
    let _parsed: toml::Value = toml::from_str(stdout).expect("export --format toml should produce valid TOML");
}

#[test]
fn export_since_filters_by_timestamp() {
    let dir = arf_repo();
    // Record once, then again - they'll have different timestamps.
    arf(&dir)
        .args(["record", "--what", "Old change", "--why", "Test"])
        .assert()
        .success();

    // Use a future cutoff to filter out the (just-now) records.
    let out = arf(&dir)
        .args(["export", "--since", "2099-01-01"])
        .assert()
        .success();
    let stdout = std::str::from_utf8(&out.get_output().stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(stdout).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 0);

    // Past cutoff should include the record.
    let out = arf(&dir)
        .args(["export", "--since", "2000-01-01"])
        .assert()
        .success();
    let stdout = std::str::from_utf8(&out.get_output().stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(stdout).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 1);
}

#[test]
fn export_commit_scopes_to_one_sha() {
    let dir = arf_repo();
    arf(&dir)
        .args(["record", "--what", "First", "--why", "Initial"])
        .assert()
        .success();

    // Make a second commit + another record.
    run(
        &dir,
        "git",
        &["commit", "--allow-empty", "-q", "-m", "second"],
    );
    arf(&dir)
        .args(["record", "--what", "Second", "--why", "Followup"])
        .assert()
        .success();

    // Verify both records exist unscoped.
    let out = arf(&dir).args(["export"]).assert().success();
    let stdout = std::str::from_utf8(&out.get_output().stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(stdout).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 2);

    // Get HEAD~1's SHA and verify --commit scopes to just it.
    let head_minus_one = String::from_utf8(
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD~1"])
            .current_dir(dir.path())
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    let out = arf(&dir)
        .args(["export", "--commit", &head_minus_one])
        .assert()
        .success();
    let stdout = std::str::from_utf8(&out.get_output().stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(stdout).unwrap();
    let records = parsed.as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["what"], "First");
}

#[test]
fn export_empty_when_no_records() {
    let dir = arf_repo();
    let out = arf(&dir).args(["export"]).assert().success();
    let stdout = std::str::from_utf8(&out.get_output().stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(stdout).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 0);
}
