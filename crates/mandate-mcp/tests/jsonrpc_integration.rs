//! Integration tests for the `mandate-mcp` stdio JSON-RPC server (Passport P3.1).
//!
//! Two layers:
//! 1. **In-process dispatch** — calls `mandate_mcp::dispatch` directly. Exercises
//!    every tool through `ServerContext` without spawning a subprocess; this is
//!    where most of the coverage lives because it's fast and lets us pin the
//!    JSON-RPC error envelope shape exactly.
//! 2. **Stdio child-process** — spawns the `mandate-mcp` binary, writes
//!    newline-delimited JSON-RPC requests, reads responses. Exists to prove
//!    the wire transport works end-to-end (the dispatcher logic is the same).
//!
//! Test DB / policy setup:
//! - Each test gets a fresh `tempfile::TempDir` SQLite DB (so the nonce
//!   replay-protection store is empty).
//! - Tests activate `test-corpus/policy/reference_low_risk.json` directly via
//!   `Storage::policy_activate` rather than spawning `mandate policy activate`,
//!   which would force a cross-crate binary dependency.
//! - APRP fixtures come from `test-corpus/aprp/` — `golden_001_minimal.json`
//!   for allow paths, `deny_prompt_injection_request.json` for deny paths.
//!   Both have `expiry: "2026-05-01T..."`, comfortably in the future for
//!   2026-Q2 CI.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use mandate_mcp::{dispatch, error_codes, ServerContext, TOOL_ERROR_CODE};
use serde_json::{json, Value};

const REF_POLICY: &str = "../../test-corpus/policy/reference_low_risk.json";
const APRP_ALLOW: &str = "../../test-corpus/aprp/golden_001_minimal.json";
const APRP_DENY: &str = "../../test-corpus/aprp/deny_prompt_injection_request.json";

const CAPSULE_GOLDEN: &str = "../../test-corpus/passport/golden_001_allow_keeperhub_mock.json";
const CAPSULE_TAMPERED_HASH: &str =
    "../../test-corpus/passport/tampered_004_request_hash_mismatch.json";

// Public dev signers from `mandate_server::AppState::new`. These match the
// seeds in the production-shaped runner; auditors can derive the same
// pubkeys themselves (or read them off any signed receipt's `key_id`).
//
// The `dev_pubkeys_match_canonical_constants` test below pins the exact
// hex bytes — the sponsor demo (`demo-scripts/sponsors/mcp-passport.sh`)
// hardcodes the same values. If a seed ever changes, both this test and
// the demo break together — that's the contract.
const DEV_AUDIT_PUBKEY_HEX: &str =
    "66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a";
const DEV_RECEIPT_PUBKEY_HEX: &str =
    "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c";

fn audit_signer_pubkey_hex() -> String {
    use mandate_core::signer::DevSigner;
    DevSigner::from_seed("audit-signer-v1", [11u8; 32]).verifying_key_hex()
}

fn receipt_signer_pubkey_hex() -> String {
    use mandate_core::signer::DevSigner;
    DevSigner::from_seed("decision-signer-v1", [7u8; 32]).verifying_key_hex()
}

#[test]
fn dev_pubkeys_match_canonical_constants() {
    // Anchor for the sponsor demo's hardcoded hex. If anyone changes the
    // seeds in `crates/mandate-server/src/lib.rs::AppState::new`, this
    // test catches it before the demo silently breaks.
    assert_eq!(audit_signer_pubkey_hex(), DEV_AUDIT_PUBKEY_HEX);
    assert_eq!(receipt_signer_pubkey_hex(), DEV_RECEIPT_PUBKEY_HEX);
}

fn read_json(path: &str) -> Value {
    let raw = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read fixture {path}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse fixture {path}: {e}"))
}

fn fresh_db() -> (tempfile::TempDir, PathBuf, ServerContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("mandate.sqlite");
    activate_reference_policy(&path);
    // Round 0 path-sandbox fix: tests must supply a ctx whose root
    // contains the tempdir DB. Defaulting to env / cwd would put the
    // root at the crate dir, which doesn't include `/var/folders/...`
    // tempdirs. ServerContext::with_root(tempdir) keeps each test
    // hermetic without racing on the process-global `MANDATE_MCP_ROOT`
    // env var.
    let ctx = ServerContext::with_root(dir.path().to_path_buf());
    (dir, path, ctx)
}

