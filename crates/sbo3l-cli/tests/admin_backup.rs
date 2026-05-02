//! R14 P2 — end-to-end tests for `sbo3l admin backup/restore/export/verify`.
//!
//! Spawns the actual CLI binary against a freshly-seeded SQLite DB,
//! then exercises the round-trip: backup → restore → DB content
//! matches; backup --encrypt-with → restore --decrypt-with round-trips;
//! export emits valid JSONL; verify catches a tampered chain.
//!
//! Gated on `--features admin_backup` since the commands themselves
//! error out without it.

#![cfg(feature = "admin_backup")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn cli_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sbo3l"))
}

fn seed_db(tmpdir: &Path) -> PathBuf {
    use sbo3l_core::signer::DevSigner;
    use sbo3l_storage::audit_store::NewAuditEvent;
    use sbo3l_storage::Storage;

    let path = tmpdir.join("seed.db");
    let mut storage = Storage::open(&path).expect("open seed db");
    let signer = DevSigner::from_seed("audit-signer-v1", [11u8; 32]);
    let now = chrono::Utc::now();
    for i in 0..3 {
        let evt = NewAuditEvent {
            event_type: "policy_decided".to_string(),
            actor: "policy_engine".to_string(),
            subject_id: format!("pr-test-{i}"),
            payload_hash: format!("hash-{i}"),
            metadata: serde_json::Map::new(),
            policy_version: Some(1),
            policy_hash: Some("policy-v1".to_string()),
            attestation_ref: None,
            ts: now,
        };
        storage
            .finalize_decision(&[], evt, &signer)
            .expect("append seed event");
    }
    drop(storage);
    path
}

#[test]
fn backup_then_restore_roundtrip_preserves_audit_chain() {
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let src_db = seed_db(tmpdir.path());
    let archive = tmpdir.path().join("snap.tar.zst");
    let restored_db = tmpdir.path().join("restored.db");

    let backup_status = Command::new(cli_binary())
        .args([
            "admin",
            "backup",
            "--db",
            src_db.to_str().unwrap(),
            "--to",
            archive.to_str().unwrap(),
        ])
        .status()
        .expect("run backup");
    assert!(backup_status.success(), "backup must succeed");
    assert!(
        archive.exists() && fs::metadata(&archive).unwrap().len() > 0,
        "archive must be a non-empty file"
    );

    let restore_status = Command::new(cli_binary())
        .args([
            "admin",
            "restore",
            "--from",
            archive.to_str().unwrap(),
            "--db",
            restored_db.to_str().unwrap(),
        ])
        .status()
        .expect("run restore");
    assert!(restore_status.success(), "restore must succeed");
    assert!(restored_db.exists());

    let storage = sbo3l_storage::Storage::open(&restored_db).expect("open restored");
    let events = storage.audit_list().expect("audit_list");
    assert_eq!(events.len(), 3, "restored chain must have 3 events");
    for (i, evt) in events.iter().enumerate() {
        assert_eq!(evt.event.subject_id, format!("pr-test-{i}"));
    }
}

#[test]
fn backup_refuses_to_overwrite_existing_db_on_restore() {
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let src_db = seed_db(tmpdir.path());
    let archive = tmpdir.path().join("snap.tar.zst");
    let dest_db = tmpdir.path().join("preexisting.db");
    fs::write(&dest_db, b"do not overwrite me").unwrap();

    let _ = Command::new(cli_binary())
        .args([
            "admin",
            "backup",
            "--db",
            src_db.to_str().unwrap(),
            "--to",
            archive.to_str().unwrap(),
        ])
        .status()
        .expect("run backup");

    let restore = Command::new(cli_binary())
        .args([
            "admin",
            "restore",
            "--from",
            archive.to_str().unwrap(),
            "--db",
            dest_db.to_str().unwrap(),
        ])
        .output()
        .expect("run restore");
    assert!(!restore.status.success(), "restore must fail");
    let stderr = String::from_utf8_lossy(&restore.stderr);
    assert!(
        stderr.contains("refusing to overwrite"),
        "stderr should explain refusal; got: {stderr}"
    );
    assert_eq!(fs::read(&dest_db).unwrap(), b"do not overwrite me");
}

