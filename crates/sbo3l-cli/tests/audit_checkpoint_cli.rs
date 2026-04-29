//! Integration tests for `sbo3l audit checkpoint {create,verify}`
//! (PSM-A4).
//!
//! Drives the real `sbo3l` binary end-to-end against tempfile-backed
//! SQLite databases that already carry an audit chain (we use the
//! research-agent harness with `--scenario legit-x402` to populate
//! one without re-implementing the policy/budget pipeline). Covers
//! every code path the user spec calls out: create ok, verify ok
//! (with + without --db), tampered checkpoint rejected, wrong-DB
//! rejected, empty audit DB rejected with exit 3.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sbo3l"))
}

fn agent_bin() -> PathBuf {
    // The research-agent's `run` script lives at the workspace root
    // and shells out to its cargo build. We resolve relative to the
    // sbo3l-cli test crate's working dir, which cargo sets to
    // `crates/sbo3l-cli`.
    PathBuf::from("../../demo-agents/research-agent/run")
}

/// Populate a fresh DB with one allow-path audit event by running the
/// research-agent harness against it. Returns the populated DB path
/// (kept alive by the supplied tempdir).
fn seed_db_with_audit_chain(tmp: &Path, label: &str) -> PathBuf {
    let db = tmp.join(format!("{label}.db"));
    let out = Command::new(agent_bin())
        .args(["--scenario", "legit-x402", "--storage-path"])
        .arg(&db)
        .output()
        .expect("research-agent run must succeed");
    assert!(
        out.status.success(),
        "seeding via research-agent failed: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    db
}

#[test]
fn create_then_verify_against_same_db_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    let db = seed_db_with_audit_chain(tmp.path(), "primary");
    let cp = tmp.path().join("checkpoint.json");

    let create = Command::new(cli_bin())
        .args(["audit", "checkpoint", "create", "--db"])
        .arg(&db)
        .args(["--out"])
        .arg(&cp)
        .output()
        .unwrap();
    assert!(
        create.status.success(),
        "create must succeed; stderr={}",
        String::from_utf8_lossy(&create.stderr)
    );
    let create_stdout = String::from_utf8_lossy(&create.stdout);
    // Truthfulness: every line of the output must carry the
    // `mock-anchor:` prefix so a single copy-pasted line cannot be
    // misread as production-anchor output.
    for (i, line) in create_stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
    {
        assert!(
            line.starts_with("mock-anchor:"),
            "line {i} ({line:?}) does not start with `mock-anchor:`"
        );
    }
    assert!(cp.exists(), "checkpoint JSON must be written");

    // verify (no db) — structural-only.
    let verify_no_db = Command::new(cli_bin())
        .args(["audit", "checkpoint", "verify"])
        .arg(&cp)
        .output()
        .unwrap();
    assert!(
        verify_no_db.status.success(),
        "structural verify must succeed; stderr={}",
        String::from_utf8_lossy(&verify_no_db.stderr)
    );
    assert!(String::from_utf8_lossy(&verify_no_db.stdout).contains("db cross-check:    skipped"));

    // verify (with db) — structural + db cross-check.
    let verify_db = Command::new(cli_bin())
        .args(["audit", "checkpoint", "verify"])
        .arg(&cp)
        .args(["--db"])
        .arg(&db)
        .output()
        .unwrap();
    assert!(
        verify_db.status.success(),
        "verify --db must succeed; stderr={}",
        String::from_utf8_lossy(&verify_db.stderr)
    );
    let stdout = String::from_utf8_lossy(&verify_db.stdout);
    assert!(stdout.contains("db cross-check:    ok"));
    assert!(stdout.contains("verify result:     ok"));
}

#[test]
fn verify_rejects_tampered_checkpoint() {
    // Tamper the persisted checkpoint JSON's `chain_digest` and
    // confirm that --db verify catches it. The DB row still has the
    // correct digest, so the cross-check fires.
    let tmp = tempfile::tempdir().unwrap();
    let db = seed_db_with_audit_chain(tmp.path(), "tamper");
    let cp = tmp.path().join("cp.json");

    let create = Command::new(cli_bin())
        .args(["audit", "checkpoint", "create", "--db"])
        .arg(&db)
        .args(["--out"])
        .arg(&cp)
        .output()
        .unwrap();
    assert!(create.status.success());

    let raw = std::fs::read_to_string(&cp).unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(&raw).unwrap();
    // Replace one nibble of the chain digest. Still 64 hex chars, so
    // structural checks pass — only the DB cross-check can catch it.
    let original = doc["chain_digest"].as_str().unwrap().to_string();
    let mut bytes: Vec<char> = original.chars().collect();
    bytes[0] = if bytes[0] == 'a' { 'b' } else { 'a' };
    let tampered: String = bytes.into_iter().collect();
    doc["chain_digest"] = serde_json::Value::String(tampered);
    std::fs::write(&cp, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let out = Command::new(cli_bin())
        .args(["audit", "checkpoint", "verify"])
        .arg(&cp)
        .args(["--db"])
        .arg(&db)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "tampered checkpoint must be rejected"
    );
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("chain_digest mismatch"),
        "stderr must explain why; got: {stderr}"
    );
}

