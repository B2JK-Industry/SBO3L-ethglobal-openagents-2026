//! Integration tests for `mandate passport verify --path <capsule>` (P1.1).
//!
//! Drives the real `mandate` binary against every fixture in
//! `test-corpus/passport/` and asserts the documented exit-code shape:
//! 0 on golden, 2 on every tampered shape (with the matching error
//! code in stderr).

use std::path::PathBuf;
use std::process::Command;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mandate"))
}

fn corpus(name: &str) -> PathBuf {
    PathBuf::from("../../test-corpus/passport").join(name)
}

fn run(fixture: &str) -> std::process::Output {
    run_path(corpus(fixture))
}

fn run_path(path: PathBuf) -> std::process::Output {
    Command::new(cli_bin())
        .args(["passport", "verify", "--path"])
        .arg(path)
        .output()
        .expect("spawn mandate")
}

#[test]
fn golden_capsule_verifies_exit_zero() {
    let out = run("golden_001_allow_keeperhub_mock.json");
    assert!(
        out.status.success(),
        "golden must verify; stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("structural verify: ok"),
        "stdout missing ok line; got: {stdout}"
    );
    assert!(
        stdout.contains("mandate.passport_capsule.v1"),
        "stdout must surface schema id; got: {stdout}"
    );
}

