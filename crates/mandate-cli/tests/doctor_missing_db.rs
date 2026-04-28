//! Regression test: `mandate doctor --db <missing>` must NOT create the
//! file. Codex P1 review on PR #25 flagged that `Storage::open` is the
//! rusqlite default which silently creates a fresh SQLite file and runs
//! migrations against it — a doctor that mutates an operator's
//! filesystem and reports "ok" against a DB that never existed is the
//! opposite of inspection-only. This test pins the corrected behaviour:
//! exit code 2, stderr explains what happened, and no file is created
//! at the requested path.

use std::path::PathBuf;
use std::process::Command;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mandate"))
}

#[test]
fn doctor_does_not_create_missing_db_file() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("does-not-exist.sqlite");
    assert!(
        !missing.exists(),
        "precondition: target path must not exist before doctor runs"
    );

    let out = Command::new(cli_bin())
        .arg("doctor")
        .arg("--db")
        .arg(&missing)
        .output()
        .expect("failed to spawn mandate doctor");

    assert!(
        !out.status.success(),
        "doctor must not succeed against a missing DB; stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit code 2 (cannot open db); got {:?}",
        out.status.code(),
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("does not exist"),
        "stderr must explain the missing DB path; got: {stderr}"
    );

    assert!(
        !missing.exists(),
        "doctor must NOT create the SQLite file at the requested path; \
         this is the inspection-only invariant Codex P1 flagged"
    );
    let entries = std::fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    assert!(
        entries.is_empty(),
        "doctor must leave the parent directory untouched; found: {entries:?}"
    );
}

#[test]
fn doctor_json_mode_against_missing_db_emits_fail_envelope() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("nope.sqlite");

    let out = Command::new(cli_bin())
        .arg("doctor")
        .arg("--json")
        .arg("--db")
        .arg(&missing)
        .output()
        .expect("failed to spawn mandate doctor");

    assert_eq!(out.status.code(), Some(2));
    assert!(!missing.exists(), "json mode must also be inspection-only");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value =
        serde_json::from_str(&stdout).expect("doctor --json must emit valid JSON even on failure");
    assert_eq!(v["report_type"], "mandate.doctor.v1");
    assert_eq!(v["overall"], "fail");
    let checks = v["checks"].as_array().expect("checks array");
    assert!(
        checks
            .iter()
            .any(|c| c["name"] == "storage_open" && c["status"] == "fail"),
        "expected a storage_open fail row; got {checks:?}"
    );
}