#[test]
fn verify_rejects_wrong_db() {
    // Create a checkpoint against DB A, then verify it against DB B
    // (which has its own, different audit chain). The lookup of the
    // mock_anchor_ref in DB B must miss → exit 2.
    let tmp = tempfile::tempdir().unwrap();
    let db_a = seed_db_with_audit_chain(tmp.path(), "alice");
    let db_b = seed_db_with_audit_chain(tmp.path(), "bob");
    let cp = tmp.path().join("cp.json");

    let create = Command::new(cli_bin())
        .args(["audit", "checkpoint", "create", "--db"])
        .arg(&db_a)
        .args(["--out"])
        .arg(&cp)
        .output()
        .unwrap();
    assert!(create.status.success());

    let out = Command::new(cli_bin())
        .args(["audit", "checkpoint", "verify"])
        .arg(&cp)
        .args(["--db"])
        .arg(&db_b)
        .output()
        .unwrap();
    assert!(!out.status.success(), "wrong-DB verify must fail");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not issued by this DB") || stderr.contains("no row"),
        "stderr must explain wrong-DB; got: {stderr}"
    );
}

#[test]
fn create_against_empty_audit_db_returns_exit_three() {
    // A fresh DB with V001..V007 applied but no audit events. The
    // CLI must NOT pretend ok — exit 3 and an honest message.
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("empty.db");
    // Prime the DB with all migrations via any read-only-ish CLI
    // call. `key list --mock --db` runs all migrations and exits 0
    // without touching audit_events.
    let probe = Command::new(cli_bin())
        .args(["key", "list", "--mock", "--db"])
        .arg(&db)
        .output()
        .unwrap();
    assert!(probe.status.success());
    assert!(db.exists());

    let out = Command::new(cli_bin())
        .args(["audit", "checkpoint", "create", "--db"])
        .arg(&db)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "create on empty audit chain must not succeed"
    );
    assert_eq!(
        out.status.code(),
        Some(3),
        "exit 3 = honest 'nothing to anchor yet'"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("audit chain is empty"),
        "stderr must surface the empty-chain reason; got: {stderr}"
    );
}

#[test]
fn verify_rejects_bad_schema_id() {
    // Defensive: a JSON file with the wrong schema id must be
    // rejected even before the --db cross-check runs.
    let tmp = tempfile::tempdir().unwrap();
    let cp = tmp.path().join("bad.json");
    let bad = serde_json::json!({
        "schema": "not-a-checkpoint-schema",
        "mock_anchor": true,
        "explanation": "x",
        "sequence": 1u64,
        "latest_event_id": "evt-x",
        "latest_event_hash": "0".repeat(64),
        "chain_digest": "0".repeat(64),
        "mock_anchor_ref": "local-mock-anchor-deadbeefdeadbeef",
        "created_at": "2026-04-28T10:00:00Z",
    });
    std::fs::write(&cp, serde_json::to_string_pretty(&bad).unwrap()).unwrap();
    let out = Command::new(cli_bin())
        .args(["audit", "checkpoint", "verify"])
        .arg(&cp)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("bad schema"),
        "stderr must call out the schema mismatch; got: {stderr}"
    );
}

#[test]
fn verify_rejects_mock_anchor_false() {
    // The schema id is right but `mock_anchor: false` would be a
    // production-readiness overclaim. The CLI refuses.
    let tmp = tempfile::tempdir().unwrap();
    let cp = tmp.path().join("not-mock.json");
    let bad = serde_json::json!({
        "schema": "sbo3l.audit_checkpoint.v1",
        "mock_anchor": false,
        "explanation": "x",
        "sequence": 1u64,
        "latest_event_id": "evt-x",
        "latest_event_hash": "0".repeat(64),
        "chain_digest": "0".repeat(64),
        "mock_anchor_ref": "local-mock-anchor-deadbeefdeadbeef",
        "created_at": "2026-04-28T10:00:00Z",
    });
    std::fs::write(&cp, serde_json::to_string_pretty(&bad).unwrap()).unwrap();
    let out = Command::new(cli_bin())
        .args(["audit", "checkpoint", "verify"])
        .arg(&cp)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("mock_anchor must be true"),
        "stderr must refuse non-mock claim; got: {stderr}"
    );
}
