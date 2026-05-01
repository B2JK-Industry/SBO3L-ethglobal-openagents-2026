//! JSON Schema embedding and validation. Schema files live under `/schemas/` at the workspace root.

use std::sync::OnceLock;

use jsonschema::{JSONSchema, ValidationError};
use serde_json::Value;

use crate::error::SchemaError;

// F-11: schemas vendored into the crate (`crates/sbo3l-core/schemas/`)
// so `cargo publish` can package them. The workspace-root `schemas/`
// directory remains the canonical authoring location; CI's
// `python3 scripts/validate_schemas.py` enforces byte-for-byte
// equality between the workspace copy and each vendored copy
// (sync check). When you edit a workspace schema, also re-copy it
// into every `crates/<name>/schemas/` directory that vendors it.
pub const APRP_SCHEMA_JSON: &str = include_str!("../schemas/aprp_v1.json");
pub const POLICY_SCHEMA_JSON: &str = include_str!("../schemas/policy_v1.json");
pub const X402_SCHEMA_JSON: &str = include_str!("../schemas/x402_v1.json");
pub const POLICY_RECEIPT_SCHEMA_JSON: &str = include_str!("../schemas/policy_receipt_v1.json");
pub const DECISION_TOKEN_SCHEMA_JSON: &str = include_str!("../schemas/decision_token_v1.json");
pub const AUDIT_EVENT_SCHEMA_JSON: &str = include_str!("../schemas/audit_event_v1.json");
pub const PASSPORT_CAPSULE_SCHEMA_JSON: &str =
    include_str!("../schemas/sbo3l.passport_capsule.v1.json");
pub const PASSPORT_CAPSULE_V2_SCHEMA_JSON: &str =
    include_str!("../schemas/sbo3l.passport_capsule.v2.json");

pub const APRP_SCHEMA_ID: &str = "https://schemas.sbo3l.dev/aprp/v1.json";
pub const POLICY_SCHEMA_ID: &str = "https://schemas.sbo3l.dev/policy/v1.json";
pub const X402_SCHEMA_ID: &str = "https://schemas.sbo3l.dev/x402/v1.json";
pub const POLICY_RECEIPT_SCHEMA_ID: &str = "https://schemas.sbo3l.dev/policy-receipt/v1.json";
pub const DECISION_TOKEN_SCHEMA_ID: &str = "https://schemas.sbo3l.dev/decision-token/v1.json";
pub const AUDIT_EVENT_SCHEMA_ID: &str = "https://schemas.sbo3l.dev/audit-event/v1.json";
pub const PASSPORT_CAPSULE_SCHEMA_ID: &str = "https://schemas.sbo3l.dev/passport-capsule/v1.json";
pub const PASSPORT_CAPSULE_V2_SCHEMA_ID: &str =
    "https://schemas.sbo3l.dev/passport-capsule/v2.json";

fn parse(schema: &str) -> Value {
    serde_json::from_str(schema).expect("invariant: embedded schema parses")
}

fn build_with_refs(main: Value, refs: &[(&str, Value)]) -> JSONSchema {
    let mut options = JSONSchema::options();
    for (id, doc) in refs {
        options.with_document((*id).to_string(), doc.clone());
    }
    options
        .compile(&main)
        .expect("invariant: embedded schema compiles")
}

fn aprp_schema() -> &'static JSONSchema {
    static CELL: OnceLock<JSONSchema> = OnceLock::new();
    CELL.get_or_init(|| {
        build_with_refs(
            parse(APRP_SCHEMA_JSON),
            &[(X402_SCHEMA_ID, parse(X402_SCHEMA_JSON))],
        )
    })
}

fn policy_schema() -> &'static JSONSchema {
    static CELL: OnceLock<JSONSchema> = OnceLock::new();
    CELL.get_or_init(|| build_with_refs(parse(POLICY_SCHEMA_JSON), &[]))
}

fn policy_receipt_schema() -> &'static JSONSchema {
    static CELL: OnceLock<JSONSchema> = OnceLock::new();
    CELL.get_or_init(|| build_with_refs(parse(POLICY_RECEIPT_SCHEMA_JSON), &[]))
}

fn decision_token_schema() -> &'static JSONSchema {
    static CELL: OnceLock<JSONSchema> = OnceLock::new();
    CELL.get_or_init(|| build_with_refs(parse(DECISION_TOKEN_SCHEMA_JSON), &[]))
}

fn audit_event_schema() -> &'static JSONSchema {
    static CELL: OnceLock<JSONSchema> = OnceLock::new();
    CELL.get_or_init(|| build_with_refs(parse(AUDIT_EVENT_SCHEMA_JSON), &[]))
}

fn passport_capsule_schema() -> &'static JSONSchema {
    static CELL: OnceLock<JSONSchema> = OnceLock::new();
    CELL.get_or_init(|| build_with_refs(parse(PASSPORT_CAPSULE_SCHEMA_JSON), &[]))
}

