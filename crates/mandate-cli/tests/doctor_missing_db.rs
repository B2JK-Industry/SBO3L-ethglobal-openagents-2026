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

/// Codex P2 on PR #32: permission/fs metadata errors must surface as a
/// `storage_open` fail, NOT mislabel the path as "does not exist". We
/// construct that shape on Unix by chmod'ing a parent directory to 000
/// so `try_exists()` of a child path returns `Err(EACCES)`. The doctor
/// should fall through to `Storage::open`, which then hits the real
/// OS-level error and reports it via the existing `storage_open` fail
/// path.
///
/// Gated on `cfg(unix)` because chmod(000) semantics differ on Windows.
/// We probe at runtime for "are mode bits actually being enforced for
/// this user?" by trying to stat a child of the locked dir; if stat
/// succeeds we're effectively root (or running on a filesystem that
/// ignores mode bits) and skip rather than fail.
#[cfg(unix)]
#[test]
fn doctor_permission_denied_falls_through_to_storage_open_fail() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let locked = tmp.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    let target = locked.join("would-be-db.sqlite");

    // Lock the parent directory: 000 means no read, no write, no
    // execute → child stat fails with EACCES, which is the exact shape
    // try_exists() converts to Err.
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

    // Confirm the lock is actually being enforced. If the caller is
    // root (or the FS ignores mode bits), `try_exists` returns Ok and
    // this test cannot exercise the Err branch — skip cleanly.
    let lock_is_effective = std::path::Path::new(&target).try_exists().is_err();
    if !lock_is_effective {
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!(
            "skipping: chmod(000) not enforced for this caller \
             (likely running as root or a permissive FS)"
        );
        return;
    }

    let out = Command::new(cli_bin())
        .arg("doctor")
        .arg("--json")
        .arg("--db")
        .arg(&target)
        .output()
        .expect("failed to spawn mandate doctor");

    // Restore permissions BEFORE asserting so tempdir cleanup works
    // even if the assertions fail.
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(
        out.status.code(),
        Some(2),
        "doctor must still exit non-zero on metadata errors; stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout)
        .expect("doctor --json must emit valid JSON on permission-denied");
    assert_eq!(v["overall"], "fail");
    let checks = v["checks"].as_array().expect("checks array");
    let storage_open = checks
        .iter()
        .find(|c| c["name"] == "storage_open")
        .expect("storage_open row required when fs metadata cannot be read");
    let err_msg = storage_open["error"].as_str().unwrap_or("");
    assert!(
        !err_msg.contains("does not exist"),
        "permission-denied must NOT be mislabelled as 'does not exist'; got error={err_msg:?}"
    );
    assert!(
        !target.exists(),
        "doctor must still leave the target untouched on the permission path"
    );
}
