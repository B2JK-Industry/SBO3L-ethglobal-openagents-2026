//! Integration smoke test for `mandate audit export` → `mandate audit verify-bundle`.
//!
//! Exercises the real CLI binary end-to-end: deterministic dev signers
//! produce a receipt and an audit chain, the export command writes a
//! bundle JSON file, and the verify-bundle command must reject any
//! tampering with the bundle's cryptographic claims.

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use mandate_core::audit::{AuditEvent, SignedAuditEvent, ZERO_HASH};
use mandate_core::receipt::{Decision, UnsignedReceipt};
use mandate_core::signer::DevSigner;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mandate"))
}

fn write_chain_jsonl(path: &std::path::Path, events: &[SignedAuditEvent]) {
    let mut f = std::fs::File::create(path).unwrap();
    for e in events {
        let line = serde_json::to_string(e).unwrap();
        writeln!(f, "{line}").unwrap();
    }
}

/// Build a small but realistic chain (3 events) plus a receipt that
/// references the middle event, signed by deterministic dev signers so
/// the bundle is reproducible across test runs.
fn build_fixture_files() -> (
    tempfile::TempDir,
    PathBuf, // receipt path
    PathBuf, // chain JSONL path
    String,  // receipt pubkey hex
    String,  // audit pubkey hex
    String,  // audit event id (the receipt's referent — used by negative tests)
) {
    let tmp = tempfile::tempdir().unwrap();

    let audit_signer = DevSigner::from_seed("audit-signer-v1", [11u8; 32]);
    let receipt_signer = DevSigner::from_seed("decision-signer-v1", [7u8; 32]);

    let mk = |seq: u64, prev: &str, ts: &str, id: &str, ty: &str| {
        let event = AuditEvent {
            version: 1,
            seq,
            id: id.to_string(),
            ts: chrono::DateTime::parse_from_rfc3339(ts).unwrap().into(),
            event_type: ty.to_string(),
            actor: "policy_engine".to_string(),
            subject_id: format!("pr-test-{seq:03}"),
            payload_hash: ZERO_HASH.to_string(),
            metadata: serde_json::Map::new(),
            policy_version: Some(1),
            policy_hash: None,
            attestation_ref: None,
            prev_event_hash: prev.to_string(),
        };
        SignedAuditEvent::sign(event, &audit_signer).unwrap()
    };
    let e1 = mk(
        1,
        ZERO_HASH,
        "2026-04-27T12:00:00Z",
        "evt-01HTAWX5K3R8YV9NQB7C6P2DGQ",
        "runtime_started",
    );
    let e2 = mk(
        2,
        &e1.event_hash,
        "2026-04-27T12:00:01Z",
        "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
        "policy_decided",
    );
    let e3 = mk(
        3,
        &e2.event_hash,
        "2026-04-27T12:00:02Z",
        "evt-01HTAWX5K3R8YV9NQB7C6P2DGS",
        "policy_decided",
    );

    let chain_path = tmp.path().join("chain.jsonl");
    write_chain_jsonl(&chain_path, &[e1.clone(), e2.clone(), e3.clone()]);

    let unsigned = UnsignedReceipt {
        agent_id: "research-agent-01".to_string(),
        decision: Decision::Allow,
        deny_code: None,
        request_hash: "1111111111111111111111111111111111111111111111111111111111111111"
            .to_string(),
        policy_hash: "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        policy_version: Some(1),
        audit_event_id: e2.event.id.clone(),
        execution_ref: None,
        issued_at: chrono::DateTime::parse_from_rfc3339("2026-04-27T12:00:01.500Z")
            .unwrap()
            .into(),
        expires_at: None,
    };
    let receipt = unsigned.sign(&receipt_signer).unwrap();
    let receipt_path = tmp.path().join("receipt.json");
    std::fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();

    let receipt_pk = receipt_signer.verifying_key_hex();
    let audit_pk = audit_signer.verifying_key_hex();
    let target_id = e2.event.id;
    (
        tmp,
        receipt_path,
        chain_path,
        receipt_pk,
        audit_pk,
        target_id,
    )
}

