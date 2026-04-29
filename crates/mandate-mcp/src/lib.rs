//! Mandate MCP — stdio JSON-RPC server (Passport P3.1).
//!
//! This crate exposes Mandate's policy + capsule + audit primitives as
//! JSON-RPC 2.0 tools over stdio. Six tools today; every one of them
//! **wraps** an existing primitive and adds *no* new business logic:
//!
//! | Tool | Wraps |
//! | --- | --- |
//! | `mandate.validate_aprp` | `mandate_core::schema::validate_aprp` |
//! | `mandate.decide` | `mandate_server::router` (oneshot pattern) |
//! | `mandate.run_guarded_execution` | `mandate_server` + `KeeperHubExecutor::local_mock` / `UniswapExecutor::local_mock` |
//! | `mandate.verify_capsule` | `mandate_core::passport::verify_capsule` |
//! | `mandate.explain_denial` | `mandate_core::passport::verify_capsule` + structured projection |
//! | `mandate.audit_lookup` | `Storage::audit_chain_prefix_through` + `mandate_core::audit_bundle::build` (IP-3) |
//!
//! Plus one meta method `tools/list` that returns the catalogue with
//! input/output schemas.
//!
//! Wire format. Requests and responses are line-delimited JSON
//! (one JSON-RPC 2.0 envelope per line). MCP's normal Content-Length
//! framing is not required for the local-stdio use cases this surface
//! targets (test harnesses, demo scripts, MCP-aware clients that accept
//! NDJSON). The Rust MCP SDK is too churn-heavy for a hackathon
//! deliverable; per the P3.1 backlog risk note we implement a minimal
//! protocol and document it (see `docs/cli/mcp.md`).
//!
//! No daemon. Each tool call opens a fresh `Storage` handle against the
//! supplied `db` path, drives the request, and drops the handle. SQLite
//! WAL mode tolerates the sequential opens; the server itself is
//! stateless.
//!
//! Live mode is rejected. The IP-3 catalogue alignment intentionally
//! does not include a "submit to a real KeeperHub backend" path — that
//! lives in P5.1 / P6.1 with concrete credentials + `live_evidence`.
//! `mandate.run_guarded_execution` accepts only `mode: "mock"`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use mandate_core::audit_bundle::{self, BundleError};
use mandate_core::error::SchemaError;
use mandate_core::passport::{verify_capsule, CapsuleVerifyError};
use mandate_core::receipt::PolicyReceipt;
use mandate_core::schema::{validate_aprp, validate_passport_capsule};
use mandate_execution::keeperhub::KeeperHubExecutor;
use mandate_execution::uniswap::UniswapExecutor;
use mandate_execution::GuardedExecutor;
use mandate_policy::Policy;
use mandate_server::{AppState, PaymentRequestResponse, PaymentStatus};
use mandate_storage::Storage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub mod jsonrpc;

pub use jsonrpc::{ErrorObject, Request, Response};

/// JSON-RPC error code namespace for tool-level failures (per JSON-RPC
/// 2.0 §5.1: `-32000..-32099` is reserved for server-defined errors).
/// We use a single base code and a stable string `data.code` to
/// distinguish kinds — that lets new failure modes land without
/// burning JSON-RPC code points.
pub const TOOL_ERROR_CODE: i64 = -32000;

