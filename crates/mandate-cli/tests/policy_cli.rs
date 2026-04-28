//! Integration tests for `mandate policy {validate,current,activate,diff}`
//! (PSM-A3).
//!
//! Drives the real `mandate` binary end-to-end against tempfile-backed
//! SQLite databases. Covers the full lifecycle plus the explicit
//! "honest no-active" path, the invalid-policy path, the idempotent
//! re-activate path, and the second-activate-replaces-first path.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mandate"))
}

fn ref_policy() -> PathBuf {
    // The repo-root reference policy. Tests run with cwd = the workspace
    // root, so a relative path is fine; we resolve it once for clarity.
    PathBuf::from("../../test-corpus/policy/reference_low_risk.json")
}

/// Read the reference policy and produce a structurally-different
/// variant by adding a second agent. The result is still a valid
/// `Policy` (parses + validates) but has a different canonical hash.
fn write_variant_policy(tmp: &Path, label: &str) -> PathBuf {
    let raw = std::fs::read_to_string(ref_policy()).expect("read ref policy");
    let mut value: serde_json::Value = serde_json::from_str(&raw).expect("parse ref policy");
    let agents = value
        .get_mut("agents")
        .and_then(|v| v.as_array_mut())
        .expect("agents array");
    agents.push(serde_json::json!({
        "agent_id": format!("variant-agent-{label}"),
        "status": "active",
        "policy_role": "research"
    }));
    let out_path = tmp.join(format!("variant-{label}.json"));
    std::fs::write(&out_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    out_path
}

/// Path to a JSON file that fails policy validation. We deliberately
/// duplicate an agent id (which `Policy::validate()` rejects) so the
/// failure is structural, not a JSON parse error.
fn write_invalid_policy(tmp: &Path) -> PathBuf {
    let raw = std::fs::read_to_string(ref_policy()).expect("read ref policy");
    let mut value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let agents = value.get_mut("agents").unwrap().as_array_mut().unwrap();
    let dup = agents[0].clone();
    agents.push(dup);
    let path = tmp.join("invalid-duplicate-agent.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    path
}

#[test]
fn validate_prints_hash_and_summary_for_valid_policy() {
    let out = Command::new(cli_bin())
        .args(["policy", "validate"])
        .arg(ref_policy())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "validate must succeed; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ok: policy parses"), "got: {stdout}");
    assert!(
        stdout.contains("policy_hash:"),
        "must show hash; got: {stdout}"
    );
    // The reference policy has a stable canonical hash — pin it so any
    // future tampering with serialisation surfaces here. This is the
    // same constant baked into `crates/mandate-identity/src/ens.rs:89`.
    assert!(
        stdout.contains("e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf"),
        "hash must match the embedded reference baseline; got: {stdout}"
    );
}

#[test]
fn validate_rejects_invalid_policy_with_exit_two() {
    let tmp = tempfile::tempdir().unwrap();
    let bad = write_invalid_policy(tmp.path());
    let out = Command::new(cli_bin())
        .args(["policy", "validate"])
        .arg(&bad)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "validate must fail on invalid policy"
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "exit 2 = invalid policy (vs exit 1 = file IO)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("invalid"),
        "stderr must explain failure; got: {stderr}"
    );
}

#[test]
fn current_with_no_active_policy_returns_exit_three() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("mandate.sqlite");
    // Seed an empty DB (so V006 has run) without putting any policy in
    // it. `mandate key list --mock --db <path>` opens the DB, runs
    // every migration, finds no keys, and exits 0. After this the
    // `active_policy` table is present but empty — exactly the shape
    // `policy current` is supposed to surface honestly.
    let probe = Command::new(cli_bin())
        .args(["key", "list", "--mock", "--db"])
        .arg(&db)
        .output()
        .unwrap();
    assert!(
        probe.status.success(),
        "DB-priming probe must succeed; stderr={}",
        String::from_utf8_lossy(&probe.stderr)
    );
    assert!(db.exists(), "probe must have created the DB file");

    let out = Command::new(cli_bin())
        .args(["policy", "current", "--db"])
        .arg(&db)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "current must fail when nothing active"
    );
    assert_eq!(
        out.status.code(),
        Some(3),
        "exit 3 = honest no-active (vs 1 = real error)"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no active policy"),
        "stdout must say so honestly; got: {stdout}"
    );
}