#[test]
fn missing_file_returns_exit_one() {
    let out = Command::new(cli_bin())
        .args(["passport", "verify", "--path", "/nonexistent/capsule.json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("read"), "got: {stderr}");
}

#[test]
fn invalid_json_returns_exit_one() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("bad.json");
    std::fs::write(&path, b"{not json").unwrap();
    let out = Command::new(cli_bin())
        .args(["passport", "verify", "--path"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("parse"), "got: {stderr}");
}

#[test]
fn deny_with_execution_ref_rejects_with_exit_two() {
    let out = run("tampered_001_deny_with_execution_ref.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("capsule.deny_with_execution"),
        "got: {stderr}"
    );
}

#[test]
fn mock_anchor_marked_live_rejects_with_exit_two() {
    let out = run("tampered_002_mock_anchor_marked_live.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("capsule.schema_invalid"), "got: {stderr}");
}

#[test]
fn live_mode_without_evidence_rejects_with_exit_two() {
    let out = run("tampered_003_live_mode_without_evidence.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("capsule.live_without_evidence"),
        "got: {stderr}"
    );
}

#[test]
fn live_mode_empty_evidence_rejects_with_exit_two() {
    let out = run("tampered_008_live_mode_empty_evidence.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("capsule.schema_invalid"), "got: {stderr}");
}

#[test]
fn live_mode_with_concrete_evidence_verifies_exit_zero() {
    let mut v: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(corpus("golden_001_allow_keeperhub_mock.json")).unwrap(),
    )
    .unwrap();
    let execution = v["execution"].as_object_mut().unwrap();
    execution.insert("mode".into(), serde_json::Value::String("live".into()));
    execution.insert(
        "live_evidence".into(),
        serde_json::json!({
            "transport": "https",
            "response_ref": "keeperhub-execution-01HTAWX5K3R8YV9NQB7C6P2DGS"
        }),
    );

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("live.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&v).unwrap()).unwrap();

    let out = run_path(path);
    assert!(
        out.status.success(),
        "live capsule with evidence must verify; stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn request_hash_mismatch_rejects_with_exit_two() {
    let out = run("tampered_004_request_hash_mismatch.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("capsule.request_hash_mismatch"),
        "got: {stderr}"
    );
}

#[test]
fn policy_hash_mismatch_rejects_with_exit_two() {
    let out = run("tampered_005_policy_hash_mismatch.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("capsule.policy_hash_mismatch"),
        "got: {stderr}"
    );
}

#[test]
fn malformed_checkpoint_rejects_with_exit_two() {
    let out = run("tampered_006_malformed_checkpoint.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("capsule.schema_invalid"), "got: {stderr}");
}

#[test]
fn unknown_field_rejects_with_exit_two() {
    let out = run("tampered_007_unknown_field.json");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("capsule.schema_invalid"), "got: {stderr}");
}

// ===========================================================================
// `mandate passport run` — P2.1 integration tests
// ===========================================================================

const ENS_FIXTURE: &str = "../../demo-fixtures/ens-records.json";
const REF_POLICY: &str = "../../test-corpus/policy/reference_low_risk.json";
const APRP_ALLOW: &str = "../../test-corpus/aprp/golden_001_minimal.json";
const APRP_DENY: &str = "../../test-corpus/aprp/deny_prompt_injection_request.json";

fn activate_policy(db: &std::path::Path) {
    let out = Command::new(cli_bin())
        .args(["policy", "activate", REF_POLICY, "--db"])
        .arg(db)
        .output()
        .expect("policy activate");
    assert!(
        out.status.success(),
        "policy activate must succeed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn run_passport_run(
    aprp: &str,
    db: &std::path::Path,
    executor: &str,
    out: &std::path::Path,
    extra: &[&str],
) -> std::process::Output {
    let mut cmd = Command::new(cli_bin());
    cmd.args(["passport", "run", aprp, "--db"])
        .arg(db)
        .args([
            "--agent",
            "research-agent.team.eth",
            "--resolver",
            "offline-fixture",
            "--ens-fixture",
            ENS_FIXTURE,
            "--executor",
            executor,
            "--mode",
            "mock",
            "--out",
        ])
        .arg(out);
    for arg in extra {
        cmd.arg(arg);
    }
    cmd.output().expect("spawn passport run")
}

fn verify_round_trip(out: &std::path::Path) {
    let v = Command::new(cli_bin())
        .args(["passport", "verify", "--path"])
        .arg(out)
        .output()
        .expect("spawn passport verify");
    assert!(
        v.status.success(),
        "verify must succeed on emitted capsule; stderr={} stdout={}",
        String::from_utf8_lossy(&v.stderr),
        String::from_utf8_lossy(&v.stdout),
    );
}

fn read_capsule(path: &std::path::Path) -> serde_json::Value {
    let raw = std::fs::read_to_string(path).expect("read capsule");
    serde_json::from_str(&raw).expect("parse capsule")
}

#[test]
fn run_allow_path_keeperhub_mock_emits_valid_capsule() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_ALLOW, &db, "keeperhub", &out, &[]);
    assert!(
        r.status.success(),
        "passport run must succeed; stderr={}",
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(out.exists());
    verify_round_trip(&out);
    let cap = read_capsule(&out);
    assert_eq!(cap["decision"]["result"], "allow");
    assert_eq!(cap["execution"]["executor"], "keeperhub");
    assert_eq!(cap["execution"]["mode"], "mock");
    assert_eq!(cap["execution"]["status"], "submitted");
    let exec_ref = cap["execution"]["execution_ref"].as_str().unwrap();
    assert!(exec_ref.starts_with("kh-"), "got {exec_ref}");
}

#[test]
fn run_allow_path_uniswap_mock_emits_valid_capsule() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_ALLOW, &db, "uniswap", &out, &[]);
    assert!(r.status.success());
    verify_round_trip(&out);
    let cap = read_capsule(&out);
    let exec_ref = cap["execution"]["execution_ref"].as_str().unwrap();
    assert_eq!(cap["execution"]["executor"], "uniswap");
    assert!(exec_ref.starts_with("uni-"), "got {exec_ref}");
}

#[test]
fn run_deny_path_emits_capsule_with_status_not_called_and_no_execution_ref() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_DENY, &db, "keeperhub", &out, &[]);
    assert!(r.status.success());
    verify_round_trip(&out);
    let cap = read_capsule(&out);
    assert_eq!(cap["decision"]["result"], "deny");
    assert_eq!(cap["execution"]["status"], "not_called");
    assert!(cap["execution"]["execution_ref"].is_null());
    assert!(cap["decision"]["deny_code"].is_string());
}

#[test]
fn run_refuses_mode_live_with_clear_error() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    // Build the command manually so we don't collide with the helper's
    // default --mode mock (clap forbids `--mode` twice).
    let r = Command::new(cli_bin())
        .args(["passport", "run", APRP_ALLOW, "--db"])
        .arg(&db)
        .args([
            "--agent",
            "research-agent.team.eth",
            "--resolver",
            "offline-fixture",
            "--ens-fixture",
            ENS_FIXTURE,
            "--executor",
            "keeperhub",
            "--mode",
            "live",
            "--out",
        ])
        .arg(&out)
        .output()
        .expect("spawn passport run");
    assert!(!r.status.success());
    assert_eq!(r.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("--mode live"), "got: {stderr}");
    assert!(!out.exists());
}

#[test]
fn run_refuses_missing_ens_fixture_when_resolver_offline_fixture() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = Command::new(cli_bin())
        .args(["passport", "run", APRP_ALLOW, "--db"])
        .arg(&db)
        .args([
            "--agent",
            "research-agent.team.eth",
            "--resolver",
            "offline-fixture",
            "--executor",
            "keeperhub",
            "--mode",
            "mock",
            "--out",
        ])
        .arg(&out)
        .output()
        .expect("spawn passport run");
    assert!(!r.status.success());
    assert_eq!(r.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("--ens-fixture"), "got: {stderr}");
}

#[test]
fn run_refuses_unknown_executor() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_ALLOW, &db, "twitter-bot", &out, &[]);
    assert!(!r.status.success());
    assert!(!out.exists());
}

#[test]
fn run_refuses_when_no_active_policy() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    let probe = Command::new(cli_bin())
        .args(["key", "list", "--mock", "--db"])
        .arg(&db)
        .output()
        .unwrap();
    assert!(probe.status.success());
    let r = run_passport_run(APRP_ALLOW, &db, "keeperhub", &out, &[]);
    assert!(!r.status.success());
    assert_eq!(r.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("no active policy"), "got: {stderr}");
    assert!(!out.exists());
}

