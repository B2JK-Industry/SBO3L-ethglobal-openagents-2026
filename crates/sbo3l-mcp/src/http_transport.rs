//! HTTP transport for the SBO3L MCP server.
//!
//! Same tool catalogue + dispatch loop as the stdio path; the only
//! difference is wire framing. Each `POST /mcp` accepts one JSON-RPC 2.0
//! request body and returns one JSON-RPC 2.0 response body. Health check
//! at `GET /health` returns `{"status":"ok","tools":<n>}`.
//!
//! Why HTTP alongside stdio:
//!   - Claude Desktop's MCP config points at `command + args` (stdio) BUT
//!     Cursor / Continue / Cline / IDE plugins increasingly target HTTP
//!     transport for cross-process scenarios.
//!   - HTTP makes the server reachable from Docker compose topologies + CI
//!     smoke jobs without needing pseudo-tty plumbing.
//!   - Streamable HTTP (the MCP spec's evolution of SSE) builds on this
//!     handler trivially — see docs/integrations/mcp-clients/index.md
//!     for the upgrade path.
//!
//! Out of scope (deliberate):
//!   - SSE / Streamable HTTP — the MCP spec is in flux on this; ship one
//!     transport at a time.
//!   - Authentication — the server is local-by-default. Operators putting
//!     it on a non-loopback interface should front it with their own
//!     reverse proxy + auth layer.
//!   - HTTP/2 / TLS — same reasoning.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use serde_json::{json, Value};

use crate::{dispatch_to_response, jsonrpc, ServerContext};

/// Build the axum Router for the HTTP MCP transport.
///
/// Routes:
///   - `POST /mcp` — JSON-RPC 2.0 envelope in body, JSON-RPC 2.0 response back
///   - `GET  /health` — `{"status":"ok","tools":N}` for compose healthchecks
pub fn router(ctx: Arc<ServerContext>) -> Router {
    Router::new()
        .route("/mcp", post(mcp_handler))
        .route("/health", get(health_handler))
        .with_state(ctx)
}

async fn mcp_handler(State(ctx): State<Arc<ServerContext>>, body: String) -> impl IntoResponse {
    // Parse error → return a JSON-RPC parse-error envelope with HTTP 200.
    // Per JSON-RPC 2.0 the protocol layer carries the error; HTTP is just
    // transport. Returning HTTP 4xx would break clients that expect to
    // discriminate via the response envelope's `error` field.
    let req = match jsonrpc::parse_request(&body) {
        Ok(r) => r,
        Err(resp) => return Json(serde_json::to_value(resp).unwrap_or(Value::Null)),
    };
    let resp = dispatch_to_response(&req, &ctx);
    Json(serde_json::to_value(resp).unwrap_or(Value::Null))
}

async fn health_handler(State(_ctx): State<Arc<ServerContext>>) -> impl IntoResponse {
    let tools = crate::tools_catalogue().len();
    (
        StatusCode::OK,
        Json(json!({ "status": "ok", "tools": tools, "transport": "http" })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request as HttpRequest, StatusCode};
    use tower::ServiceExt;

    fn ctx() -> Arc<ServerContext> {
        Arc::new(ServerContext::new())
    }

    #[tokio::test]
    async fn health_returns_ok_with_tool_count() {
        let app = router(ctx());
        let response = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["status"], "ok");
        assert!(body["tools"].as_u64().unwrap() >= 6);
        assert_eq!(body["transport"], "http");
    }

    #[tokio::test]
    async fn tools_list_round_trip() {
        // tools/list is the cheapest method that exercises dispatch + the
        // catalogue surface — proves the HTTP wrapper preserves what the
        // stdio transport already exposes. The result is a flat array
        // (not {tools: [...]}) per the existing P3.1 wire contract.
        let app = router(ctx());
        let req_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        });
        let response = app
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(req_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["id"], 1);
        let tools = body["result"]
            .as_array()
            .expect("result is the tools array");
        assert!(tools.len() >= 6);
        // Every tool descriptor has at minimum a name + input_schema (snake_case
        // per the existing crate's serialization).
        for tool in tools {
            assert!(tool["name"].is_string(), "missing name: {tool}");
            assert!(
                tool["input_schema"].is_object(),
                "missing input_schema: {tool}"
            );
        }
    }

    #[tokio::test]
    async fn parse_error_returns_jsonrpc_envelope_not_http_4xx() {
        // A malformed body must come back as a JSON-RPC parse-error
        // envelope (HTTP 200), not an HTTP 400 — clients discriminate
        // failures via the protocol layer.
        let app = router(ctx());
        let response = app
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from("{not json"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        // -32700 is the canonical JSON-RPC parse-error code.
        assert_eq!(body["error"]["code"], -32700);
    }

    #[tokio::test]
    async fn unknown_method_returns_tool_error_envelope() {
        // The existing P3.1 wire contract maps unknown methods to the
        // tool-error namespace (-32000) with data.code = "params_invalid".
        // We keep this behaviour over the HTTP transport so clients that
        // already drive the stdio surface don't see a wire-shape change.
        let app = router(ctx());
        let req_body = json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "no.such.method",
            "params": {}
        });
        let response = app
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(req_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["id"], 7);
        assert_eq!(body["error"]["code"], crate::TOOL_ERROR_CODE);
        assert_eq!(
            body["error"]["data"]["code"],
            crate::error_codes::PARAMS_INVALID
        );
    }
}