#[test]
fn export_then_verify_bundle_succeeds() {
    let (tmp, receipt, chain, rpk, apk, target_id) = build_fixture_files();
    let bundle_path = tmp.path().join("bundle.json");

    let out = Command::new(cli_bin())
        .args([
            "audit",
            "export",
            "--receipt",
            receipt.to_str().unwrap(),
            "--chain",
            chain.to_str().unwrap(),
            "--receipt-pubkey",
            &rpk,
            "--audit-pubkey",
            &apk,
            "--out",
            bundle_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "export must succeed; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(bundle_path.exists());

    let out = Command::new(cli_bin())
        .args([
            "audit",
            "verify-bundle",
            "--path",
            bundle_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "verify-bundle must succeed; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ok: bundle verified"),
        "unexpected verify stdout: {stdout}"
    );
    assert!(
        stdout.contains(&target_id),
        "verify summary must name the audit_event_id; stdout={stdout}"
    );
}

#[test]
fn verify_bundle_rejects_tampered_receipt_signature() {
    // Tamper with the bundle's receipt signature bytes after export. The
    // verify command must exit non-zero; the daemon-style signature check
    // protects every receipt-covered field (request_hash, policy_hash,
    // decision, etc.) by extension.
    let (tmp, receipt, chain, rpk, apk, _) = build_fixture_files();
    let bundle_path = tmp.path().join("bundle.json");
    let status = Command::new(cli_bin())
        .args([
            "audit",
            "export",
            "--receipt",
            receipt.to_str().unwrap(),
            "--chain",
            chain.to_str().unwrap(),
            "--receipt-pubkey",
            &rpk,
            "--audit-pubkey",
            &apk,
            "--out",
            bundle_path.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let raw = std::fs::read_to_string(&bundle_path).unwrap();
    let mut bundle: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let sig = bundle["receipt"]["signature"]["signature_hex"]
        .as_str()
        .unwrap()
        .to_string();
    let mut chars: Vec<char> = sig.chars().collect();
    let last = chars.pop().unwrap();
    chars.push(if last == '0' { '1' } else { '0' });
    bundle["receipt"]["signature"]["signature_hex"] =
        serde_json::Value::String(chars.into_iter().collect());
    std::fs::write(&bundle_path, serde_json::to_vec_pretty(&bundle).unwrap()).unwrap();

    let out = Command::new(cli_bin())
        .args([
            "audit",
            "verify-bundle",
            "--path",
            bundle_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "verify-bundle must reject tampered signature"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("bundle invalid"),
        "expected diagnostic on stderr; got {stderr}"
    );
}

#[test]
fn verify_bundle_rejects_broken_chain_linkage() {
    // Mutate prev_event_hash on an event after export — chain verification
    // must catch the broken linkage even though signatures are still
    // present.
    let (tmp, receipt, chain, rpk, apk, _) = build_fixture_files();
    let bundle_path = tmp.path().join("bundle.json");
    let status = Command::new(cli_bin())
        .args([
            "audit",
            "export",
            "--receipt",
            receipt.to_str().unwrap(),
            "--chain",
            chain.to_str().unwrap(),
            "--receipt-pubkey",
            &rpk,
            "--audit-pubkey",
            &apk,
            "--out",
            bundle_path.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let raw = std::fs::read_to_string(&bundle_path).unwrap();
    let mut bundle: serde_json::Value = serde_json::from_str(&raw).unwrap();
    bundle["audit_chain_segment"][2]["event"]["prev_event_hash"] = serde_json::Value::String(
        "0000000000000000000000000000000000000000000000000000000000000001".to_string(),
    );
    std::fs::write(&bundle_path, serde_json::to_vec_pretty(&bundle).unwrap()).unwrap();

    let out = Command::new(cli_bin())
        .args([
            "audit",
            "verify-bundle",
            "--path",
            bundle_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "verify-bundle must reject broken linkage"
    );
}

// ----- DB-backed export ---------------------------------------------------
//
// The DB-backed export reads the audit chain directly from a Mandate
// daemon's SQLite store via `mandate_storage::Storage`. The fixture below
// builds a real on-disk DB by appending three events with the same
// deterministic dev signers used by `build_fixture_files`, then writes a
// signed receipt that points at the middle event. This is the realistic
// shape: a long-running daemon's storage + a receipt the agent kept.

use mandate_storage::audit_store::NewAuditEvent;
use mandate_storage::Storage;

fn build_db_fixture() -> (
    tempfile::TempDir,
    PathBuf, // db path
    PathBuf, // receipt path
    String,  // receipt pubkey hex
    String,  // audit pubkey hex
    String,  // audit_event_id the receipt references
) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("mandate.sqlite");
    let audit_signer = DevSigner::from_seed("audit-signer-v1", [11u8; 32]);
    let receipt_signer = DevSigner::from_seed("decision-signer-v1", [7u8; 32]);

    let middle_event_id;
    {
        let mut storage = Storage::open(&db_path).unwrap();
        storage
            .audit_append(
                NewAuditEvent::now("runtime_started", "mandate-server", "runtime"),
                &audit_signer,
            )
            .unwrap();
        let middle = storage
            .audit_append(
                NewAuditEvent::now("policy_decided", "policy_engine", "pr-test-001"),
                &audit_signer,
            )
            .unwrap();
        storage
            .audit_append(
                NewAuditEvent::now("policy_decided", "policy_engine", "pr-test-002"),
                &audit_signer,
            )
            .unwrap();
        middle_event_id = middle.event.id.clone();
    } // Drop the Storage handle so the CLI can open its own connection.

    let unsigned = UnsignedReceipt {
        agent_id: "research-agent-01".to_string(),
        decision: Decision::Allow,
        deny_code: None,
        request_hash: "1111111111111111111111111111111111111111111111111111111111111111"
            .to_string(),
        policy_hash: "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        policy_version: Some(1),
        audit_event_id: middle_event_id.clone(),
        execution_ref: None,
        issued_at: chrono::DateTime::parse_from_rfc3339("2026-04-27T12:00:01.500Z")
            .unwrap()
            .into(),
        expires_at: None,
    };
    let receipt = unsigned.sign(&receipt_signer).unwrap();
    let receipt_path = tmp.path().join("receipt.json");
    std::fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();

    (
        tmp,
        db_path,
        receipt_path,
        receipt_signer.verifying_key_hex(),
        audit_signer.verifying_key_hex(),
        middle_event_id,
    )
}

#[test]
fn db_backed_export_then_verify_bundle_succeeds() {
    // Happy path: a real on-disk daemon DB + a signed receipt → export
    // assembles a bundle without ever touching a JSONL file → verify
    // round-trips clean. This is the "Mandate leaves behind verifiable
    // proof directly from its daemon storage" claim end-to-end.
    let (tmp, db, receipt, rpk, apk, target_id) = build_db_fixture();
    let bundle_path = tmp.path().join("bundle.json");

    let out = Command::new(cli_bin())
        .args([
            "audit",
            "export",
            "--receipt",
            receipt.to_str().unwrap(),
            "--db",
            db.to_str().unwrap(),
            "--receipt-pubkey",
            &rpk,
            "--audit-pubkey",
            &apk,
            "--out",
            bundle_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "db-backed export must succeed; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(bundle_path.exists());

    let out = Command::new(cli_bin())
        .args([
            "audit",
            "verify-bundle",
            "--path",
            bundle_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "verify-bundle must succeed; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ok: bundle verified"),
        "unexpected stdout: {stdout}"
    );
    assert!(
        stdout.contains(&target_id),
        "verify summary must name the audit_event_id; stdout={stdout}"
    );

    // Bundle's chain segment must be exactly genesis through the receipt's
    // referenced event (length 2), not the entire DB chain (length 3).
    let bundle: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&bundle_path).unwrap()).unwrap();
    let segment = bundle["audit_chain_segment"].as_array().unwrap();
    assert_eq!(segment.len(), 2, "DB-backed export must slice prefix");
}

#[test]
fn db_backed_export_fails_when_db_path_missing() {
    let (tmp, _db, receipt, rpk, apk, _) = build_db_fixture();
    let bundle_path = tmp.path().join("bundle.json");
    let nonexistent = tmp.path().join("does-not-exist.sqlite");

    let out = Command::new(cli_bin())
        .args([
            "audit",
            "export",
            "--receipt",
            receipt.to_str().unwrap(),
            "--db",
            nonexistent.to_str().unwrap(),
            "--receipt-pubkey",
            &rpk,
            "--audit-pubkey",
            &apk,
            "--out",
            bundle_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "must reject missing db path; stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("does not exist"),
        "expected diagnostic naming the missing db; got {stderr}"
    );
    assert!(
        !bundle_path.exists(),
        "no bundle file should be written when export fails"
    );
}

#[test]
fn db_backed_export_fails_when_audit_event_id_not_in_db() {
    // Receipt points at an audit_event_id the DB never wrote (typical
    // cause: receipt and DB belong to different daemons). The error
    // message must echo the missing id verbatim so the operator can
    // diagnose it without re-reading the receipt.
    let (tmp, db, _good_receipt, rpk, apk, _) = build_db_fixture();
    let bundle_path = tmp.path().join("bundle.json");

    // Build a fresh receipt that points at a bogus audit_event_id.
    let receipt_signer = DevSigner::from_seed("decision-signer-v1", [7u8; 32]);
    let bogus_id = "evt-DOES-NOT-EXIST-IN-DB-XX".to_string();
    let unsigned = UnsignedReceipt {
        agent_id: "research-agent-01".to_string(),
        decision: Decision::Allow,
        deny_code: None,
        request_hash: "1111111111111111111111111111111111111111111111111111111111111111"
            .to_string(),
        policy_hash: "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        policy_version: Some(1),
        audit_event_id: bogus_id.clone(),
        execution_ref: None,
        issued_at: chrono::DateTime::parse_from_rfc3339("2026-04-27T12:00:01.500Z")
            .unwrap()
            .into(),
        expires_at: None,
    };
    let receipt = unsigned.sign(&receipt_signer).unwrap();
    let receipt_path = tmp.path().join("bad-receipt.json");
    std::fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();

    let out = Command::new(cli_bin())
        .args([
            "audit",
            "export",
            "--receipt",
            receipt_path.to_str().unwrap(),
            "--db",
            db.to_str().unwrap(),
            "--receipt-pubkey",
            &rpk,
            "--audit-pubkey",
            &apk,
            "--out",
            bundle_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "must reject receipt pointing at unknown audit_event_id"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&bogus_id),
        "diagnostic must echo the missing id; got {stderr}"
    );
    assert!(!bundle_path.exists(), "no bundle on failure");
}

#[test]
fn db_backed_export_fails_when_db_chain_is_tampered() {
    // Manually rewrite a row in the SQLite audit_events table to break
    // the hash chain. The export's pre-flight `verify_chain` must catch
    // this BEFORE writing any bundle, so a downstream consumer never
    // sees an unverifiable bundle from a corrupted DB.
    let (tmp, db, receipt, rpk, apk, _) = build_db_fixture();
    let bundle_path = tmp.path().join("bundle.json");

    // Mutate seq=2's actor — this changes its canonical hash, so the
    // recorded event_hash and the chain's prev_event_hash linkage no
    // longer match what `verify_chain` recomputes.
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "UPDATE audit_events SET actor = 'tampered-actor' WHERE seq = 2",
        [],
    )
    .unwrap();
    drop(conn);

    let out = Command::new(cli_bin())
        .args([
            "audit",
            "export",
            "--receipt",
            receipt.to_str().unwrap(),
            "--db",
            db.to_str().unwrap(),
            "--receipt-pubkey",
            &rpk,
            "--audit-pubkey",
            &apk,
            "--out",
            bundle_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "must reject tampered DB chain at export time"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("audit chain pre-flight failed"),
        "expected pre-flight diagnostic; got {stderr}"
    );
    assert!(
        !bundle_path.exists(),
        "no bundle should be written when the DB chain doesn't verify"
    );
}
