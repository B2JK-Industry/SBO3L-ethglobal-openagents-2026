//! Integration tests for `sbo3l key {init,list,rotate} --mock` (PSM-A1.9).
//!
//! Exercises the real `sbo3l` binary end-to-end: each test builds a
//! tempfile-backed SQLite db, drives the CLI with `Command::new(cli_bin())`,
//! and asserts on stdout/stderr/exit-status of the actual process.

use std::path::PathBuf;
use std::process::Command;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sbo3l"))
}

/// 32-byte deterministic seed used across all CLI integration tests here.
/// **Mock** — never used outside the test suite.
const TEST_ROOT_SEED: &str = "2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a";

/// A different deterministic seed used to exercise the
/// "mismatched-seed rejected at rotate" path.
const OTHER_ROOT_SEED: &str = "5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b";

#[test]
fn init_writes_v1_row_then_list_shows_it() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("sbo3l.sqlite");

    let init = Command::new(cli_bin())
        .args([
            "key",
            "init",
            "--mock",
            "--role",
            "audit-mock",
            "--root-seed",
            TEST_ROOT_SEED,
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "init must succeed; stderr={}",
        String::from_utf8_lossy(&init.stderr)
    );
    let init_out = String::from_utf8_lossy(&init.stdout);
    assert!(
        init_out.contains("mock-kms"),
        "init output must lead with `mock-kms` disclosure; got: {init_out}"
    );

    let list = Command::new(cli_bin())
        .args(["key", "list", "--mock", "--db", db.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(
        stdout.contains("audit-mock"),
        "list should show audit-mock; got: {stdout}"
    );
    assert!(
        stdout.contains("audit-mock-v1"),
        "list should show key_id; got: {stdout}"
    );
}

#[test]
fn init_is_idempotent_on_repeat() {
    // Running the same init twice should NOT fail. The second run
    // reports the existing row; both runs exit 0. Catches a regression
    // where the duplicate-row path returned non-zero.
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("sbo3l.sqlite");
    let args = [
        "key",
        "init",
        "--mock",
        "--role",
        "audit-mock",
        "--root-seed",
        TEST_ROOT_SEED,
        "--db",
        db.to_str().unwrap(),
    ];
    let first = Command::new(cli_bin()).args(args).output().unwrap();
    assert!(first.status.success());
    let second = Command::new(cli_bin()).args(args).output().unwrap();
    assert!(
        second.status.success(),
        "second init must be idempotent; stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains("already initialised"),
        "expected idempotency message; got: {stdout}"
    );
}

#[test]
fn rotate_advances_version_and_old_version_remains_listed() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("sbo3l.sqlite");

    // init v1
    let init = Command::new(cli_bin())
        .args([
            "key",
            "init",
            "--mock",
            "--role",
            "audit-mock",
            "--root-seed",
            TEST_ROOT_SEED,
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(init.status.success());

    // rotate → v2
    let rotate = Command::new(cli_bin())
        .args([
            "key",
            "rotate",
            "--mock",
            "--role",
            "audit-mock",
            "--root-seed",
            TEST_ROOT_SEED,
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        rotate.status.success(),
        "rotate must succeed; stderr={}",
        String::from_utf8_lossy(&rotate.stderr)
    );
    let rotate_out = String::from_utf8_lossy(&rotate.stdout);
    assert!(
        rotate_out.contains("v1 → v2"),
        "rotate should announce version change; got: {rotate_out}"
    );

    // list shows both v1 and v2
    let list = Command::new(cli_bin())
        .args([
            "key",
            "list",
            "--mock",
            "--role",
            "audit-mock",
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("audit-mock-v1"));
    assert!(stdout.contains("audit-mock-v2"));
}

#[test]
fn rotate_without_init_fails_with_clear_message() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("sbo3l.sqlite");
    let out = Command::new(cli_bin())
        .args([
            "key",
            "rotate",
            "--mock",
            "--role",
            "audit-mock",
            "--root-seed",
            TEST_ROOT_SEED,
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success(), "rotate without init must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no keyring exists") || stderr.contains("init"),
        "stderr must point at `init`; got: {stderr}"
    );
}

#[test]
fn list_with_no_keyring_prints_friendly_message() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("sbo3l.sqlite");
    let out = Command::new(cli_bin())
        .args(["key", "list", "--mock", "--db", db.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no keyring entries"),
        "should friendly-message; got: {stdout}"
    );
}

#[test]
fn missing_mock_flag_is_required_by_clap() {
    // Without `--mock` clap itself rejects the call (the bool is a
    // required arg via the always-set default semantics; we accept
    // either clap's "required" message or our own runtime "--mock"
    // disclosure check).
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("sbo3l.sqlite");
    let out = Command::new(cli_bin())
        .args([
            "key",
            "init",
            "--role",
            "audit-mock",
            "--root-seed",
            TEST_ROOT_SEED,
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success(), "init without --mock must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--mock") || stderr.contains("required") || stderr.contains("acknowledge"),
        "stderr should explain --mock; got: {stderr}"
    );
}

#[test]
fn root_seed_with_wrong_length_returns_clear_error() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("sbo3l.sqlite");
    let out = Command::new(cli_bin())
        .args([
            "key",
            "init",
            "--mock",
            "--role",
            "audit-mock",
            "--root-seed",
            "abcd", // 4 chars
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("64 hex chars"),
        "stderr should explain length; got: {stderr}"
    );
}

#[test]
fn list_filters_by_role_when_given() {
    // Init two roles, list with --role only one, assert only that
    // one's rows appear.
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("sbo3l.sqlite");
    for role in ["audit-mock", "decision-mock"] {
        let s = Command::new(cli_bin())
            .args([
                "key",
                "init",
                "--mock",
                "--role",
                role,
                "--root-seed",
                TEST_ROOT_SEED,
                "--db",
                db.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(s.success());
    }
    let list = Command::new(cli_bin())
        .args([
            "key",
            "list",
            "--mock",
            "--role",
            "audit-mock",
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("audit-mock-v1"));
    assert!(
        !stdout.contains("decision-mock-v1"),
        "filtered listing must not leak other roles; got: {stdout}"
    );
}

/// Codex P2 on PR #28: every output line of the non-empty `key list`
/// path must carry the `mock-kms:` disclosure prefix. Previously only
/// the header announced "mock-kms keyring", but the column header and
/// row lines started with whitespace — so a single copy-pasted row
/// could be misread as production KMS output. We seed two roles, run
/// list, and assert that **every non-blank stdout line** begins with
/// `mock-kms:` (including the column header and each data row).
#[test]
fn list_prefixes_every_line_with_mock_kms_disclosure() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("sbo3l.sqlite");
    for role in ["audit-mock", "decision-mock"] {
        let s = Command::new(cli_bin())
            .args([
                "key",
                "init",
                "--mock",
                "--role",
                role,
                "--root-seed",
                TEST_ROOT_SEED,
                "--db",
                db.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(s.success(), "init for role {role} must succeed");
    }
    let list = Command::new(cli_bin())
        .args(["key", "list", "--mock", "--db", db.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);

    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(
        lines.len() >= 4,
        "non-empty list must produce at least header + col-header + 2 row lines; got: {stdout}"
    );
    for (i, line) in lines.iter().enumerate() {
        assert!(
            line.starts_with("mock-kms:"),
            "line {i} ({line:?}) does not start with `mock-kms:`; full stdout:\n{stdout}"
        );
    }
    // And confirm both roles' rows are still present (regression
    // guard: changing the prefix must not have broken the data).
    assert!(stdout.contains("audit-mock-v1"));
    assert!(stdout.contains("decision-mock-v1"));
}

/// Codex P2 on PR #28: rotating with the wrong `--root-seed` must
/// refuse, exit non-zero (we use `2` for "operator passed bad input"),
/// and leave the keyring untouched. Without this check, a typo would
/// silently insert a v2 row whose keys can't be re-derived from the
/// "real" rotation seed — irrecoverable drift in the mock keyring's
/// only authentication assumption.
#[test]
fn rotate_with_wrong_root_seed_refuses_and_does_not_advance() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("sbo3l.sqlite");

    // init v1 with the canonical test seed.
    let init = Command::new(cli_bin())
        .args([
            "key",
            "init",
            "--mock",
            "--role",
            "audit-mock",
            "--root-seed",
            TEST_ROOT_SEED,
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(init.status.success());

    // attempt rotate with the OTHER seed — must fail with exit 2.
    let bad_rotate = Command::new(cli_bin())
        .args([
            "key",
            "rotate",
            "--mock",
            "--role",
            "audit-mock",
            "--root-seed",
            OTHER_ROOT_SEED,
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        !bad_rotate.status.success(),
        "rotate with wrong --root-seed must fail; stdout={} stderr={}",
        String::from_utf8_lossy(&bad_rotate.stdout),
        String::from_utf8_lossy(&bad_rotate.stderr),
    );
    assert_eq!(
        bad_rotate.status.code(),
        Some(2),
        "wrong-seed rotate should return exit 2 (bad operator input)"
    );
    let stderr = String::from_utf8_lossy(&bad_rotate.stderr);
    assert!(
        stderr.contains("--root-seed") && stderr.contains("does not match"),
        "stderr must explain seed mismatch; got: {stderr}"
    );

    // list must still show only v1 — no v2 leaked through.
    let list = Command::new(cli_bin())
        .args([
            "key",
            "list",
            "--mock",
            "--role",
            "audit-mock",
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(list.status.success());
    let list_stdout = String::from_utf8_lossy(&list.stdout);
    assert!(
        list_stdout.contains("audit-mock-v1"),
        "v1 must still be present; got: {list_stdout}"
    );
    assert!(
        !list_stdout.contains("audit-mock-v2"),
        "rejected rotate must NOT have inserted v2; got: {list_stdout}"
    );

    // and a follow-up rotate WITH the correct seed must still succeed.
    let good_rotate = Command::new(cli_bin())
        .args([
            "key",
            "rotate",
            "--mock",
            "--role",
            "audit-mock",
            "--root-seed",
            TEST_ROOT_SEED,
            "--db",
            db.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        good_rotate.status.success(),
        "rotate with the right seed must succeed after a rejected attempt; stderr={}",
        String::from_utf8_lossy(&good_rotate.stderr)
    );
}