#[test]
fn activate_seeds_first_version_and_current_reports_it() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("mandate.sqlite");

    let activate = Command::new(cli_bin())
        .args(["policy", "activate"])
        .arg(ref_policy())
        .args(["--db"])
        .arg(&db)
        .output()
        .unwrap();
    assert!(
        activate.status.success(),
        "activate must succeed; stderr={}",
        String::from_utf8_lossy(&activate.stderr)
    );
    let act_stdout = String::from_utf8_lossy(&activate.stdout);
    assert!(act_stdout.contains("activated:"));
    assert!(act_stdout.contains("version=v1"));
    assert!(act_stdout.contains("source=operator-cli"));

    let current = Command::new(cli_bin())
        .args(["policy", "current", "--db"])
        .arg(&db)
        .output()
        .unwrap();
    assert!(current.status.success());
    let cur_stdout = String::from_utf8_lossy(&current.stdout);
    assert!(cur_stdout.contains("active policy:"));
    assert!(cur_stdout.contains("version:       v1"));
}

#[test]
fn activate_is_idempotent_for_same_policy() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("mandate.sqlite");
    let first = Command::new(cli_bin())
        .args(["policy", "activate"])
        .arg(ref_policy())
        .args(["--db"])
        .arg(&db)
        .output()
        .unwrap();
    assert!(first.status.success());
    let second = Command::new(cli_bin())
        .args(["policy", "activate"])
        .arg(ref_policy())
        .args(["--db"])
        .arg(&db)
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "second activate must be idempotent; stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains("already active") && stdout.contains("v1"),
        "second activate must report no-op; got: {stdout}"
    );
}

#[test]
fn activate_different_policy_replaces_active_row() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("mandate.sqlite");
    let v2 = write_variant_policy(tmp.path(), "v2");

    let _ = Command::new(cli_bin())
        .args(["policy", "activate"])
        .arg(ref_policy())
        .args(["--db"])
        .arg(&db)
        .output()
        .unwrap();
    let second = Command::new(cli_bin())
        .args(["policy", "activate"])
        .arg(&v2)
        .args(["--db"])
        .arg(&db)
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "v2 activate must succeed; stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(stdout.contains("activated:"));
    assert!(stdout.contains("version=v2"));

    // current must now reflect v2.
    let current = Command::new(cli_bin())
        .args(["policy", "current", "--db"])
        .arg(&db)
        .output()
        .unwrap();
    assert!(current.status.success());
    let cur_stdout = String::from_utf8_lossy(&current.stdout);
    assert!(cur_stdout.contains("version:       v2"));
}

#[test]
fn diff_returns_zero_for_identical_files() {
    let out = Command::new(cli_bin())
        .args(["policy", "diff"])
        .arg(ref_policy())
        .arg(ref_policy())
        .output()
        .unwrap();
    assert!(out.status.success(), "identical files must diff to exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no differences"),
        "stdout must say no differences; got: {stdout}"
    );
}

#[test]
fn diff_returns_one_with_diff_for_different_files() {
    let tmp = tempfile::tempdir().unwrap();
    let v2 = write_variant_policy(tmp.path(), "diff-test");
    let out = Command::new(cli_bin())
        .args(["policy", "diff"])
        .arg(ref_policy())
        .arg(&v2)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "different files must diff to non-zero"
    );
    assert_eq!(out.status.code(), Some(1), "exit 1 = differs (with diff)");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("policies differ:"), "got: {stdout}");
    // The diff should reference the added agent_id.
    assert!(
        stdout.contains("variant-agent-diff-test"),
        "diff must surface the difference; got: {stdout}"
    );
}

#[test]
fn diff_returns_two_when_either_policy_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let bad = write_invalid_policy(tmp.path());
    let out = Command::new(cli_bin())
        .args(["policy", "diff"])
        .arg(&bad)
        .arg(ref_policy())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(
        out.status.code(),
        Some(2),
        "exit 2 = invalid policy file (left or right)"
    );
}
