//! `sbo3l passport {verify,run,explain}` (Passport P1.1 + P2.1).
//!
//! `verify` (P1.1) — structural verification of a
//! `sbo3l.passport_capsule.v1` JSON artifact via
//! `sbo3l-core::passport::verify_capsule`.
//!
//! `run` (P2.1) — orchestrates the existing offline SBO3L flow
//! (APRP → policy → budget → audit → signed receipt) end-to-end and
//! emits a `sbo3l.passport_capsule.v1` JSON to `--out`. **Wraps**
//! existing primitives (`sbo3l-server::router` oneshot, mock
//! KeeperHub/Uniswap executors, `Storage::policy_current`,
//! `Storage::audit_last`, `Storage::audit_checkpoint_create`); does
//! NOT reimplement crypto, audit chain semantics, or the policy
//! engine. Live mode is rejected with exit 2 in this PR — live
//! integration belongs in P5.1 / P6.1 / future work.
//!
//! `explain` (P2.1) — runs the P1.1 verifier on a capsule and prints
//! a 6–10 line human summary (or `--json` structured object). On
//! verifier failure exits 2 with the same `(capsule.<code>)` shape.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sbo3l_core::passport::{verify_capsule, CapsuleVerifyError};
use sbo3l_core::schema::validate_passport_capsule;
use sbo3l_execution::keeperhub::KeeperHubExecutor;
use sbo3l_execution::uniswap::UniswapExecutor;
use sbo3l_execution::GuardedExecutor;
use sbo3l_identity::ens::OfflineEnsResolver;
use sbo3l_policy::Policy;
use sbo3l_server::{AppState, PaymentRequestResponse, PaymentStatus};
use sbo3l_storage::audit_checkpoint_store::compute_chain_digest;
use sbo3l_storage::Storage;
use serde_json::{json, Map, Value};

/// `sbo3l passport verify --path <capsule>`
///
/// Exit codes:
/// - 0 — capsule verifies (schema + every cross-field invariant).
/// - 1 — IO / parse failure (file missing, not JSON).
/// - 2 — capsule is malformed, tampered, or internally inconsistent.
pub fn cmd_verify(path: &Path) -> ExitCode {
    let value = match load_capsule(path) {
        Ok(v) => v,
        Err(rc) => return rc,
    };

    match verify_capsule(&value) {
        Ok(()) => {
            print_verify_summary(&value);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("sbo3l passport verify: {} ({})", e, e.code());
            ExitCode::from(2)
        }
    }
}

/// `sbo3l passport explain --path <capsule> [--json]`
///
/// Reads + verifies a capsule via the P1.1 verifier; on success
/// prints a concise human (or JSON) summary. On verifier failure
/// exits 2 with `(capsule.<code>)` in stderr — same shape as
/// `verify`, so any tooling that branches on verify codes also
/// works for explain.
pub fn cmd_explain(path: &Path, json_out: bool) -> ExitCode {
    let value = match load_capsule(path) {
        Ok(v) => v,
        Err(rc) => return rc,
    };
    if let Err(e) = verify_capsule(&value) {
        eprintln!("sbo3l passport explain: {} ({})", e, e.code());
        return ExitCode::from(2);
    }

    let summary = build_explanation(&value);
    if json_out {
        match serde_json::to_string_pretty(&summary) {
            Ok(s) => {
                println!("{s}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("sbo3l passport explain: serialise: {e}");
                ExitCode::from(1)
            }
        }
    } else {
        print_explanation_text(&summary);
        ExitCode::SUCCESS
    }
}

fn load_capsule(path: &Path) -> Result<Value, ExitCode> {
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sbo3l passport: read {} failed: {e}", path.display());
            return Err(ExitCode::from(1));
        }
    };
    serde_json::from_str(&raw).map_err(|e| {
        eprintln!("sbo3l passport: parse {} failed: {e}", path.display());
        ExitCode::from(1)
    })
}