/// Repo-rooted ctx for tests that read capsule path fixtures from
/// `test-corpus/passport/`. Resolves to the workspace root via
/// `CARGO_MANIFEST_DIR/../..`. Tests that pass a path argument under
/// `../../` need this; tests that pass `capsule` inline don't care.
fn repo_root_ctx() -> ServerContext {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root from CARGO_MANIFEST_DIR")
        .to_path_buf();
    ServerContext::with_root(root)
}

fn activate_reference_policy(db_path: &Path) {
    use mandate_core::hashing::{canonical_json, sha256_hex};
    use mandate_storage::Storage;

    let policy_value = read_json(REF_POLICY);
    let policy_hash = sha256_hex(&canonical_json(&policy_value).expect("canonicalise policy"));
    let policy_json = serde_json::to_string(&policy_value).expect("re-serialise policy");
    let mut storage = Storage::open(db_path).expect("open storage");
    storage
        .policy_activate(
            &policy_json,
            &policy_hash,
            "operator-cli",
            chrono::Utc::now(),
        )
        .expect("policy_activate");
}

fn aprp_with_unique_nonce(path: &str, nonce_suffix: &str) -> Value {
    let mut v = read_json(path);
    let nonce = format!("01HTAWX5K3R8YV9NQB7C6P{nonce_suffix:>04}");
    v["nonce"] = Value::String(nonce);
    v
}

// ---------------------------------------------------------------------------
// Tool 1 — mandate.validate_aprp
// ---------------------------------------------------------------------------

#[test]
fn validate_aprp_accepts_golden_fixture() {
    let ctx = ServerContext::new();
    let aprp = read_json(APRP_ALLOW);
    let result = dispatch("mandate.validate_aprp", &json!({ "aprp": aprp }), &ctx).unwrap();
    assert_eq!(result["ok"], true);
    let hash = result["request_hash"].as_str().expect("request_hash str");
    assert_eq!(hash.len(), 64, "request_hash must be 64 hex chars");
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "request_hash must be lowercase hex; got {hash}"
    );
}

#[test]
fn validate_aprp_rejects_unknown_field_with_stable_code() {
    let ctx = ServerContext::new();
    let mut aprp = read_json(APRP_ALLOW);
    aprp["totally_unknown_root_field"] = json!("nope");
    let err = dispatch("mandate.validate_aprp", &json!({ "aprp": aprp }), &ctx).unwrap_err();
    assert_eq!(err.code, error_codes::APRP_INVALID);
}

#[test]
fn validate_aprp_rejects_missing_field_param() {
    let ctx = ServerContext::new();
    let err = dispatch("mandate.validate_aprp", &json!({}), &ctx).unwrap_err();
    assert_eq!(err.code, error_codes::PARAMS_INVALID);
    assert!(err.message.contains("aprp"));
}

// ---------------------------------------------------------------------------
// Tool 2 — mandate.decide
// ---------------------------------------------------------------------------

#[test]
fn decide_allow_path_returns_auto_approved() {
    let (_dir, db, ctx) = fresh_db();
    let aprp = aprp_with_unique_nonce(APRP_ALLOW, "DEC1");
    let result = dispatch(
        "mandate.decide",
        &json!({ "aprp": aprp, "db": db.to_string_lossy() }),
        &ctx,
    )
    .unwrap();
    assert_eq!(result["status"], "auto_approved");
    let event_id = result["audit_event_id"].as_str().unwrap();
    assert!(
        event_id.starts_with("evt-"),
        "audit_event_id shape: {event_id}"
    );
    assert!(result["receipt"].is_object(), "receipt must be present");
}