fn passport_capsule_v2_schema() -> &'static JSONSchema {
    static CELL: OnceLock<JSONSchema> = OnceLock::new();
    CELL.get_or_init(|| build_with_refs(parse(PASSPORT_CAPSULE_V2_SCHEMA_JSON), &[]))
}

pub fn validate_aprp(value: &Value) -> std::result::Result<(), SchemaError> {
    validate(aprp_schema(), value)
}

pub fn validate_policy(value: &Value) -> std::result::Result<(), SchemaError> {
    validate(policy_schema(), value)
}

pub fn validate_policy_receipt(value: &Value) -> std::result::Result<(), SchemaError> {
    validate(policy_receipt_schema(), value)
}

pub fn validate_decision_token(value: &Value) -> std::result::Result<(), SchemaError> {
    validate(decision_token_schema(), value)
}

pub fn validate_audit_event(value: &Value) -> std::result::Result<(), SchemaError> {
    validate(audit_event_schema(), value)
}

/// Validate a passport capsule JSON document against the embedded
/// schema. Dispatches on the `schema` field — `sbo3l.passport_capsule.v1`
/// goes to the v1 schema; `sbo3l.passport_capsule.v2` goes to the v2
/// schema (additive: same shape plus optional `policy.policy_snapshot`
/// and `audit.audit_segment`). Any other value (or missing field) routes
/// to v1 for backwards compat with callers that pre-date the v2 bump;
/// the v1 schema itself enforces `schema: const "sbo3l.passport_capsule.v1"`
/// so a malformed marker still surfaces as a schema error there.
///
/// This is *purely structural* (shape, required fields, hex/UUID-ish
/// patterns, `additionalProperties: false`). Cross-field truthfulness
/// invariants (deny→no execution, live→evidence, hash internal-
/// consistency) live in `crate::passport::verify_capsule`.
pub fn validate_passport_capsule(value: &Value) -> std::result::Result<(), SchemaError> {
    let version = value.get("schema").and_then(|v| v.as_str()).unwrap_or("");
    match version {
        "sbo3l.passport_capsule.v2" => validate(passport_capsule_v2_schema(), value),
        _ => validate(passport_capsule_schema(), value),
    }
}

fn validate(schema: &JSONSchema, value: &Value) -> std::result::Result<(), SchemaError> {
    let result = schema.validate(value);
    if let Err(errors) = result {
        if let Some(first) = errors.into_iter().next() {
            return Err(map_error(first));
        }
    }
    Ok(())
}

fn map_error(err: ValidationError<'_>) -> SchemaError {
    use jsonschema::error::ValidationErrorKind;
    let path = err.instance_path.to_string();
    match err.kind {
        ValidationErrorKind::AdditionalProperties { unexpected } => {
            let field = unexpected.first().cloned().unwrap_or_default();
            let p = if path.is_empty() {
                format!("/{field}")
            } else {
                format!("{path}/{field}")
            };
            SchemaError::UnknownField { path: p }
        }
        ValidationErrorKind::Required { property } => SchemaError::MissingField {
            field: property.to_string().trim_matches('"').to_string(),
        },
        ValidationErrorKind::Type { .. } => SchemaError::WrongType {
            path,
            detail: format!("{}", err),
        },
        _ => SchemaError::ValueOutOfRange {
            path,
            detail: format!("{}", err),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(path: &str) -> Value {
        let raw = std::fs::read_to_string(path).unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    #[test]
    fn aprp_schema_compiles() {
        let _ = aprp_schema();
    }

    #[test]
    fn aprp_golden_passes_validation() {
        let v: Value = serde_json::from_str(include_str!(
            "../../../test-corpus/aprp/golden_001_minimal.json"
        ))
        .unwrap();
        validate_aprp(&v).expect("golden must pass schema");
    }

    #[test]
    fn aprp_prompt_injection_passes_schema_validation() {
        // Prompt-injection fixture is *schema-valid*; it is denied by *policy*, not schema.
        let v: Value = serde_json::from_str(include_str!(
            "../../../test-corpus/aprp/deny_prompt_injection_request.json"
        ))
        .unwrap();
        validate_aprp(&v).expect("prompt-injection fixture must pass schema");
    }

    #[test]
    fn aprp_adversarial_fails_with_unknown_field() {
        let v: Value = serde_json::from_str(include_str!(
            "../../../test-corpus/aprp/adversarial_unknown_field.json"
        ))
        .unwrap();
        let err = validate_aprp(&v).expect_err("adversarial must fail");
        assert_eq!(err.code(), "schema.unknown_field", "got: {:?}", err);
    }

    #[test]
    fn policy_reference_fixture_passes_validation() {
        let v: Value = serde_json::from_str(include_str!(
            "../../../test-corpus/policy/reference_low_risk.json"
        ))
        .unwrap();
        validate_policy(&v).expect("policy fixture must pass schema");
    }

    // Smoke test that loading paths still works (used by CLI).
    #[test]
    fn loading_via_path_works() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-corpus/aprp/golden_001_minimal.json"
        );
        let v = load(path);
        validate_aprp(&v).unwrap();
    }
}