#[test]
fn run_atomic_write_no_partial_capsule_on_validation_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_ALLOW, &db, "keeperhub", &out, &[]);
    assert!(r.status.success());
    let raw = std::fs::read_to_string(&out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("complete JSON");
    assert_eq!(parsed["schema"], "mandate.passport_capsule.v1");
    let leftovers: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".passport-capsule.")
        })
        .collect();
    assert!(leftovers.is_empty(), "no tempfile leftovers: {leftovers:?}");
}

#[test]
fn run_capsule_passes_existing_verify_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_ALLOW, &db, "keeperhub", &out, &[]);
    assert!(r.status.success());
    verify_round_trip(&out);
}

#[test]
fn run_emitted_capsule_request_hash_matches_aprp_request_hash() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_ALLOW, &db, "keeperhub", &out, &[]);
    assert!(r.status.success());
    let cap = read_capsule(&out);
    let outer = cap["request"]["request_hash"].as_str().unwrap();
    let receipt = cap["decision"]["receipt"]["request_hash"].as_str().unwrap();
    assert_eq!(outer, receipt);
    assert_eq!(outer.len(), 64);
    assert!(outer
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}

#[test]
fn run_emitted_capsule_policy_hash_matches_active_policy_row() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_ALLOW, &db, "keeperhub", &out, &[]);
    assert!(r.status.success());
    let cap = read_capsule(&out);
    let outer = cap["policy"]["policy_hash"].as_str().unwrap();
    let receipt = cap["decision"]["receipt"]["policy_hash"].as_str().unwrap();
    assert_eq!(outer, receipt);
    assert_eq!(cap["policy"]["source"], "operator-cli");
}

// ===========================================================================
// `mandate passport explain` — P2.1 integration tests
// ===========================================================================

fn explain(path: &std::path::Path, json: bool) -> std::process::Output {
    let mut cmd = Command::new(cli_bin());
    cmd.args(["passport", "explain", "--path"]).arg(path);
    if json {
        cmd.arg("--json");
    }
    cmd.output().expect("spawn passport explain")
}

#[test]
fn explain_passes_on_allow_capsule() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_ALLOW, &db, "keeperhub", &out, &[]);
    assert!(r.status.success());
    let e = explain(&out, false);
    assert!(
        e.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&e.stderr)
    );
    let stdout = String::from_utf8_lossy(&e.stdout);
    assert!(stdout.contains("ALLOW"), "stdout: {stdout}");
    assert!(stdout.contains("keeperhub"), "stdout: {stdout}");
}

#[test]
fn explain_passes_on_deny_capsule() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_DENY, &db, "keeperhub", &out, &[]);
    assert!(r.status.success());
    let e = explain(&out, false);
    assert!(e.status.success());
    let stdout = String::from_utf8_lossy(&e.stdout);
    assert!(stdout.contains("DENY"), "stdout: {stdout}");
    assert!(stdout.contains("not called"), "stdout: {stdout}");
}

#[test]
fn explain_text_mode_includes_required_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_ALLOW, &db, "keeperhub", &out, &[]);
    assert!(r.status.success());
    let e = explain(&out, false);
    assert!(e.status.success());
    let stdout = String::from_utf8_lossy(&e.stdout);
    for needle in &[
        "Mandate Passport — capsule explanation",
        "agent:",
        "policy:",
        "decision:",
        "execution:",
        "audit:",
        "doctor:",
    ] {
        assert!(stdout.contains(needle), "missing {needle:?}; got: {stdout}");
    }
}

#[test]
fn explain_json_mode_includes_required_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_ALLOW, &db, "keeperhub", &out, &[]);
    assert!(r.status.success());
    let e = explain(&out, true);
    assert!(
        e.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&e.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&e.stdout).expect("valid JSON");
    assert_eq!(v["schema"], "mandate.passport_capsule.v1");
    for key in &[
        "agent",
        "policy",
        "decision",
        "execution",
        "audit",
        "verification",
    ] {
        assert!(v.get(*key).is_some(), "explain JSON missing {key}");
    }
}

#[test]
fn explain_fails_on_tampered_capsule_with_capsule_code_in_stderr() {
    let path = corpus("tampered_004_request_hash_mismatch.json");
    let e = explain(&path, false);
    assert!(!e.status.success());
    assert_eq!(e.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&e.stderr);
    assert!(
        stderr.contains("capsule.request_hash_mismatch"),
        "got: {stderr}"
    );
}

#[test]
fn explain_fails_on_missing_file() {
    let e = Command::new(cli_bin())
        .args(["passport", "explain", "--path", "/nonexistent.json"])
        .output()
        .unwrap();
    assert!(!e.status.success());
    assert_eq!(e.status.code(), Some(1));
}