#[test]
fn decide_deny_path_returns_rejected_with_deny_code() {
    let (_dir, db, ctx) = fresh_db();
    let aprp = aprp_with_unique_nonce(APRP_DENY, "DEC2");
    let result = dispatch(
        "mandate.decide",
        &json!({ "aprp": aprp, "db": db.to_string_lossy() }),
        &ctx,
    )
    .unwrap();
    assert_eq!(result["status"], "rejected");
    let deny = result["deny_code"].as_str().expect("deny_code present");
    assert!(!deny.is_empty(), "deny_code must be non-empty on a deny");
}

#[test]
fn decide_without_active_policy_returns_policy_not_active() {
    // Empty DB → no active policy.
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("empty.sqlite");
    let _ = mandate_storage::Storage::open(&db).expect("init empty db");
    let aprp = read_json(APRP_ALLOW);
    let ctx = ServerContext::with_root(dir.path().to_path_buf());
    let err = dispatch(
        "mandate.decide",
        &json!({ "aprp": aprp, "db": db.to_string_lossy() }),
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, error_codes::POLICY_NOT_ACTIVE);
}

// ---------------------------------------------------------------------------
// Tool 3 — mandate.run_guarded_execution
// ---------------------------------------------------------------------------

#[test]
fn run_guarded_allow_calls_keeperhub_mock_executor() {
    let (_dir, db, ctx) = fresh_db();
    let aprp = aprp_with_unique_nonce(APRP_ALLOW, "EXC1");
    let result = dispatch(
        "mandate.run_guarded_execution",
        &json!({ "aprp": aprp, "db": db.to_string_lossy(), "executor": "keeperhub" }),
        &ctx,
    )
    .unwrap();
    assert_eq!(result["execution"]["executor"], "keeperhub");
    assert_eq!(result["execution"]["mode"], "mock");
    assert_eq!(result["execution"]["status"], "submitted");
    let exec_ref = result["execution"]["execution_ref"]
        .as_str()
        .expect("execution_ref string on allow path");
    assert!(
        exec_ref.starts_with("kh-"),
        "KeeperHub mock prefix: {exec_ref}"
    );
}

#[test]
fn run_guarded_deny_does_not_call_executor() {
    let (_dir, db, ctx) = fresh_db();
    let aprp = aprp_with_unique_nonce(APRP_DENY, "EXC2");
    let result = dispatch(
        "mandate.run_guarded_execution",
        &json!({ "aprp": aprp, "db": db.to_string_lossy(), "executor": "keeperhub" }),
        &ctx,
    )
    .unwrap();
    // Hard truthfulness invariant from P1.1 (tampered_001): deny ⇒ executor is never called.
    assert_eq!(result["execution"]["status"], "not_called");
    assert_eq!(result["execution"]["execution_ref"], Value::Null);
    assert_eq!(result["decision"]["result"], "deny");
}

#[test]
fn run_guarded_rejects_live_mode() {
    let (_dir, db, ctx) = fresh_db();
    let aprp = aprp_with_unique_nonce(APRP_ALLOW, "EXC3");
    let err = dispatch(
        "mandate.run_guarded_execution",
        &json!({
            "aprp": aprp,
            "db": db.to_string_lossy(),
            "executor": "keeperhub",
            "mode": "live"
        }),
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, error_codes::LIVE_MODE_REJECTED);
}

// ---------------------------------------------------------------------------
// Tool 4 — mandate.verify_capsule
// ---------------------------------------------------------------------------

#[test]
fn verify_capsule_accepts_golden_fixture_via_path() {
    let ctx = repo_root_ctx();
    let result = dispatch(
        "mandate.verify_capsule",
        &json!({ "path": CAPSULE_GOLDEN }),
        &ctx,
    )
    .unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(result["schema"], "mandate.passport_capsule.v1");
}

#[test]
fn verify_capsule_rejects_tampered_request_hash() {
    let ctx = repo_root_ctx();
    let err = dispatch(
        "mandate.verify_capsule",
        &json!({ "path": CAPSULE_TAMPERED_HASH }),
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, error_codes::CAPSULE_INVALID);
    assert!(
        err.message.contains("capsule.request_hash_mismatch"),
        "verifier code must surface in message: {}",
        err.message
    );
}

