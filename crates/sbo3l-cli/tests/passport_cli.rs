//! Integration tests for `sbo3l passport verify --path <capsule>` (P1.1).
//!
//! Drives the real `sbo3l` binary against every fixture in
//! `test-corpus/passport/` and asserts the documented exit-code shape:
//! 0 on golden, 2 on every tampered shape (with the matching error
//! code in stderr).

use std::path::PathBuf;
use std::process::Command;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sbo3l"))
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
        .expect("spawn sbo3l")
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
        stdout.contains("sbo3l.passport_capsule.v1"),
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
// `sbo3l passport run` — P2.1 integration tests
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
    // F-6: passport run --schema-version defaults to v2.
    assert_eq!(parsed["schema"], "sbo3l.passport_capsule.v2");
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
// `sbo3l passport explain` — P2.1 integration tests
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
        "SBO3L Passport — capsule explanation",
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
    // F-6: passport run --schema-version defaults to v2; explain
    // surfaces the actual schema id from the capsule.
    assert_eq!(v["schema"], "sbo3l.passport_capsule.v2");
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

// ===========================================================================
// Codex P1/P2 fixes on PR #44 — coverage tests
// ===========================================================================

#[test]
fn run_returns_exit_1_on_missing_aprp_file() {
    // Codex P2: APRP IO failure must exit 1 (infrastructure error),
    // not exit 2 (semantic invalid input). The brief locks the exact
    // stderr format we surface here.
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = Command::new(cli_bin())
        .args(["passport", "run", "/no/such/aprp/file.json", "--db"])
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
            "mock",
            "--out",
        ])
        .arg(&out)
        .output()
        .expect("spawn passport run");
    assert_eq!(r.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(
        stderr.contains("failed to read APRP file"),
        "stderr must lock the documented format; got: {stderr}"
    );
    assert!(!out.exists(), "no capsule should be written on IO error");
}

#[test]
fn run_returns_exit_1_on_malformed_aprp_json() {
    // Codex P2: APRP parse failure also exits 1, with a distinct
    // stderr shape so consumers can branch on read-vs-parse.
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    let aprp = tmp.path().join("bad.json");
    std::fs::write(&aprp, b"{not json").unwrap();
    activate_policy(&db);
    let r = Command::new(cli_bin())
        .args(["passport", "run"])
        .arg(&aprp)
        .args(["--db"])
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
            "mock",
            "--out",
        ])
        .arg(&out)
        .output()
        .expect("spawn passport run");
    assert_eq!(r.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(
        stderr.contains("failed to parse APRP JSON"),
        "stderr must lock the documented format; got: {stderr}"
    );
    assert!(!out.exists());
}

#[test]
fn run_rejects_requires_human_decision_with_clear_error() {
    // Codex P1: when the policy returns a `requires_human` outcome,
    // the capsule schema's `decision.result` enum has no value for it
    // (only allow|deny). The CLI must reject **before** building the
    // capsule — otherwise the self-verify step catches the
    // contradiction late, after running the entire pipeline. We
    // construct the requires_human shape by activating a custom
    // policy whose `default_decision` is `requires_human` and whose
    // rules don't match the legit-x402 APRP, so the engine falls
    // through to the default.
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    let policy_path = tmp.path().join("requires-human-policy.json");

    // Read the reference policy and mutate it minimally:
    //   * default_decision = requires_human
    //   * remove the matching `allow-small-x402-api-call` rule so
    //     no rule fires for the legit fixture
    let mut policy: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(REF_POLICY).unwrap()).unwrap();
    policy["policy_id"] = serde_json::Value::String("requires-human-test".into());
    policy["description"] = serde_json::Value::String(
        "Test policy for requires_human path; default is requires_human \
         and no rule matches the legit-x402 fixture."
            .into(),
    );
    policy["default_decision"] = serde_json::Value::String("requires_human".into());
    let rules = policy["rules"].as_array_mut().unwrap();
    rules.retain(|r| {
        r["id"]
            .as_str()
            .map(|s| s != "allow-small-x402-api-call")
            .unwrap_or(true)
    });
    std::fs::write(&policy_path, serde_json::to_vec_pretty(&policy).unwrap()).unwrap();

    let pol = Command::new(cli_bin())
        .args(["policy", "activate"])
        .arg(&policy_path)
        .args(["--db"])
        .arg(&db)
        .output()
        .expect("policy activate");
    assert!(
        pol.status.success(),
        "policy activate must succeed: stderr={}",
        String::from_utf8_lossy(&pol.stderr)
    );

    let r = run_passport_run(APRP_ALLOW, &db, "keeperhub", &out, &[]);
    assert!(!r.status.success());
    assert_eq!(r.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(
        stderr.contains("requires_human policy outcomes"),
        "stderr must explain why we reject: {stderr}"
    );
    assert!(
        !out.exists(),
        "requires_human rejection must NOT have written a capsule"
    );
}

