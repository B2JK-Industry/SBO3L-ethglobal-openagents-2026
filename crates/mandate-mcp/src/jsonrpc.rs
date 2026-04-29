//! Minimal JSON-RPC 2.0 envelope types for the stdio transport.
//!
//! We deliberately do not depend on `jsonrpc-core` or any other RPC
//! crate — every dependency is one more thing to track for security
//! advisories on a hackathon submission. The shape we use is exactly
//! what `https://www.jsonrpc.org/specification` defines, and tests
//! pin the wire format.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// JSON-RPC 2.0 request / notification envelope. Notifications (no
/// `id`) are not currently emitted by any tool, but the field is
/// optional to accept them on input — we just ignore them.
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    #[serde(default)]
    pub id: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObject>,
    pub id: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorObject {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Response {
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn err(id: Value, error: ErrorObject) -> Self {
        Self {
            jsonrpc: "2.0",
            result: None,
            error: Some(error),
            id,
        }
    }

    /// Manufacture a parse-error response — used when stdin contains
    /// non-JSON or a JSON value that doesn't match the request shape.
    /// Per JSON-RPC 2.0 §5: `id` MUST be null when the request itself
    /// could not be parsed, since the server cannot recover the
    /// original id.
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::err(
            Value::Null,
            ErrorObject {
                code: -32700,
                message: message.into(),
                data: None,
            },
        )
    }

    /// Manufacture an invalid-request response — used when the JSON
    /// parsed but lacks the JSON-RPC 2.0 envelope (no `method`, wrong
    /// `jsonrpc` version, etc).
    pub fn invalid_request(id: Value, message: impl Into<String>) -> Self {
        Self::err(
            id,
            ErrorObject {
                code: -32600,
                message: message.into(),
                data: None,
            },
        )
    }
}

/// Parse one line of stdin into a `Request`. Returns `Err` containing a
/// pre-built `Response` (parse error / invalid request) on failure, so
/// the caller can write that directly back to stdout without further
/// branching.
//
// `Response` is ~144 bytes and clippy's `result_large_err` lint flags
// returning it as `Err` directly. Boxing isn't worth the indirection
// here — the `Err` path is the one-per-bad-input case on the slow stdio
// boundary, and the size is dominated by `serde_json::Value`.
#[allow(clippy::result_large_err)]
pub fn parse_request(line: &str) -> Result<Request, Response> {
    let trimmed = line.trim();
    let value: Value = serde_json::from_str(trimmed)
        .map_err(|e| Response::parse_error(format!("parse error: {e}")))?;
    if value.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
        return Err(Response::invalid_request(
            value.get("id").cloned().unwrap_or(Value::Null),
            "jsonrpc field must be \"2.0\"",
        ));
    }
    if value.get("method").and_then(|v| v.as_str()).is_none() {
        return Err(Response::invalid_request(
            value.get("id").cloned().unwrap_or(Value::Null),
            "method field is required",
        ));
    }
    serde_json::from_value(value)
        .map_err(|e| Response::invalid_request(Value::Null, format!("envelope deserialise: {e}")))
}

/// Convenience builder for tests. Produces a request value as
/// `serde_json::Value` so callers can pretty-print it before writing.
pub fn make_request(id: Value, method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_request_accepts_valid_envelope() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"method":"foo","params":{"x":1}}"#;
        let req = parse_request(raw).unwrap();
        assert_eq!(req.method, "foo");
        assert_eq!(req.params["x"], 1);
    }

    #[test]
    fn parse_request_rejects_wrong_version() {
        let raw = r#"{"jsonrpc":"1.0","id":1,"method":"foo"}"#;
        let resp = parse_request(raw).unwrap_err();
        assert_eq!(resp.error.unwrap().code, -32600);
    }

    #[test]
    fn parse_request_rejects_missing_method() {
        let raw = r#"{"jsonrpc":"2.0","id":1}"#;
        let resp = parse_request(raw).unwrap_err();
        assert_eq!(resp.error.unwrap().code, -32600);
    }

    #[test]
    fn parse_error_id_is_null() {
        let raw = "not json";
        let resp = parse_request(raw).unwrap_err();
        assert_eq!(resp.id, Value::Null);
        assert_eq!(resp.error.unwrap().code, -32700);
    }
}