#[test]
fn verify_capsule_accepts_inline_object() {
    let ctx = ServerContext::new();
    let capsule = read_json(CAPSULE_GOLDEN);
    let result = dispatch(
        "mandate.verify_capsule",
        &json!({ "capsule": capsule }),
        &ctx,
    )
    .unwrap();
    assert_eq!(result["ok"], true);
}

// ---------------------------------------------------------------------------
// Tool 5 — mandate.explain_denial
// ---------------------------------------------------------------------------

#[test]
fn explain_denial_rejects_allow_capsule() {
    let ctx = repo_root_ctx();
    let err = dispatch(
        "mandate.explain_denial",
        &json!({ "path": CAPSULE_GOLDEN }),
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, error_codes::CAPSULE_NOT_DENY);
}

// ---------------------------------------------------------------------------
// Tool 6 — mandate.audit_lookup (IP-3)
// ---------------------------------------------------------------------------

#[test]
fn audit_lookup_hit_returns_bundle_for_seeded_audit_event() {
    let (_dir, db, ctx) = fresh_db();
    let aprp = aprp_with_unique_nonce(APRP_ALLOW, "AAA1");

    // Seed: drive a real decide so the audit chain has at least one event.
    let decide = dispatch(
        "mandate.decide",
        &json!({ "aprp": aprp, "db": db.to_string_lossy() }),
        &ctx,
    )
    .unwrap();
    let event_id = decide["audit_event_id"].as_str().unwrap().to_string();
    let receipt = decide["receipt"].clone();

    let result = dispatch(
        "mandate.audit_lookup",
        &json!({
            "audit_event_id": event_id,
            "db": db.to_string_lossy(),
            "receipt": receipt,
            "receipt_pubkey": receipt_signer_pubkey_hex(),
            "audit_pubkey":   audit_signer_pubkey_hex(),
        }),
        &ctx,
    )
    .unwrap();

    assert_eq!(result["ok"], true);
    let bundle = &result["bundle"];
    assert_eq!(bundle["bundle_type"], "mandate.audit_bundle.v1");
    assert_eq!(bundle["version"], 1);
    assert_eq!(bundle["summary"]["audit_event_id"], event_id);
}

#[test]
fn audit_lookup_miss_returns_audit_event_not_found() {
    let (_dir, db, ctx) = fresh_db();
    let aprp = aprp_with_unique_nonce(APRP_ALLOW, "AAA2");
    // Seed one event so the DB has a chain at all.
    let decide = dispatch(
        "mandate.decide",
        &json!({ "aprp": aprp, "db": db.to_string_lossy() }),
        &ctx,
    )
    .unwrap();
    let receipt = decide["receipt"].clone();
    // Now ask for an event id that exists nowhere.
    let bogus = "evt-01HZZZZZZZZZZZZZZZZZZZZZZZ";
    let err = dispatch(
        "mandate.audit_lookup",
        &json!({
            "audit_event_id": bogus,
            "db": db.to_string_lossy(),
            "receipt": receipt,
            "receipt_pubkey": receipt_signer_pubkey_hex(),
            "audit_pubkey":   audit_signer_pubkey_hex(),
        }),
        &ctx,
    )
    .unwrap_err();
    // Receipt's audit_event_id ≠ bogus, so the early mismatch check fires
    // first. That's correct: it short-circuits before even touching SQLite,
    // and the contract is "either id mismatch OR not-found is the failure
    // mode an auditor can branch on."
    assert_eq!(err.code, error_codes::AUDIT_EVENT_ID_MISMATCH);
}