// ---------------------------------------------------------------------------
// Round 0 — Issue 2: requires_human is rejected as a PREFLIGHT, before any
// side-effect-producing pipeline work. The 3 tests below pin the new
// behaviour: nonce is NOT consumed, audit chain is NOT extended, no
// capsule is written. Previously the rejection happened post-pipeline,
// which violated the "no partial work persisted" comment in passport.rs.
// ---------------------------------------------------------------------------

/// Helper: activate the requires_human-default policy in `db_path`. Used
/// by the Issue 2 tests; mirrors the inline setup in
/// `run_rejects_requires_human_decision_with_clear_error` so each test
/// gets a clean, hermetic policy state.
fn activate_requires_human_policy(tmp: &std::path::Path, db: &std::path::Path) {
    let policy_path = tmp.join("requires-human-policy.json");
    let mut policy: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(REF_POLICY).unwrap()).unwrap();
    policy["policy_id"] = serde_json::Value::String("requires-human-test".into());
    policy["description"] = serde_json::Value::String(
        "Issue 2 preflight test: default_decision=requires_human, allow-rule removed.".into(),
    );
    policy["default_decision"] = serde_json::Value::String("requires_human".into());
    let rules = policy["rules"].as_array_mut().unwrap();
    rules.retain(|r| {
        r["id"]
            .as_str()
            .map(|s| s != "allow-small-x402-api-call")
            .unwrap_or(true)
    });
    std::fs::write(&policy_path, serde_json::to_vec_pretty(&policy).unwrap()).unwrap();
    let pol = Command::new(cli_bin())
        .args(["policy", "activate"])
        .arg(&policy_path)
        .args(["--db"])
        .arg(db)
        .output()
        .expect("policy activate");
    assert!(
        pol.status.success(),
        "policy activate must succeed: stderr={}",
        String::from_utf8_lossy(&pol.stderr)
    );
}

#[test]
#[allow(non_snake_case)]
fn run_rejects_requires_human_BEFORE_appending_audit_event() {
    // The audit chain must be unchanged after a requires_human rejection.
    // Pre-Round-0, the pipeline ran end-to-end before the rejection
    // fired — leaving an audit event behind despite the doc-comment
    // claiming "no partial work persisted."
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_requires_human_policy(tmp.path(), &db);

    let chain_before = sbo3l_storage::Storage::open(&db)
        .expect("open db")
        .audit_count()
        .expect("audit_count before");

    let r = run_passport_run(APRP_ALLOW, &db, "keeperhub", &out, &[]);
    assert_eq!(r.status.code(), Some(2));

    let chain_after = sbo3l_storage::Storage::open(&db)
        .expect("open db")
        .audit_count()
        .expect("audit_count after");
    assert_eq!(
        chain_before, chain_after,
        "audit chain length must NOT change after a requires_human preflight reject; \
         got {chain_before} → {chain_after}"
    );
}

#[test]
#[allow(non_snake_case)]
fn run_rejects_requires_human_BEFORE_consuming_nonce() {
    // The nonce must remain replay-able after a requires_human rejection.
    // Pre-Round-0 the full pipeline consumed the nonce before rejection
    // → a follow-up request reusing that nonce would have hit
    // policy.nonce_replay (HTTP 409). With the preflight in place the
    // nonce is untouched, so swapping the policy and re-running the
    // SAME APRP (same nonce) must succeed.
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out_rh = tmp.path().join("rh-capsule.json");
    let out_allow = tmp.path().join("allow-capsule.json");
    activate_requires_human_policy(tmp.path(), &db);

    // First run: requires_human policy → reject (preflight).
    let r1 = run_passport_run(APRP_ALLOW, &db, "keeperhub", &out_rh, &[]);
    assert_eq!(r1.status.code(), Some(2));
    assert!(!out_rh.exists(), "no capsule on rh path");

    // Swap to the reference policy (which would allow the legit
    // fixture). If the nonce had been consumed by the rejected run,
    // this second run would 409 on policy.nonce_replay.
    let pol = Command::new(cli_bin())
        .args(["policy", "activate", REF_POLICY, "--db"])
        .arg(&db)
        .output()
        .expect("policy activate ref");
    assert!(
        pol.status.success(),
        "swap to reference policy: stderr={}",
        String::from_utf8_lossy(&pol.stderr)
    );

    let r2 = run_passport_run(APRP_ALLOW, &db, "keeperhub", &out_allow, &[]);
    assert!(
        r2.status.success(),
        "second run with same APRP must succeed (nonce was NOT consumed); stderr={}",
        String::from_utf8_lossy(&r2.stderr)
    );
    assert!(out_allow.exists(), "allow path must write capsule");
}