/// Stable, machine-readable error codes attached to the JSON-RPC
/// `error.data.code` field. Tests assert on these — they are part of
/// the wire contract.
pub mod error_codes {
    pub const PARAMS_INVALID: &str = "params_invalid";
    pub const SCHEMA_VIOLATION: &str = "schema_violation";
    pub const APRP_INVALID: &str = "aprp_invalid";
    pub const POLICY_LOAD_FAILED: &str = "policy_load_failed";
    pub const POLICY_NOT_ACTIVE: &str = "policy_not_active";
    pub const PIPELINE_FAILED: &str = "pipeline_failed";
    pub const EXECUTOR_FAILED: &str = "executor_failed";
    pub const REQUIRES_HUMAN: &str = "requires_human_unsupported";
    pub const LIVE_MODE_REJECTED: &str = "live_mode_rejected";
    pub const CAPSULE_IO_FAILED: &str = "capsule_io_failed";
    pub const CAPSULE_INVALID: &str = "capsule_invalid";
    pub const CAPSULE_NOT_DENY: &str = "capsule_not_deny";
    pub const AUDIT_EVENT_NOT_FOUND: &str = "audit_event_not_found";
    pub const AUDIT_EVENT_ID_MISMATCH: &str = "audit_event_id_mismatch";
    pub const BUNDLE_BUILD_FAILED: &str = "bundle_build_failed";
    pub const STORAGE_FAILED: &str = "storage_failed";
    /// Round 0 audit fix: the supplied path resolves outside the
    /// `MANDATE_MCP_ROOT` sandbox (or its symlink-resolved canonical
    /// form does). Stable wire string is the dotted form to match the
    /// capsule-verifier `(capsule.<code>)` discriminator family — MCP
    /// clients can branch on it the same way they already branch on
    /// verifier codes.
    pub const PATH_ESCAPE: &str = "capsule.path_escape";
}

/// Tool dispatch context. Held by the binary and passed into each
/// `dispatch` call.
///
/// `root` constrains every filesystem path argument the dispatcher
/// accepts (db paths, capsule paths). When `None`, the dispatcher
/// reads `MANDATE_MCP_ROOT` from the environment, falling back to the
/// process working directory. Setting this field programmatically is
/// the test-friendly path: tests construct
/// `ServerContext::with_root(tempdir)` so they don't race on the
/// process-global env var.
#[derive(Default, Clone)]
pub struct ServerContext {
    pub root: Option<PathBuf>,
}

impl ServerContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a context with an explicit sandbox root. Used by tests
    /// to supply a tempdir without writing to the process-global
    /// `MANDATE_MCP_ROOT` env var.
    pub fn with_root(root: PathBuf) -> Self {
        Self { root: Some(root) }
    }

    /// Resolve the effective sandbox root. Precedence:
    ///   1. `self.root` if set (test-time programmatic override).
    ///   2. `MANDATE_MCP_ROOT` env var (operator-time configuration).
    ///   3. process working directory.
    pub fn effective_root(&self) -> Result<PathBuf, ToolError> {
        if let Some(p) = &self.root {
            return Ok(p.clone());
        }
        if let Ok(s) = std::env::var("MANDATE_MCP_ROOT") {
            if !s.is_empty() {
                return Ok(PathBuf::from(s));
            }
        }
        std::env::current_dir().map_err(|e| {
            ToolError::new(
                error_codes::PATH_ESCAPE,
                format!("MANDATE_MCP_ROOT unset and current_dir() failed: {e}"),
            )
        })
    }
}

/// Canonicalize `path` and assert it lives under `root`. Returns the
/// canonical path on success; `path_escape` error on any escape.
///
/// The check follows symlinks (via `Path::canonicalize`) so a symlink
/// pointing outside the root is treated as escape, not as an in-root
/// path. For paths that don't yet exist on disk (e.g. a fresh SQLite
/// DB filename that `Storage::open` will create on first use), the
/// parent directory is canonicalized instead and the filename is
/// reattached — this still resolves any symlink in the parent chain,
/// which is where escape attacks would land.
///
/// Relative paths resolve against the **process working directory**
/// (matching standard filesystem semantics), then the canonicalised
/// result is checked for containment in `root`. This means a relative
/// path like `..` is resolved against cwd, may end up outside `root`,
/// and is rejected.
pub fn canonicalize_within_root(path: &Path, root: &Path) -> Result<PathBuf, ToolError> {
    let canonical_root = root.canonicalize().map_err(|e| {
        ToolError::new(
            error_codes::PATH_ESCAPE,
            format!(
                "MANDATE_MCP_ROOT {} could not be canonicalized: {e}",
                root.display()
            ),
        )
    })?;
    let target = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| {
                ToolError::new(
                    error_codes::PATH_ESCAPE,
                    format!("could not read process current_dir: {e}"),
                )
            })?
            .join(path)
    };
    let canonical_path = match target.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            let parent = target.parent().ok_or_else(|| {
                ToolError::new(
                    error_codes::PATH_ESCAPE,
                    format!("path {} has no parent component", target.display()),
                )
            })?;
            let file_name = target.file_name().ok_or_else(|| {
                ToolError::new(
                    error_codes::PATH_ESCAPE,
                    format!("path {} has no file component", target.display()),
                )
            })?;
            let canonical_parent = parent.canonicalize().map_err(|e| {
                ToolError::new(
                    error_codes::PATH_ESCAPE,
                    format!(
                        "path parent {} could not be canonicalized: {e}",
                        parent.display()
                    ),
                )
            })?;
            canonical_parent.join(file_name)
        }
    };
    if !canonical_path.starts_with(&canonical_root) {
        return Err(ToolError::new(
            error_codes::PATH_ESCAPE,
            format!(
                "path {} escapes MANDATE_MCP_ROOT {}",
                canonical_path.display(),
                canonical_root.display()
            ),
        ));
    }
    Ok(canonical_path)
}