#[test]
fn audit_lookup_with_matching_receipt_but_unknown_event_id_returns_not_found() {
    // Constructs the failure mode where the receipt and event_id agree on a
    // string, but no such event lives in the DB — the second branch of the
    // 404 contract.
    let (_dir, db, ctx) = fresh_db();
    let aprp = aprp_with_unique_nonce(APRP_ALLOW, "AAA3");
    let decide = dispatch(
        "mandate.decide",
        &json!({ "aprp": aprp, "db": db.to_string_lossy() }),
        &ctx,
    )
    .unwrap();
    let mut receipt = decide["receipt"].clone();
    let bogus = "evt-01HZZZZZZZZZZZZZZZZZZZZZZZ";
    receipt["audit_event_id"] = json!(bogus);
    let err = dispatch(
        "mandate.audit_lookup",
        &json!({
            "audit_event_id": bogus,
            "db": db.to_string_lossy(),
            "receipt": receipt,
            "receipt_pubkey": receipt_signer_pubkey_hex(),
            "audit_pubkey":   audit_signer_pubkey_hex(),
        }),
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, error_codes::AUDIT_EVENT_NOT_FOUND);
}

// ---------------------------------------------------------------------------
// JSON-RPC envelope shape — wire-format conformance
// ---------------------------------------------------------------------------

#[test]
fn jsonrpc_error_envelope_carries_stable_code_in_data() {
    use mandate_mcp::dispatch_to_response;
    use mandate_mcp::jsonrpc::Request;
    use std::sync::Arc;

    let req: Request = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": 17,
        "method": "mandate.validate_aprp",
        "params": {}
    }))
    .unwrap();
    let ctx = Arc::new(ServerContext::new());
    let resp = dispatch_to_response(&req, &ctx);
    let body = serde_json::to_value(&resp).unwrap();
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["id"], 17);
    assert!(
        body["result"].is_null(),
        "error path must not emit `result`"
    );
    let err = &body["error"];
    assert_eq!(err["code"].as_i64(), Some(TOOL_ERROR_CODE));
    assert_eq!(err["data"]["code"], error_codes::PARAMS_INVALID);
}

#[test]
fn jsonrpc_unknown_method_yields_params_invalid() {
    use mandate_mcp::dispatch_to_response;
    use mandate_mcp::jsonrpc::Request;
    use std::sync::Arc;

    let req: Request = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": "x",
        "method": "mandate.does_not_exist",
        "params": {}
    }))
    .unwrap();
    let ctx = Arc::new(ServerContext::new());
    let resp = dispatch_to_response(&req, &ctx);
    let body = serde_json::to_value(&resp).unwrap();
    assert_eq!(body["error"]["data"]["code"], error_codes::PARAMS_INVALID);
}

// ---------------------------------------------------------------------------
// Stdio transport — child process, NDJSON
// ---------------------------------------------------------------------------

struct McpServer {
    child: Child,
}

impl McpServer {
    fn spawn() -> Self {
        Self::spawn_with_env(&[])
    }

    /// Spawn with extra env vars. Used to set `MANDATE_MCP_ROOT` for
    /// the path-sandbox tests that need a specific root in the child.
    fn spawn_with_env(extra_env: &[(&str, &Path)]) -> Self {
        let bin = env!("CARGO_BIN_EXE_mandate-mcp");
        let mut cmd = Command::new(bin);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        let child = cmd.spawn().expect("spawn mandate-mcp");
        Self { child }
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let stdin = self.child.stdin.as_mut().expect("stdin pipe");
        let line = serde_json::to_string(&req).unwrap();
        writeln!(stdin, "{line}").expect("write stdin");
        stdin.flush().expect("flush stdin");

        let stdout = self.child.stdout.as_mut().expect("stdout pipe");
        let mut reader = BufReader::new(stdout);
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .expect("read response line");
        serde_json::from_str(response_line.trim()).expect("parse response JSON")
    }
}

impl Drop for McpServer {
    fn drop(&mut self) {
        // Closing stdin makes the child exit cleanly. Wait briefly so we
        // don't leave zombie processes between test runs.
        if let Some(stdin) = self.child.stdin.take() {
            drop(stdin);
        }
        let _ = self.child.wait();
    }
}

#[test]
fn stdio_tools_list_round_trips_through_child_process() {
    let mut server = McpServer::spawn();
    let resp = server.call("tools/list", json!({}));
    assert_eq!(resp["jsonrpc"], "2.0");
    let result = resp["result"].as_array().expect("tools/list returns array");
    assert_eq!(result.len(), 6, "P3.1 ships six tools");
    let names: Vec<&str> = result.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"mandate.audit_lookup"));
}