// ===========================================================================
// P6.1 — `execution.executor_evidence` (Uniswap quote evidence carry-through)
// ===========================================================================
//
// These tests pin the wire-form contract between the Uniswap mock executor
// and the capsule's NEW `execution.executor_evidence` slot:
//
//   * On allow paths the slot is a non-null object with the documented
//     10 keys and survives `sbo3l passport verify`.
//   * On deny paths the slot is omitted (or null) — the executor never
//     ran, so there is no quote to attach.
//   * `live_evidence` stays null in mock mode regardless of evidence;
//     the verifier's bidirectional invariant is unchanged. This is the
//     point of putting evidence in a separate slot.

const UNI_EVIDENCE_KEYS: &[&str] = &[
    "quote_id",
    "quote_source",
    "input_token",
    "output_token",
    "route_tokens",
    "notional_in",
    "slippage_cap_bps",
    "quote_timestamp_unix",
    "quote_freshness_seconds",
    "recipient_address",
];

#[test]
fn uniswap_capsule_carries_executor_evidence_on_allow() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_ALLOW, &db, "uniswap", &out, &[]);
    assert!(
        r.status.success(),
        "uniswap allow path must succeed; stderr={}",
        String::from_utf8_lossy(&r.stderr)
    );
    let cap = read_capsule(&out);
    let evidence = &cap["execution"]["executor_evidence"];
    assert!(
        evidence.is_object(),
        "executor_evidence must be a non-null object on uniswap allow path; got: {evidence}"
    );
    let quote_id = evidence["quote_id"].as_str().unwrap_or("");
    assert!(
        quote_id.starts_with("mock-uniswap-quote-"),
        "expected mock-prefixed quote_id; got: {quote_id}"
    );
    // `live_evidence` MUST stay null in mock mode — this test fails
    // closed if a future refactor accidentally rewires evidence into
    // the wrong slot (the bug the P6.1 schema bump exists to prevent).
    assert!(
        cap["execution"]["live_evidence"].is_null(),
        "mock mode must keep live_evidence null even when executor_evidence is populated; got: {}",
        cap["execution"]["live_evidence"]
    );
}

#[test]
fn uniswap_capsule_omits_executor_evidence_on_deny() {
    // Deny path → `not_called` → executor never returns evidence →
    // capsule omits the field (or carries null). Either is schema-valid;
    // the test accepts both.
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_DENY, &db, "uniswap", &out, &[]);
    assert!(
        r.status.success(),
        "uniswap deny path must still emit a capsule; stderr={}",
        String::from_utf8_lossy(&r.stderr)
    );
    let cap = read_capsule(&out);
    assert_eq!(cap["decision"]["result"], "deny");
    assert_eq!(cap["execution"]["status"], "not_called");
    let evidence = &cap["execution"]["executor_evidence"];
    assert!(
        evidence.is_null(),
        "deny path must NOT carry executor_evidence (executor never ran); got: {evidence}"
    );
    // Sanity: the verifier's existing deny-with-execution invariant
    // still fires, and the round-trip verify exits 0.
    verify_round_trip(&out);
}

#[test]
fn uniswap_evidence_serialises_with_required_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_ALLOW, &db, "uniswap", &out, &[]);
    assert!(r.status.success());
    let cap = read_capsule(&out);
    let evidence = cap["execution"]["executor_evidence"]
        .as_object()
        .expect("executor_evidence must be an object on allow path");
    for key in UNI_EVIDENCE_KEYS {
        assert!(
            evidence.contains_key(*key),
            "executor_evidence missing required key {key:?}; got keys: {:?}",
            evidence.keys().collect::<Vec<_>>()
        );
    }
    // The 10-field struct is locked; new fields would need a deliberate
    // schema-bump conversation. Pin the exact key count too.
    assert_eq!(
        evidence.len(),
        UNI_EVIDENCE_KEYS.len(),
        "expected {} keys in executor_evidence; got: {:?}",
        UNI_EVIDENCE_KEYS.len(),
        evidence.keys().collect::<Vec<_>>()
    );
}

#[test]
fn uniswap_capsule_passes_existing_verify_with_evidence_populated() {
    // Round-trip: emit + verify on the new shape. This is the safety
    // net for the schema bump — if `sbo3l passport run` ever produces
    // a capsule that the (unchanged) verifier rejects, the bump
    // accidentally broke compat.
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("m.db");
    let out = tmp.path().join("capsule.json");
    activate_policy(&db);
    let r = run_passport_run(APRP_ALLOW, &db, "uniswap", &out, &[]);
    assert!(r.status.success());
    verify_round_trip(&out);
    // Belt-and-suspenders: the verifier is already invoked inside
    // cmd_run before the file is written, but a future refactor that
    // accidentally bypasses self-verify must still fail this test.
    let cap = read_capsule(&out);
    assert_eq!(cap["execution"]["mode"], "mock");
    assert!(cap["execution"]["executor_evidence"].is_object());
    assert!(cap["execution"]["live_evidence"].is_null());
}