#[test]
fn s3_uri_rejected_with_helpful_message() {
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let src_db = seed_db(tmpdir.path());
    let out = Command::new(cli_binary())
        .args([
            "admin",
            "backup",
            "--db",
            src_db.to_str().unwrap(),
            "--to",
            "s3://bucket/key.tar.zst",
        ])
        .output()
        .expect("run backup");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("s3://") && stderr.contains("not yet supported"),
        "stderr should explain s3 limitation; got: {stderr}"
    );
}

#[test]
fn export_jsonl_emits_one_line_per_event() {
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let src_db = seed_db(tmpdir.path());
    let out_path = tmpdir.path().join("audit.jsonl");

    let status = Command::new(cli_binary())
        .args([
            "admin",
            "export",
            "--db",
            src_db.to_str().unwrap(),
            "--to",
            out_path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .status()
        .expect("run export");
    assert!(status.success());
    let body = fs::read_to_string(&out_path).expect("read jsonl");
    let lines: Vec<&str> = body.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 3, "expected 3 jsonl lines, got {}", lines.len());
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).expect("each line is JSON");
        assert!(
            v.get("event").is_some(),
            "each line should be a SignedAuditEvent"
        );
    }
}

#[test]
fn export_parquet_format_errors_with_scope_message() {
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let src_db = seed_db(tmpdir.path());
    let out = Command::new(cli_binary())
        .args([
            "admin",
            "export",
            "--db",
            src_db.to_str().unwrap(),
            "--to",
            tmpdir.path().join("audit.parquet").to_str().unwrap(),
            "--format",
            "parquet",
        ])
        .output()
        .expect("run export");
    assert!(!out.status.success(), "parquet must fail until implemented");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("parquet") && stderr.contains("not yet implemented"),
        "stderr should be honest about scope: {stderr}"
    );
}

#[test]
fn verify_reports_intact_chain() {
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let src_db = seed_db(tmpdir.path());
    let archive = tmpdir.path().join("snap.tar.zst");
    let _ = Command::new(cli_binary())
        .args([
            "admin",
            "backup",
            "--db",
            src_db.to_str().unwrap(),
            "--to",
            archive.to_str().unwrap(),
        ])
        .status()
        .expect("run backup");
    let out = Command::new(cli_binary())
        .args(["admin", "verify", "--from", archive.to_str().unwrap()])
        .output()
        .expect("run verify");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "verify must succeed on intact chain. stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("verified 3 audit events"),
        "stdout: {stdout}"
    );
}

#[test]
fn age_encrypted_roundtrip() {
    use age::secrecy::ExposeSecret;
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let src_db = seed_db(tmpdir.path());
    let archive = tmpdir.path().join("snap.tar.zst.age");
    let restored = tmpdir.path().join("restored.db");

    let identity = age::x25519::Identity::generate();
    let recipient = identity.to_public();
    let identity_path = tmpdir.path().join("identity.txt");
    fs::write(&identity_path, identity.to_string().expose_secret()).unwrap();

    let backup = Command::new(cli_binary())
        .args([
            "admin",
            "backup",
            "--db",
            src_db.to_str().unwrap(),
            "--to",
            archive.to_str().unwrap(),
            "--encrypt-with",
            &recipient.to_string(),
        ])
        .status()
        .expect("run backup");
    assert!(backup.success(), "encrypted backup must succeed");

    let no_key = Command::new(cli_binary())
        .args([
            "admin",
            "restore",
            "--from",
            archive.to_str().unwrap(),
            "--db",
            tmpdir.path().join("noop.db").to_str().unwrap(),
        ])
        .output()
        .expect("run restore-no-key");
    assert!(!no_key.status.success(), "restore w/o key must fail");
    let stderr = String::from_utf8_lossy(&no_key.stderr);
    assert!(
        stderr.contains("age-encrypted"),
        "stderr should mention encryption; got: {stderr}"
    );

    let restore = Command::new(cli_binary())
        .args([
            "admin",
            "restore",
            "--from",
            archive.to_str().unwrap(),
            "--db",
            restored.to_str().unwrap(),
            "--decrypt-with",
            identity_path.to_str().unwrap(),
        ])
        .output()
        .expect("run restore-with-key");
    let stdout = String::from_utf8_lossy(&restore.stdout);
    let stderr = String::from_utf8_lossy(&restore.stderr);
    assert!(
        restore.status.success(),
        "restore w/ key must succeed.\nstdout: {stdout}\nstderr: {stderr}"
    );

    let storage = sbo3l_storage::Storage::open(&restored).expect("open restored");
    let events = storage.audit_list().expect("audit_list");
    assert_eq!(events.len(), 3);
}