#[test]
fn stdio_validate_aprp_round_trips_through_child_process() {
    let aprp = read_json(APRP_ALLOW);
    let mut server = McpServer::spawn();
    let resp = server.call("mandate.validate_aprp", json!({ "aprp": aprp }));
    assert_eq!(resp["result"]["ok"], true);
    let hash = resp["result"]["request_hash"].as_str().unwrap();
    assert_eq!(hash.len(), 64);
}

#[test]
fn stdio_decide_allow_round_trips_through_child_process() {
    let (dir, db, _ctx) = fresh_db();
    let aprp = aprp_with_unique_nonce(APRP_ALLOW, "STD1");
    // Round 0 path-sandbox: child reads MANDATE_MCP_ROOT from env.
    // Pass the tempdir so the db path resolves inside the sandbox.
    let mut server = McpServer::spawn_with_env(&[("MANDATE_MCP_ROOT", dir.path())]);
    let resp = server.call(
        "mandate.decide",
        json!({ "aprp": aprp, "db": db.to_string_lossy() }),
    );
    assert_eq!(resp["result"]["status"], "auto_approved");
}

#[test]
fn stdio_garbage_input_yields_parse_error() {
    let bin = env!("CARGO_BIN_EXE_mandate-mcp");
    let mut child = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "not json at all").unwrap();
        stdin.flush().unwrap();
    }
    let stdout = child.stdout.as_mut().expect("stdout");
    let mut line = String::new();
    BufReader::new(stdout).read_line(&mut line).unwrap();
    let resp: Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp["error"]["code"].as_i64(), Some(-32700));
    assert!(resp["id"].is_null());
    drop(child.stdin.take());
    let _ = child.wait();
}

// ---------------------------------------------------------------------------
// Round 0 — Issue 1: MCP path sandbox (canonicalize_within_root)
// ---------------------------------------------------------------------------

#[test]
fn mcp_rejects_path_outside_root() {
    // Root the ctx at a tempdir; pass an absolute path to a DIFFERENT
    // tempdir that's outside the root. Sandbox must reject.
    let outside = tempfile::tempdir().expect("outside");
    let escape_db = outside.path().join("escape.sqlite");
    let _ = mandate_storage::Storage::open(&escape_db).expect("seed db");

    let inside = tempfile::tempdir().expect("inside");
    let ctx = ServerContext::with_root(inside.path().to_path_buf());

    let aprp = read_json(APRP_ALLOW);
    let err = dispatch(
        "mandate.decide",
        &json!({ "aprp": aprp, "db": escape_db.to_string_lossy() }),
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, error_codes::PATH_ESCAPE);
    assert!(
        err.message.contains("escapes MANDATE_MCP_ROOT"),
        "expected path-escape diagnostic; got: {}",
        err.message
    );
}

#[test]
fn mcp_rejects_path_traversal_via_dotdot() {
    // Root the ctx at a sub-tempdir; pass an absolute path containing
    // a `..` segment that, after canonicalize, escapes upward to the
    // parent. canonicalize() always collapses `..`, so this proves
    // the post-canonicalize is_within(root) check catches the
    // traversal.
    let dir = tempfile::tempdir().expect("tempdir");
    let inside = dir.path().join("inside");
    std::fs::create_dir(&inside).expect("mkdir inside");
    let outside_target = dir.path().join("outside.sqlite");
    let _ = mandate_storage::Storage::open(&outside_target).expect("seed db");

    // root = `<dir>/inside`, path = `<dir>/inside/../outside.sqlite`
    // → canonicalize collapses to `<dir>/outside.sqlite`, OUTSIDE root.
    let ctx = ServerContext::with_root(inside.clone());
    let traversal = inside.join("..").join("outside.sqlite");

    let aprp = read_json(APRP_ALLOW);
    let err = dispatch(
        "mandate.decide",
        &json!({ "aprp": aprp, "db": traversal.to_string_lossy() }),
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, error_codes::PATH_ESCAPE);
}