/// One tool call's structured failure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolError {
    pub code: String,
    pub message: String,
}

impl ToolError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }

    pub fn into_jsonrpc(self) -> ErrorObject {
        ErrorObject {
            code: TOOL_ERROR_CODE,
            message: self.message.clone(),
            data: Some(json!({ "code": self.code })),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool catalogue
// ---------------------------------------------------------------------------

/// Static description of one MCP tool. Returned by the `tools/list`
/// meta-method.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDescriptor {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub output_schema: Value,
}

pub fn tools_catalogue() -> Vec<ToolDescriptor> {
    vec![
        ToolDescriptor {
            name: "mandate.validate_aprp",
            description: "Validate an APRP body against schemas/aprp_v1.json. Returns ok=true on \
                 success; on failure returns a tool error with code=aprp_invalid.",
            input_schema: json!({
                "type": "object",
                "required": ["aprp"],
                "properties": {
                    "aprp": { "type": "object" }
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "ok":           { "const": true },
                    "request_hash": { "type": "string", "pattern": "^[0-9a-f]{64}$" }
                },
                "required": ["ok", "request_hash"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "mandate.decide",
            description:
                "Drive the offline payment-requests pipeline (APRP → policy → budget → audit \
                 → signed receipt) in-process. Returns the same PaymentRequestResponse the \
                 HTTP API would. Requires an active policy in the DB.",
            input_schema: json!({
                "type": "object",
                "required": ["aprp", "db"],
                "properties": {
                    "aprp": { "type": "object" },
                    "db":   { "type": "string", "description": "filesystem path to a Mandate SQLite DB" }
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "description": "mandate-server PaymentRequestResponse",
                "additionalProperties": true
            }),
        },
        ToolDescriptor {
            name: "mandate.run_guarded_execution",
            description:
                "Run mandate.decide + (allow path) the chosen mock executor (KeeperHub or \
                 Uniswap). Returns the receipt + execution block. mode=live is rejected with \
                 code=live_mode_rejected.",
            input_schema: json!({
                "type": "object",
                "required": ["aprp", "db", "executor"],
                "properties": {
                    "aprp":     { "type": "object" },
                    "db":       { "type": "string" },
                    "executor": { "enum": ["keeperhub", "uniswap"] },
                    "mode":     { "enum": ["mock"], "default": "mock" }
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "decision":         { "type": "object" },
                    "execution":        { "type": "object" },
                    "audit_event_id":   { "type": "string" }
                },
                "required": ["decision", "execution", "audit_event_id"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "mandate.verify_capsule",
            description: "Run the P1.1 verifier on a capsule (schema + 8 cross-field invariants). \
                 Accepts either an inline `capsule` object or a `path` to read from disk.",
            input_schema: json!({
                "type": "object",
                "oneOf": [
                    { "required": ["capsule"] },
                    { "required": ["path"] }
                ],
                "properties": {
                    "capsule": { "type": "object" },
                    "path":    { "type": "string" }
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "ok":     { "const": true },
                    "schema": { "const": "mandate.passport_capsule.v1" }
                },
                "required": ["ok", "schema"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "mandate.explain_denial",
            description: "Read + verify a capsule and return a deny-only structured explanation \
                 (matched_rule, deny_code, audit_event_id). Returns code=capsule_not_deny if \
                 the capsule is an allow.",
            input_schema: json!({
                "type": "object",
                "oneOf": [
                    { "required": ["capsule"] },
                    { "required": ["path"] }
                ],
                "properties": {
                    "capsule": { "type": "object" },
                    "path":    { "type": "string" }
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "schema":         { "const": "mandate.passport_capsule.v1" },
                    "decision":       { "type": "object" },
                    "audit":          { "type": "object" },
                    "policy":         { "type": "object" }
                },
                "required": ["schema", "decision", "audit", "policy"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "mandate.audit_lookup",
            description:
                "IP-3 sister tool. Given a Mandate audit_event_id + a signed PolicyReceipt + \
                 the DB path + signer pubkeys, returns the corresponding mandate.audit_bundle.v1. \
                 Calls `Storage::audit_chain_prefix_through` and `audit_bundle::build` — no \
                 storage changes vs. main. See docs/keeperhub-integration-paths.md §IP-3.",
            input_schema: json!({
                "type": "object",
                "required": ["audit_event_id", "db", "receipt", "receipt_pubkey", "audit_pubkey"],
                "properties": {
                    "audit_event_id": { "type": "string", "pattern": "^evt-[0-9A-Z]{26}$" },
                    "db":             { "type": "string" },
                    "receipt":        { "type": "object" },
                    "receipt_pubkey": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
                    "audit_pubkey":   { "type": "string", "pattern": "^[0-9a-f]{64}$" }
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "ok":     { "const": true },
                    "bundle": { "type": "object", "description": "mandate.audit_bundle.v1" }
                },
                "required": ["ok", "bundle"],
                "additionalProperties": false
            }),
        },
    ]
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Synchronous dispatch entrypoint. Returns the JSON-RPC `result` value
/// on success, or a structured `ToolError` on failure. The binary
/// translates `ToolError` into a JSON-RPC error envelope.
///
/// Internally, methods that need to drive the axum router (which is
/// async) build a single-thread tokio runtime and `block_on` it. This
/// matches the in-process pattern `mandate passport run` already uses
/// (`crates/mandate-cli/src/passport.rs::cmd_run` step 6).
pub fn dispatch(method: &str, params: &Value, ctx: &ServerContext) -> Result<Value, ToolError> {
    match method {
        "tools/list" => Ok(serde_json::to_value(tools_catalogue()).map_err(|e| {
            ToolError::new(
                error_codes::PARAMS_INVALID,
                format!("serialise catalogue: {e}"),
            )
        })?),
        "mandate.validate_aprp" => tool_validate_aprp(params),
        "mandate.decide" => tool_decide(params, ctx),
        "mandate.run_guarded_execution" => tool_run_guarded(params, ctx),
        "mandate.verify_capsule" => tool_verify_capsule(params, ctx),
        "mandate.explain_denial" => tool_explain_denial(params, ctx),
        "mandate.audit_lookup" => tool_audit_lookup(params, ctx),
        _ => Err(ToolError::new(
            error_codes::PARAMS_INVALID,
            format!("unknown method: {method}"),
        )),
    }
}

// ---------------------------------------------------------------------------
// Per-tool implementations
// ---------------------------------------------------------------------------

fn tool_validate_aprp(params: &Value) -> Result<Value, ToolError> {
    let aprp = params
        .get("aprp")
        .ok_or_else(|| ToolError::new(error_codes::PARAMS_INVALID, "missing field `aprp`"))?;
    if let Err(e) = validate_aprp(aprp) {
        return Err(ToolError::new(
            error_codes::APRP_INVALID,
            schema_error_message(e),
        ));
    }
    let bytes = mandate_core::hashing::canonical_json(aprp).map_err(|e| {
        ToolError::new(
            error_codes::APRP_INVALID,
            format!("APRP is schema-valid but could not be canonicalised: {e}"),
        )
    })?;
    let request_hash = mandate_core::hashing::sha256_hex(&bytes);
    Ok(json!({ "ok": true, "request_hash": request_hash }))
}

fn tool_decide(params: &Value, ctx: &ServerContext) -> Result<Value, ToolError> {
    let aprp = params
        .get("aprp")
        .ok_or_else(|| ToolError::new(error_codes::PARAMS_INVALID, "missing field `aprp`"))?
        .clone();
    let db_path = params
        .get("db")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::new(error_codes::PARAMS_INVALID, "missing field `db`"))?;
    let safe_db = canonicalize_within_root(Path::new(db_path), &ctx.effective_root()?)?;

    let policy = load_active_policy(&safe_db)?;
    let response = run_pipeline(&safe_db, policy, &aprp)?;
    serde_json::to_value(&response).map_err(|e| {
        ToolError::new(
            error_codes::PIPELINE_FAILED,
            format!("serialise response: {e}"),
        )
    })
}

fn tool_run_guarded(params: &Value, ctx: &ServerContext) -> Result<Value, ToolError> {
    let aprp = params
        .get("aprp")
        .ok_or_else(|| ToolError::new(error_codes::PARAMS_INVALID, "missing field `aprp`"))?
        .clone();
    let db_path = params
        .get("db")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::new(error_codes::PARAMS_INVALID, "missing field `db`"))?;
    let executor = params
        .get("executor")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::new(error_codes::PARAMS_INVALID, "missing field `executor`"))?;
    let mode = params
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("mock");

    if mode != "mock" {
        return Err(ToolError::new(
            error_codes::LIVE_MODE_REJECTED,
            "mandate.run_guarded_execution only supports mode=mock in P3.1; live integration \
             lands with P5.1/P6.1.",
        ));
    }
    let exec_choice = match executor {
        "keeperhub" => ExecutorChoice::Keeperhub,
        "uniswap" => ExecutorChoice::Uniswap,
        other => {
            return Err(ToolError::new(
                error_codes::PARAMS_INVALID,
                format!("unknown executor: {other} (expected keeperhub|uniswap)"),
            ));
        }
    };

    let safe_db = canonicalize_within_root(Path::new(db_path), &ctx.effective_root()?)?;
    let policy = load_active_policy(&safe_db)?;
    let response = run_pipeline(&safe_db, policy, &aprp)?;

    // Truthfulness rule from passport run: deny path never calls executor.
    if matches!(
        response.receipt.decision,
        mandate_core::receipt::Decision::RequiresHuman
    ) {
        return Err(ToolError::new(
            error_codes::REQUIRES_HUMAN,
            "policy returned requires_human; mandate.run_guarded_execution does not encode that \
             outcome (mandate.passport_capsule.v1 only allows allow/deny). Use the regular API \
             surface for human-review workflows.",
        ));
    }
    let allow_path = matches!(response.status, PaymentStatus::AutoApproved);
    let exec_block = if allow_path {
        match call_mock_executor(exec_choice, &aprp, &response) {
            Ok(block) => block,
            Err(msg) => {
                return Err(ToolError::new(error_codes::EXECUTOR_FAILED, msg));
            }
        }
    } else {
        deny_execution_block(exec_choice)
    };

    let receipt_value = serde_json::to_value(&response.receipt).map_err(|e| {
        ToolError::new(
            error_codes::PIPELINE_FAILED,
            format!("serialise receipt: {e}"),
        )
    })?;
    let decision_block = json!({
        "result": match response.receipt.decision {
            mandate_core::receipt::Decision::Allow => "allow",
            mandate_core::receipt::Decision::Deny => "deny",
            mandate_core::receipt::Decision::RequiresHuman => unreachable!("rejected above"),
        },
        "matched_rule": response.matched_rule_id,
        "deny_code":    response.deny_code,
        "request_hash": response.request_hash,
        "policy_hash":  response.policy_hash,
        "receipt":      receipt_value,
    });
    Ok(json!({
        "decision": decision_block,
        "execution": Value::Object(exec_block),
        "audit_event_id": response.audit_event_id,
    }))
}

fn tool_verify_capsule(params: &Value, ctx: &ServerContext) -> Result<Value, ToolError> {
    let capsule = load_capsule_param(params, ctx)?;
    if let Err(e) = validate_passport_capsule(&capsule) {
        return Err(ToolError::new(
            error_codes::SCHEMA_VIOLATION,
            schema_error_message(e),
        ));
    }
    if let Err(e) = verify_capsule(&capsule) {
        return Err(ToolError::new(
            capsule_error_code(&e),
            format!("{e} ({})", e.code()),
        ));
    }
    Ok(json!({ "ok": true, "schema": "mandate.passport_capsule.v1" }))
}

fn tool_explain_denial(params: &Value, ctx: &ServerContext) -> Result<Value, ToolError> {
    let capsule = load_capsule_param(params, ctx)?;
    if let Err(e) = verify_capsule(&capsule) {
        return Err(ToolError::new(
            capsule_error_code(&e),
            format!("{e} ({})", e.code()),
        ));
    }
    let result = capsule
        .pointer("/decision/result")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    if result != "deny" {
        return Err(ToolError::new(
            error_codes::CAPSULE_NOT_DENY,
            format!(
                "mandate.explain_denial only operates on deny capsules; this capsule is `{result}`"
            ),
        ));
    }
    let projection = json!({
        "schema": "mandate.passport_capsule.v1",
        "decision": {
            "result": "deny",
            "matched_rule": capsule.pointer("/decision/matched_rule").cloned().unwrap_or(Value::Null),
            "deny_code":    capsule.pointer("/decision/deny_code").cloned().unwrap_or(Value::Null),
        },
        "audit": {
            "audit_event_id": capsule.pointer("/audit/audit_event_id").cloned().unwrap_or(Value::Null),
        },
        "policy": {
            "policy_hash":    capsule.pointer("/policy/policy_hash").cloned().unwrap_or(Value::Null),
            "policy_version": capsule.pointer("/policy/policy_version").cloned().unwrap_or(Value::Null),
        },
    });
    Ok(projection)
}

fn tool_audit_lookup(params: &Value, ctx: &ServerContext) -> Result<Value, ToolError> {
    let event_id = params
        .get("audit_event_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ToolError::new(
                error_codes::PARAMS_INVALID,
                "missing field `audit_event_id`",
            )
        })?;
    let db_path = params
        .get("db")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::new(error_codes::PARAMS_INVALID, "missing field `db`"))?;
    let safe_db = canonicalize_within_root(Path::new(db_path), &ctx.effective_root()?)?;
    let receipt_value = params
        .get("receipt")
        .ok_or_else(|| ToolError::new(error_codes::PARAMS_INVALID, "missing field `receipt`"))?;
    let receipt_pubkey = params
        .get("receipt_pubkey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ToolError::new(
                error_codes::PARAMS_INVALID,
                "missing field `receipt_pubkey`",
            )
        })?
        .to_string();
    let audit_pubkey = params
        .get("audit_pubkey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::new(error_codes::PARAMS_INVALID, "missing field `audit_pubkey`"))?
        .to_string();

    let receipt: PolicyReceipt = serde_json::from_value(receipt_value.clone()).map_err(|e| {
        ToolError::new(
            error_codes::PARAMS_INVALID,
            format!("receipt does not deserialise as PolicyReceipt: {e}"),
        )
    })?;
    if receipt.audit_event_id != event_id {
        return Err(ToolError::new(
            error_codes::AUDIT_EVENT_ID_MISMATCH,
            format!(
                "receipt.audit_event_id={} but request asked for audit_event_id={event_id}",
                receipt.audit_event_id
            ),
        ));
    }
    let storage = Storage::open(&safe_db).map_err(|e| {
        ToolError::new(
            error_codes::STORAGE_FAILED,
            format!("open db {}: {e}", safe_db.display()),
        )
    })?;
    let chain = match storage.audit_chain_prefix_through(event_id) {
        Ok(c) => c,
        Err(mandate_storage::error::StorageError::AuditEventNotFound { id }) => {
            return Err(ToolError::new(
                error_codes::AUDIT_EVENT_NOT_FOUND,
                format!("audit_event_id {id} not present in {}", safe_db.display()),
            ));
        }
        Err(e) => {
            return Err(ToolError::new(
                error_codes::STORAGE_FAILED,
                format!("audit_chain_prefix_through: {e}"),
            ));
        }
    };
    let bundle = audit_bundle::build(
        receipt,
        chain,
        receipt_pubkey,
        audit_pubkey,
        chrono::Utc::now(),
    )
    .map_err(|e| ToolError::new(error_codes::BUNDLE_BUILD_FAILED, bundle_error_message(e)))?;
    let serialised = serde_json::to_value(&bundle).map_err(|e| {
        ToolError::new(
            error_codes::BUNDLE_BUILD_FAILED,
            format!("serialise bundle: {e}"),
        )
    })?;
    Ok(json!({ "ok": true, "bundle": serialised }))
}

// ---------------------------------------------------------------------------
// Internals shared between tools
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum ExecutorChoice {
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

fn load_active_policy(db_path: &Path) -> Result<Policy, ToolError> {
    let storage = Storage::open(db_path).map_err(|e| {
        ToolError::new(
            error_codes::STORAGE_FAILED,
            format!("open db {}: {e}", db_path.display()),
        )
    })?;
    let active = storage
        .policy_current()
        .map_err(|e| ToolError::new(error_codes::STORAGE_FAILED, format!("policy_current: {e}")))?
        .ok_or_else(|| {
            ToolError::new(
                error_codes::POLICY_NOT_ACTIVE,
                format!(
                    "no active policy in {}; run `mandate policy activate` first",
                    db_path.display()
                ),
            )
        })?;
    Policy::parse_json(&active.policy_json).map_err(|e| {
        ToolError::new(
            error_codes::POLICY_LOAD_FAILED,
            format!("active policy v{} no longer validates: {e}", active.version),
        )
    })
}

fn run_pipeline(
    db_path: &Path,
    policy: Policy,
    aprp: &Value,
) -> Result<PaymentRequestResponse, ToolError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| {
            ToolError::new(
                error_codes::PIPELINE_FAILED,
                format!("tokio runtime init: {e}"),
            )
        })?;
    let aprp_owned = aprp.clone();
    let db_owned = db_path.to_path_buf();
    runtime.block_on(async move {
        let storage = Storage::open(&db_owned).map_err(|e| {
            ToolError::new(
                error_codes::STORAGE_FAILED,
                format!("open db {}: {e}", db_owned.display()),
            )
        })?;
        let state = AppState::new(policy, storage);
        let app = mandate_server::router(state);
        oneshot_payment_request(app, &aprp_owned).await
    })
}

async fn oneshot_payment_request(
    app: axum::Router,
    aprp: &Value,
) -> Result<PaymentRequestResponse, ToolError> {
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    let body = serde_json::to_vec(aprp).map_err(|e| {
        ToolError::new(error_codes::PIPELINE_FAILED, format!("serialise aprp: {e}"))
    })?;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/payment-requests")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .map_err(|e| ToolError::new(error_codes::PIPELINE_FAILED, format!("build request: {e}")))?;
    let resp = app
        .oneshot(req)
        .await
        .map_err(|e| ToolError::new(error_codes::PIPELINE_FAILED, format!("oneshot: {e}")))?;
    let status = resp.status();
    let body_bytes = resp
        .into_body()
        .collect()
        .await
        .map_err(|e| {
            ToolError::new(
                error_codes::PIPELINE_FAILED,
                format!("read response body: {e}"),
            )
        })?
        .to_bytes();
    if !status.is_success() {
        let body: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);
        // Map the schema/protocol/policy/budget Problem codes back to a
        // useful tool error. The Problem body's `code` field is the
        // canonical machine-readable error string; surface it verbatim
        // through `data.code` so MCP clients branch on the same key as
        // the HTTP API.
        let code = body
            .get("code")
            .and_then(|v| v.as_str())
            .unwrap_or(error_codes::PIPELINE_FAILED)
            .to_string();
        return Err(ToolError::new(&code, format!("HTTP {status}: {body}")));
    }
    serde_json::from_slice(&body_bytes)
        .map_err(|e| ToolError::new(error_codes::PIPELINE_FAILED, format!("parse response: {e}")))
}

fn call_mock_executor(
    choice: ExecutorChoice,
    aprp: &Value,
    response: &PaymentRequestResponse,
) -> Result<Map<String, Value>, String> {
    let request: mandate_core::aprp::PaymentRequest =
        serde_json::from_value(aprp.clone()).map_err(|e| format!("aprp typed parse: {e}"))?;
    let executor: Box<dyn GuardedExecutor> = match choice {
        ExecutorChoice::Keeperhub => Box::new(KeeperHubExecutor::local_mock()),
        ExecutorChoice::Uniswap => Box::new(UniswapExecutor::local_mock()),
    };
    let exec_receipt = executor
        .execute(&request, &response.receipt)
        .map_err(|e| format!("executor: {e}"))?;
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

fn load_capsule_param(params: &Value, ctx: &ServerContext) -> Result<Value, ToolError> {
    if let Some(v) = params.get("capsule") {
        return Ok(v.clone());
    }
    if let Some(p) = params.get("path").and_then(|v| v.as_str()) {
        let safe = canonicalize_within_root(Path::new(p), &ctx.effective_root()?)?;
        let raw = std::fs::read_to_string(&safe).map_err(|e| {
            ToolError::new(
                error_codes::CAPSULE_IO_FAILED,
                format!("read {}: {e}", safe.display()),
            )
        })?;
        return serde_json::from_str::<Value>(&raw).map_err(|e| {
            ToolError::new(
                error_codes::CAPSULE_INVALID,
                format!("parse {}: {e}", safe.display()),
            )
        });
    }
    Err(ToolError::new(
        error_codes::PARAMS_INVALID,
        "expected one of fields `capsule` (object) or `path` (string)",
    ))
}

fn schema_error_message(err: SchemaError) -> String {
    err.to_string()
}

fn bundle_error_message(err: BundleError) -> String {
    err.to_string()
}

fn capsule_error_code(_err: &CapsuleVerifyError) -> &'static str {
    // All P1.1 verifier failures collapse into one tool-level code today;
    // the verifier's own `(capsule.<code>)` discriminator is preserved
    // verbatim in the error message so MCP clients can branch on it
    // exactly the way `mandate passport verify` callers do.
    error_codes::CAPSULE_INVALID
}

// ---------------------------------------------------------------------------
// Public re-exports for tests and the binary
// ---------------------------------------------------------------------------

/// Convenience: dispatch into a JSON-RPC `Response`. Used by the binary
/// and by integration tests that drive the dispatcher without spawning
/// a child process.
pub fn dispatch_to_response(req: &Request, ctx: &Arc<ServerContext>) -> Response {
    match dispatch(&req.method, &req.params, ctx) {
        Ok(result) => Response::ok(req.id.clone(), result),
        Err(e) => Response::err(req.id.clone(), e.into_jsonrpc()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_catalogue_lists_six_tools_plus_meta() {
        let catalogue = tools_catalogue();
        assert_eq!(catalogue.len(), 6, "P3.1 ships six tools");
        let names: Vec<&str> = catalogue.iter().map(|t| t.name).collect();
        for name in [
            "mandate.validate_aprp",
            "mandate.decide",
            "mandate.run_guarded_execution",
            "mandate.verify_capsule",
            "mandate.explain_denial",
            "mandate.audit_lookup",
        ] {
            assert!(names.contains(&name), "missing tool `{name}` in catalogue");
        }
    }

    #[test]
    fn unknown_method_returns_params_invalid() {
        let ctx = ServerContext::new();
        let err = dispatch("does.not.exist", &json!({}), &ctx).unwrap_err();
        assert_eq!(err.code, error_codes::PARAMS_INVALID);
    }
}