fn print_verify_summary(value: &Value) {
    let schema = value.get("schema").and_then(|v| v.as_str()).unwrap_or("?");
    let result = value
        .get("decision")
        .and_then(|d| d.get("result"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let executor = value
        .get("execution")
        .and_then(|e| e.get("executor"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let mode = value
        .get("execution")
        .and_then(|e| e.get("mode"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let exec_status = value
        .get("execution")
        .and_then(|e| e.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let policy_hash = value
        .get("policy")
        .and_then(|p| p.get("policy_hash"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let request_hash = value
        .get("request")
        .and_then(|r| r.get("request_hash"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let policy_prefix: String = policy_hash.chars().take(12).collect();
    let request_prefix: String = request_hash.chars().take(12).collect();

    println!("passport: schema:        {schema}");
    println!("passport: decision:      {result}");
    println!("passport: executor:      {executor} (mode={mode}, status={exec_status})");
    println!("passport: policy_hash:   {policy_prefix}…");
    println!("passport: request_hash:  {request_prefix}…");
    println!("passport: structural verify: ok");
}

/// Compose the explanation as a structured JSON object — same shape
/// drives both the `--json` output and the text-mode renderer.
fn build_explanation(value: &Value) -> Value {
    let agent_id = value
        .pointer("/agent/agent_id")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let ens_name = value
        .pointer("/agent/ens_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let resolver = value
        .pointer("/agent/resolver")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let result = value
        .pointer("/decision/result")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let matched_rule = value
        .pointer("/decision/matched_rule")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let deny_code = value
        .pointer("/decision/deny_code")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let executor = value
        .pointer("/execution/executor")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let mode = value
        .pointer("/execution/mode")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let exec_status = value
        .pointer("/execution/status")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let exec_ref = value
        .pointer("/execution/execution_ref")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let audit_event_id = value
        .pointer("/audit/audit_event_id")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let mock_anchor_ref = value
        .pointer("/audit/checkpoint/mock_anchor_ref")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let policy_hash = value
        .pointer("/policy/policy_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let policy_version = value
        .pointer("/policy/policy_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let doctor_status = value
        .pointer("/verification/doctor_status")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let live_claims_count = value
        .pointer("/verification/live_claims")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    json!({
        "schema": "sbo3l.passport_capsule.v1",
        "agent": {
            "agent_id": agent_id,
            "ens_name": ens_name,
            "resolver": resolver,
        },
        "policy": {
            "policy_hash": policy_hash,
            "policy_version": policy_version,
        },
        "decision": {
            "result": result,
            "matched_rule": matched_rule,
            "deny_code": deny_code,
        },
        "execution": {
            "executor": executor,
            "mode": mode,
            "status": exec_status,
            "execution_ref": exec_ref,
        },
        "audit": {
            "audit_event_id": audit_event_id,
            "mock_anchor_ref": mock_anchor_ref,
        },
        "verification": {
            "doctor_status": doctor_status,
            "offline_verifiable": true,
            "live_claims_count": live_claims_count,
        },
    })
}

fn print_explanation_text(s: &Value) {
    let agent_id = s
        .pointer("/agent/agent_id")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let ens_name = s
        .pointer("/agent/ens_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let resolver = s
        .pointer("/agent/resolver")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let result = s
        .pointer("/decision/result")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let matched_rule = s
        .pointer("/decision/matched_rule")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let deny_code = s
        .pointer("/decision/deny_code")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let executor = s
        .pointer("/execution/executor")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let mode = s
        .pointer("/execution/mode")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let exec_status = s
        .pointer("/execution/status")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let exec_ref = s
        .pointer("/execution/execution_ref")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let audit_event_id = s
        .pointer("/audit/audit_event_id")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let mock_anchor_ref = s
        .pointer("/audit/mock_anchor_ref")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let policy_hash = s
        .pointer("/policy/policy_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let policy_version = s
        .pointer("/policy/policy_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let policy_prefix: String = policy_hash.chars().take(12).collect();
    let doctor_status = s
        .pointer("/verification/doctor_status")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let live_claims_count = s
        .pointer("/verification/live_claims_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let ens_part = if ens_name.is_empty() {
        String::new()
    } else {
        format!(" ({ens_name})")
    };
    println!("SBO3L Passport — capsule explanation");
    println!("  agent:        {agent_id}{ens_part}, resolver={resolver}");
    println!("  policy:       v{policy_version}, hash={policy_prefix}…");
    if result == "deny" {
        println!(
            "  decision:     DENY (matched_rule={}, deny_code={})",
            non_empty_or_dash(matched_rule),
            non_empty_or_dash(deny_code),
        );
        println!(
            "  execution:    not called (executor={executor}, mode={mode}, status={exec_status})"
        );
    } else {
        println!(
            "  decision:     ALLOW (matched_rule={})",
            non_empty_or_dash(matched_rule)
        );
        println!(
            "  execution:    {executor} (mode={mode}, status={exec_status}, ref={})",
            non_empty_or_dash(exec_ref)
        );
    }
    println!("  audit:        event_id={audit_event_id}");
    if !mock_anchor_ref.is_empty() {
        println!("  checkpoint:   mock_anchor_ref={mock_anchor_ref}");
    }
    println!(
        "  doctor:       {doctor_status}, offline-verifiable: yes, live-claims: {live_claims_count}"
    );
}

fn non_empty_or_dash(s: &str) -> &str {
    if s.is_empty() {
        "—"
    } else {
        s
    }
}

// ===========================================================================
// `sbo3l passport run` — orchestration (P2.1)
// ===========================================================================

/// Configuration for `cmd_run`. One struct so `main.rs` can map clap
/// fields directly without an arity explosion at the call site.
#[derive(Debug, Clone)]
pub struct RunArgs {
    pub aprp_path: PathBuf,
    pub db_path: PathBuf,
    pub agent: String,
    pub resolver: ResolverChoice,
    pub ens_fixture: Option<PathBuf>,
    pub executor: ExecutorChoice,
    pub mode: ModeChoice,
    pub out_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverChoice {
    OfflineFixture,
    LiveEns,
}

impl ResolverChoice {
    fn label(self) -> &'static str {
        match self {
            Self::OfflineFixture => "offline-fixture",
            Self::LiveEns => "live-ens",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorChoice {
    Keeperhub,
    Uniswap,
}

impl ExecutorChoice {
    fn label(self) -> &'static str {
        match self {
            Self::Keeperhub => "keeperhub",
            Self::Uniswap => "uniswap",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeChoice {
    Mock,
    Live,
}

impl ModeChoice {
    fn label(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Live => "live",
        }
    }
}

/// `sbo3l passport run <APRP> --db <PATH> ...`
///
/// Exit codes:
/// - 0 — capsule emitted to `--out`.
/// - 1 — IO / parse failure (file missing, bad JSON, executor backend
///   IO error, capsule write failure).
/// - 2 — invalid input (bad APRP, ENS resolution failed, mode=live
///   rejected by P2.1, executor refused, capsule self-verify failed
///   — i.e. we somehow built a capsule that wouldn't pass our own
///   verifier; that's a hard refuse, not a "ship anyway").
pub fn cmd_run(args: RunArgs) -> ExitCode {
    // Live mode is rejected here. P5.1 / P6.1 / future work will
    // un-gate this behind real credentials and live evidence; until
    // then, the CLI must not produce a capsule that *claims* live
    // mode without proof.
    if args.mode == ModeChoice::Live {
        eprintln!(
            "sbo3l passport run: --mode live is not implemented in P2.1 \
             (truthfulness rule: live claims require real evidence). Re-run \
             with --mode mock; live mode lands in P5.1/P6.1 with concrete \
             credentials + live_evidence."
        );
        return ExitCode::from(2);
    }

    // 1. ENS resolver fixture. Required when resolver is offline.
    let resolver_path = match args.resolver {
        ResolverChoice::OfflineFixture => match args.ens_fixture.as_ref() {
            Some(p) => p.clone(),
            None => {
                eprintln!(
                    "sbo3l passport run: --resolver offline-fixture requires \
                     --ens-fixture <PATH>"
                );
                return ExitCode::from(2);
            }
        },
        ResolverChoice::LiveEns => {
            eprintln!(
                "sbo3l passport run: --resolver live-ens is reserved for P4.1 \
                 (live ENS resolver). Use --resolver offline-fixture in P2.1."
            );
            return ExitCode::from(2);
        }
    };

    // 2. Read + parse the APRP body. IO failure (file missing,
    // permission denied, …) and parse failure (malformed JSON) are
    // both **infrastructure** errors and surface as exit 1, matching
    // the contract in `docs/cli/passport.md`. Exit 2 is reserved for
    // semantic invalid-input cases (bad mode, missing ens-fixture,
    // no active policy, requires_human, …) — Codex P2 on PR #44
    // pointed out the original code conflated these.
    let aprp_raw = match std::fs::read_to_string(&args.aprp_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "passport run: failed to read APRP file {}: {e}",
                args.aprp_path.display()
            );
            return ExitCode::from(1);
        }
    };
    let aprp_value: Value = match serde_json::from_str(&aprp_raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "passport run: failed to parse APRP JSON in {}: {e}",
                args.aprp_path.display()
            );
            return ExitCode::from(1);
        }
    };

    // 3. Resolve ENS records via the existing offline resolver.
    let ens_records_obj = match OfflineEnsResolver::from_file(&resolver_path) {
        Ok(resolver) => match resolver.records.get(&args.agent).cloned() {
            Some(rec) => match serde_json::to_value(&rec) {
                Ok(Value::Object(map)) => map,
                _ => {
                    eprintln!(
                        "sbo3l passport run: ENS records for {} did not \
                         serialise as a JSON object; fixture is malformed",
                        args.agent
                    );
                    return ExitCode::from(2);
                }
            },
            None => {
                eprintln!(
                    "sbo3l passport run: agent {} not present in ENS fixture {}",
                    args.agent,
                    resolver_path.display()
                );
                return ExitCode::from(2);
            }
        },
        Err(e) => {
            eprintln!(
                "sbo3l passport run: load ENS fixture {} failed: {e}",
                resolver_path.display()
            );
            return ExitCode::from(2);
        }
    };

    // 4. Look up the active policy from the supplied DB. Reuses the
    // PSM-A3 storage API verbatim — the capsule's `policy.*` block is
    // populated entirely from this row.
    let active_policy = match Storage::open(&args.db_path)
        .map_err(|e| format!("open db {}: {e}", args.db_path.display()))
        .and_then(|s| {
            s.policy_current()
                .map_err(|e| format!("policy_current: {e}"))
        }) {
        Ok(Some(rec)) => rec,
        Ok(None) => {
            eprintln!(
                "sbo3l passport run: no active policy in {} — run \
                 `sbo3l policy activate <file> --db {}` first",
                args.db_path.display(),
                args.db_path.display()
            );
            return ExitCode::from(2);
        }
        Err(msg) => {
            eprintln!("sbo3l passport run: {msg}");
            return ExitCode::from(1);
        }
    };

    // 5. Re-parse the active policy JSON into a `Policy` so the
    // existing pipeline accepts it. Schema validation happens
    // inside `Policy::parse_json`; failure here means the policy
    // table contains JSON that doesn't validate, which is itself a
    // serious bug — surface it loudly.
    let policy = match Policy::parse_json(&active_policy.policy_json) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "sbo3l passport run: active policy v{} in {} no longer \
                 validates: {e}",
                active_policy.version,
                args.db_path.display()
            );
            return ExitCode::from(2);
        }
    };

    // 5b. Preflight policy decision (Round 0 audit fix — Issue 2).
    //
    // `sbo3l_policy::engine::decide` is a pure function: it takes the
    // already-loaded `Policy` plus the typed `PaymentRequest` and
    // returns the decision *without* consuming a nonce, appending an
    // audit event, or signing a receipt. Running it here — before the
    // AppState pipeline in step 6 — means we can reject
    // `requires_human` outcomes (which sbo3l.passport_capsule.v1
    // cannot encode) without producing any of those side effects.
    //
    // Previously this check lived at step 6b, AFTER the pipeline. The
    // doc-comment there claimed "no partial work persisted", but in
    // fact a `requires_human` decision reaching cmd_run would consume
    // a nonce, append an audit event, and emit a signed receipt before
    // the rejection fired. The audit caught this; the post-pipeline
    // check below is now defence-in-depth (it should be unreachable
    // since the preflight catches it first).
    let payment_request: sbo3l_core::aprp::PaymentRequest =
        match serde_json::from_value(aprp_value.clone()) {
            Ok(req) => req,
            Err(e) => {
                eprintln!(
                    "passport run: APRP body did not parse as PaymentRequest \
                     (preflight typed parse): {e}"
                );
                return ExitCode::from(2);
            }
        };
    match sbo3l_policy::engine::decide(&policy, &payment_request) {
        Ok(outcome)
            if matches!(
                outcome.decision,
                sbo3l_policy::engine::Decision::RequiresHuman
            ) =>
        {
            let matched = outcome
                .matched_rule_id
                .as_deref()
                .unwrap_or("(no matched rule)");
            eprintln!(
                "passport run does not support requires_human policy outcomes \
                 in this build; sbo3l.passport_capsule.v1 only encodes \
                 allow/deny. The decision was requires_human (matched_rule={matched}); \
                 use the regular API surface for human-review workflows."
            );
            return ExitCode::from(2);
        }
        Ok(_) => { /* allow / deny — fall through to pipeline */ }
        Err(e) => {
            eprintln!("passport run: policy preflight error: {e}");
            return ExitCode::from(2);
        }
    }

    // 6. Drive the existing `POST /v1/payment-requests` pipeline
    // in-process via the same oneshot pattern research-agent uses.
    // A fresh on-disk Storage handle is given to AppState so the
    // request, audit chain, idempotency, and signing all flow through
    // production code paths.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("sbo3l passport run: tokio runtime init failed: {e}");
            return ExitCode::from(1);
        }
    };
    let response = match runtime.block_on(async {
        let storage = Storage::open(&args.db_path)
            .map_err(|e| format!("open db {}: {e}", args.db_path.display()))?;
        let state = AppState::new(policy.clone(), storage);
        let app = sbo3l_server::router(state);
        oneshot_payment_request(app, &aprp_value).await
    }) {
        Ok(resp) => resp,
        Err(msg) => {
            eprintln!("sbo3l passport run: pipeline failed: {msg}");
            return ExitCode::from(2);
        }
    };

    // 6b. Defence-in-depth `requires_human` reject.
    //
    // The step 5b preflight (Round 0 / Issue 2) already filters
    // `requires_human` out before the pipeline runs, so this branch
    // SHOULD be unreachable today — the pipeline cannot promote an
    // allow/deny back into requires_human. Kept as a safety net in
    // case a future refactor makes the pipeline outcome diverge from
    // `policy::decide`. If we ever reach this branch, that's a bug;
    // the rejection here means we still don't write a malformed
    // capsule, but the "no partial work persisted" guarantee no
    // longer holds at this point (nonce + audit have already been
    // committed) — surface that loudly.
    if matches!(
        response.receipt.decision,
        sbo3l_core::receipt::Decision::RequiresHuman
    ) {
        let matched = response
            .matched_rule_id
            .as_deref()
            .unwrap_or("(no matched rule)");
        eprintln!(
            "passport run does not support requires_human policy outcomes \
             in this build; sbo3l.passport_capsule.v1 only encodes \
             allow/deny. The decision was requires_human (matched_rule={matched}); \
             use the regular API surface for human-review workflows."
        );
        return ExitCode::from(2);
    }

    // 7. Allow path → call mock executor; deny path → execution.status
    // = "not_called" (HARD invariant — verified by tampered_001 in
    // P1.1). Mode is forced to `mock` here because we rejected `live`
    // up top.
    let allow_path = matches!(response.status, PaymentStatus::AutoApproved);
    let exec_block = if allow_path {
        match call_mock_executor(args.executor, &aprp_value, &response) {
            Ok(block) => block,
            Err(msg) => {
                eprintln!("sbo3l passport run: executor: {msg}");
                return ExitCode::from(1);
            }
        }
    } else {
        deny_execution_block(args.executor)
    };

    // 8. Re-open storage to read the just-appended audit event +
    // create a checkpoint. The previous AppState's storage handle
    // was dropped at the end of the runtime block; SQLite WAL mode
    // is happy with multiple sequential handles to the same file.
    let (audit_block, checkpoint_payload) =
        match build_audit_and_checkpoint_blocks(&args.db_path, &response.audit_event_id) {
            Ok(t) => t,
            Err(msg) => {
                eprintln!("sbo3l passport run: audit/checkpoint: {msg}");
                return ExitCode::from(1);
            }
        };

    // 9. Compose the capsule.
    let capsule = build_capsule(BuildCapsuleArgs {
        aprp: aprp_value,
        ens_records: ens_records_obj,
        agent_name: args.agent.clone(),
        resolver_label: args.resolver.label(),
        active_policy,
        response,
        executor_label: args.executor.label(),
        mode_label: args.mode.label(),
        execution_block: exec_block,
        audit_block,
        checkpoint_payload,
    });

    // 10. Self-verify against the schema BEFORE writing. We never
    // emit a capsule that would fail `passport verify`.
    if let Err(e) = validate_passport_capsule(&capsule) {
        eprintln!(
            "sbo3l passport run: refusing to emit — assembled capsule fails \
             schema validation: {e}"
        );
        return ExitCode::from(2);
    }
    if let Err(e) = verify_capsule(&capsule) {
        eprintln!(
            "sbo3l passport run: refusing to emit — assembled capsule fails \
             cross-field verifier: {e} ({})",
            e.code()
        );
        return ExitCode::from(2);
    }

    // 11. Atomic write: tempfile in same dir + rename. A reader who
    // opens the path mid-write either sees the prior contents or the
    // complete new file — never half a JSON object.
    if let Err(e) = atomic_write_json(&args.out_path, &capsule) {
        eprintln!(
            "sbo3l passport run: write {} failed: {e}",
            args.out_path.display()
        );
        return ExitCode::from(1);
    }

    // 12. Friendly summary of what was emitted.
    let result = match response_decision_str(&capsule) {
        Some("allow") => "ALLOW",
        Some("deny") => "DENY",
        _ => "?",
    };
    println!(
        "passport run: agent={} executor={} mode={} decision={}",
        args.agent,
        args.executor.label(),
        args.mode.label(),
        result
    );
    println!("passport run: wrote {}", args.out_path.display());
    ExitCode::SUCCESS
}

fn response_decision_str(capsule: &Value) -> Option<&str> {
    capsule.pointer("/decision/result").and_then(|v| v.as_str())
}

async fn oneshot_payment_request(
    app: axum::Router,
    aprp: &Value,
) -> Result<PaymentRequestResponse, String> {
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    let body = serde_json::to_vec(aprp).map_err(|e| format!("serialise aprp: {e}"))?;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/payment-requests")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .map_err(|e| format!("build request: {e}"))?;
    let resp = app
        .oneshot(req)
        .await
        .map_err(|e| format!("oneshot: {e}"))?;
    let status = resp.status();
    let body_bytes = resp
        .into_body()
        .collect()
        .await
        .map_err(|e| format!("read response body: {e}"))?
        .to_bytes();
    if !status.is_success() {
        let v: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);
        return Err(format!("HTTP {status}: {v}"));
    }
    serde_json::from_slice(&body_bytes).map_err(|e| format!("parse response: {e}"))
}

fn call_mock_executor(
    choice: ExecutorChoice,
    aprp: &Value,
    response: &PaymentRequestResponse,
) -> Result<Map<String, Value>, String> {
    // The existing `GuardedExecutor::execute` takes a typed
    // `PaymentRequest`. Round-trip via serde so this CLI doesn't
    // depend on the APRP struct surface (tests already pin that).
    let request: sbo3l_core::aprp::PaymentRequest =
        serde_json::from_value(aprp.clone()).map_err(|e| format!("aprp typed parse: {e}"))?;
    let executor: Box<dyn GuardedExecutor> = match choice {
        ExecutorChoice::Keeperhub => Box::new(KeeperHubExecutor::local_mock()),
        ExecutorChoice::Uniswap => Box::new(UniswapExecutor::local_mock()),
    };
    let exec_receipt = executor
        .execute(&request, &response.receipt)
        .map_err(|e| format!("executor: {e}"))?;
    // status: mock allow path is "submitted" by spec — the mock
    // executor returns immediately without a separate confirmation,
    // so we surface the optimistic state, not a fake "succeeded".
    let mut block = Map::new();
    block.insert("executor".into(), Value::String(choice.label().to_string()));
    block.insert("mode".into(), Value::String("mock".to_string()));
    block.insert(
        "execution_ref".into(),
        Value::String(exec_receipt.execution_ref),
    );
    block.insert("status".into(), Value::String("submitted".to_string()));
    block.insert("sponsor_payload_hash".into(), Value::Null);
    block.insert("live_evidence".into(), Value::Null);
    Ok(block)
}

fn deny_execution_block(choice: ExecutorChoice) -> Map<String, Value> {
    let mut block = Map::new();
    block.insert("executor".into(), Value::String(choice.label().to_string()));
    block.insert("mode".into(), Value::String("mock".to_string()));
    block.insert("execution_ref".into(), Value::Null);
    block.insert("status".into(), Value::String("not_called".to_string()));
    block.insert("sponsor_payload_hash".into(), Value::Null);
    block.insert("live_evidence".into(), Value::Null);
    block
}

/// `(audit_block, checkpoint_payload)` — returned together because both
/// are derived from the same `Storage::open(db_path)` handle and share
/// the just-appended audit-chain tip lookup.
type AuditAndCheckpointBlocks = (Map<String, Value>, Map<String, Value>);

fn build_audit_and_checkpoint_blocks(
    db_path: &Path,
    audit_event_id: &str,
) -> Result<AuditAndCheckpointBlocks, String> {
    let mut storage = Storage::open(db_path).map_err(|e| format!("reopen db: {e}"))?;
    let last = storage
        .audit_last()
        .map_err(|e| format!("audit_last: {e}"))?
        .ok_or_else(|| {
            "audit chain is empty after the request — pipeline did not append".to_string()
        })?;
    if last.event.id != audit_event_id {
        return Err(format!(
            "audit chain tip id {} doesn't match the response audit_event_id {} — \
             concurrent writers? refusing to compose a possibly-misaligned capsule",
            last.event.id, audit_event_id
        ));
    }
    let event_hash = last.event_hash.clone();
    let prev_event_hash = last.event.prev_event_hash.clone();

    // Build a checkpoint over the chain prefix through the just-
    // appended event. Reuses PSM-A4's existing surface verbatim.
    let hashes = storage
        .audit_event_hashes_in_order()
        .map_err(|e| format!("audit_event_hashes_in_order: {e}"))?;
    let chain_digest = compute_chain_digest(&hashes).map_err(|e| format!("chain_digest: {e}"))?;
    let now = chrono::Utc::now();
    let checkpoint = storage
        .audit_checkpoint_create(&chain_digest, now)
        .map_err(|e| format!("audit_checkpoint_create: {e}"))?;

    let mut audit_block = Map::new();
    audit_block.insert(
        "audit_event_id".into(),
        Value::String(audit_event_id.to_string()),
    );
    audit_block.insert("prev_event_hash".into(), Value::String(prev_event_hash));
    audit_block.insert("event_hash".into(), Value::String(event_hash));
    audit_block.insert(
        "bundle_ref".into(),
        Value::String("sbo3l.audit_bundle.v1".to_string()),
    );

    let mut checkpoint_payload = Map::new();
    checkpoint_payload.insert(
        "schema".into(),
        Value::String("sbo3l.audit_checkpoint.v1".to_string()),
    );
    checkpoint_payload.insert("sequence".into(), Value::Number(checkpoint.sequence.into()));
    checkpoint_payload.insert(
        "latest_event_id".into(),
        Value::String(checkpoint.latest_event_id),
    );
    checkpoint_payload.insert(
        "latest_event_hash".into(),
        Value::String(checkpoint.latest_event_hash),
    );
    checkpoint_payload.insert(
        "chain_digest".into(),
        Value::String(checkpoint.chain_digest),
    );
    checkpoint_payload.insert("mock_anchor".into(), Value::Bool(true));
    checkpoint_payload.insert(
        "mock_anchor_ref".into(),
        Value::String(checkpoint.mock_anchor_ref),
    );
    checkpoint_payload.insert(
        "created_at".into(),
        Value::String(checkpoint.created_at.to_rfc3339()),
    );

    Ok((audit_block, checkpoint_payload))
}

struct BuildCapsuleArgs {
    aprp: Value,
    ens_records: Map<String, Value>,
    agent_name: String,
    resolver_label: &'static str,
    active_policy: sbo3l_storage::ActivePolicyRecord,
    response: PaymentRequestResponse,
    executor_label: &'static str,
    mode_label: &'static str,
    execution_block: Map<String, Value>,
    audit_block: Map<String, Value>,
    checkpoint_payload: Map<String, Value>,
}

fn build_capsule(args: BuildCapsuleArgs) -> Value {
    // Pull `sbo3l:agent_id` out of ENS records; fall back to the
    // CLI-supplied agent name if (somehow) missing. The schema
    // requires `agent.agent_id` to match the receipt's agent_id; we
    // prefer the receipt's value as the canonical source.
    let receipt = serde_json::to_value(&args.response.receipt).unwrap_or(Value::Null);
    let receipt_agent_id = receipt
        .get("agent_id")
        .and_then(|v| v.as_str())
        .unwrap_or("research-agent-01")
        .to_string();
    let receipt_signature = receipt
        .pointer("/signature/signature_hex")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Build a copy of the ens records with the same key set. The map
    // serialises with HashMap iteration order; re-insert into a
    // BTreeMap-style ordering by going through serde_json::Map again.
    // (Map preserves insertion order on serialize_pretty; we keep it
    // as-is for compactness.)
    let agent_block = json!({
        "agent_id": receipt_agent_id,
        "ens_name": args.agent_name,
        "resolver": args.resolver_label,
        "records": Value::Object(args.ens_records),
    });

    let mut request_block = Map::new();
    request_block.insert("aprp".into(), args.aprp.clone());
    request_block.insert(
        "request_hash".into(),
        Value::String(args.response.request_hash.clone()),
    );
    request_block.insert(
        "idempotency_key".into(),
        args.aprp
            .get("idempotency_key")
            .cloned()
            .unwrap_or(Value::Null),
    );
    request_block.insert(
        "nonce".into(),
        args.aprp.get("nonce").cloned().unwrap_or(Value::Null),
    );

    let mut policy_block = Map::new();
    policy_block.insert(
        "policy_hash".into(),
        Value::String(args.active_policy.policy_hash.clone()),
    );
    policy_block.insert(
        "policy_version".into(),
        Value::Number(args.active_policy.version.into()),
    );
    policy_block.insert(
        "activated_at".into(),
        Value::String(args.active_policy.activated_at.to_rfc3339()),
    );
    policy_block.insert(
        "source".into(),
        Value::String(args.active_policy.source.clone()),
    );

    // `requires_human` is rejected up in `cmd_run` (Codex P1 on PR #44)
    // before this function runs — the capsule schema's
    // `decision.result` enum is `{allow, deny}` only. The
    // `unreachable!` is defense-in-depth: if a future refactor
    // bypasses the early reject, we panic loudly rather than silently
    // collapse the third decision into "deny" and ship a misleading
    // capsule.
    let result = match args.response.decision {
        sbo3l_core::receipt::Decision::Allow => "allow",
        sbo3l_core::receipt::Decision::Deny => "deny",
        sbo3l_core::receipt::Decision::RequiresHuman => {
            unreachable!("requires_human must be rejected by cmd_run before build_capsule runs")
        }
    };
    let mut decision_block = Map::new();
    decision_block.insert("result".into(), Value::String(result.to_string()));
    decision_block.insert(
        "matched_rule".into(),
        args.response
            .matched_rule_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    decision_block.insert(
        "deny_code".into(),
        args.response
            .deny_code
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    decision_block.insert("receipt".into(), receipt);
    decision_block.insert("receipt_signature".into(), Value::String(receipt_signature));

    let mut audit_block = args.audit_block;
    audit_block.insert("checkpoint".into(), Value::Object(args.checkpoint_payload));

    let _ = args.executor_label; // captured into execution_block already
    let _ = args.mode_label;

    let verification_block = json!({
        "doctor_status": "not_run",
        "offline_verifiable": true,
        "live_claims": Value::Array(Vec::new()),
    });

    json!({
        "schema": "sbo3l.passport_capsule.v1",
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "agent": agent_block,
        "request": Value::Object(request_block),
        "policy": Value::Object(policy_block),
        "decision": Value::Object(decision_block),
        "execution": Value::Object(args.execution_block),
        "audit": Value::Object(audit_block),
        "verification": verification_block,
    })
}

fn atomic_write_json(out_path: &Path, value: &Value) -> std::io::Result<()> {
    use std::io::Write;
    let parent = out_path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".passport-capsule.")
        .suffix(".tmp")
        .tempfile_in(parent)?;
    let body = serde_json::to_vec_pretty(value)
        .map_err(|e| std::io::Error::other(format!("serialise capsule: {e}")))?;
    tmp.as_file_mut().write_all(&body)?;
    tmp.as_file_mut().sync_all()?;
    tmp.persist(out_path).map_err(|e| e.error)?;
    Ok(())
}

#[allow(dead_code)]
fn _silence_capsule_unused(_e: &CapsuleVerifyError) {
    // CapsuleVerifyError is re-exported via `verify_capsule` use; this
    // sentinel keeps clippy quiet if the verifier ever returns Ok-only.
}