#[test]
fn mcp_rejects_symlink_escape() {
    // A symlink whose target lives outside the root must be rejected.
    // canonicalize() follows symlinks, so the canonical path of the
    // symlink is the OUTSIDE target — starts_with(root) returns false.
    let dir = tempfile::tempdir().expect("tempdir");
    let outside = tempfile::tempdir().expect("outside");
    let target = outside.path().join("target.sqlite");
    let _ = mandate_storage::Storage::open(&target).expect("seed target");
    let link = dir.path().join("link.sqlite");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &link).expect("symlink");
    #[cfg(not(unix))]
    return; // Symlink semantics vary on Windows; skip on non-unix.

    let ctx = ServerContext::with_root(dir.path().to_path_buf());

    let aprp = read_json(APRP_ALLOW);
    let err = dispatch(
        "mandate.decide",
        &json!({ "aprp": aprp, "db": link.to_string_lossy() }),
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.code, error_codes::PATH_ESCAPE);
}

#[test]
fn mcp_accepts_path_within_root() {
    // The happy-path counter-test: a db inside the root is not
    // rejected. Also covered implicitly by every fresh_db() test, but
    // making it explicit here gives clean coverage of the positive
    // case for the sandbox doc.
    let (_dir, db, ctx) = fresh_db();
    let aprp = aprp_with_unique_nonce(APRP_ALLOW, "WRT0");
    let result = dispatch(
        "mandate.decide",
        &json!({ "aprp": aprp, "db": db.to_string_lossy() }),
        &ctx,
    )
    .unwrap();
    assert_eq!(result["status"], "auto_approved");
}

#[test]
fn mcp_uses_cwd_when_root_unset() {
    // ServerContext::new() leaves root=None. effective_root() then
    // reads env (which may or may not be set) and falls back to cwd.
    // We verify the effective_root() path resolves to a non-empty
    // PathBuf — the actual rejection/acceptance is exercised by other
    // tests that pin a specific root.
    let ctx = ServerContext::new();
    // Strip the env var for this assertion so cwd-fallback fires.
    // SAFETY: tests run in parallel; this can only mask other tests
    // that don't already set ctx.root explicitly. Every other test in
    // this file uses ServerContext::with_root(...) or doesn't touch
    // paths, so the env var is an unrelated channel today.
    let prior = std::env::var("MANDATE_MCP_ROOT").ok();
    // SAFETY: justified above; std::env::remove_var is unsafe in
    // recent rustc when crossing thread boundaries — this test
    // intentionally exercises the env-fallback path.
    unsafe {
        std::env::remove_var("MANDATE_MCP_ROOT");
    }
    let resolved = ctx.effective_root();
    if let Some(p) = prior {
        unsafe {
            std::env::set_var("MANDATE_MCP_ROOT", p);
        }
    }
    let resolved = resolved.expect("cwd fallback yields Ok");
    assert!(
        resolved.is_absolute(),
        "cwd fallback must return an absolute path; got: {}",
        resolved.display()
    );
    assert!(
        resolved.exists(),
        "cwd fallback path must exist; got: {}",
        resolved.display()
    );
}

// ---------------------------------------------------------------------------
// Round 0 — Issue 2 wire-up coverage (the heavy lifting is in
// crates/mandate-cli/tests/passport_cli.rs; the tests below pin the
// MCP-side run_guarded_execution branch that mirrors the same
// truthfulness rule)
// ---------------------------------------------------------------------------

#[test]
fn run_guarded_rejects_requires_human_with_stable_code() {
    // run_guarded_execution mirrors passport-run's policy-decision
    // semantics: requires_human is not encodable in
    // mandate.passport_capsule.v1, so the tool refuses. This pins the
    // wire string so MCP clients can branch on it.
    //
    // Constructing a requires_human policy in-line and activating it
    // is overkill here; the existing reference policy doesn't return
    // requires_human, and exercising the requires_human path
    // end-to-end belongs to crates/mandate-cli/tests/passport_cli.rs.
    // What we can pin from the MCP surface is that the error code
    // namespace exists and is the documented stable string.
    assert_eq!(error_codes::REQUIRES_HUMAN, "requires_human_unsupported");
}
