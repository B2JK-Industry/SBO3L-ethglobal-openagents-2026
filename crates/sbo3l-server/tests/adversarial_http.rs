//! R20 Task C — adversarial HTTP inputs (Dev 1 slice).
//!
//! Four new fail-closed tests on `POST /v1/payment-requests`. Each test:
//!   1. Spins up the in-process axum router via `tower::oneshot` (no
//!      real network sockets — same harness as `lib.rs::tests`).
//!   2. Sends a deliberately-malicious payload.
//!   3. Asserts a specific 4xx status + a specific error code in the
//!      RFC 7807 response body.
//!   4. Verifies the server did not panic / crash (the oneshot
//!      returning at all is sufficient evidence — a panicked task in
//!      axum surfaces as a 500 with a connection close).
//!
//! Coverage matrix (this file):
//!
//!   | Adversarial input                                     | Expected status | Expected code                       |
//!   |-------------------------------------------------------|-----------------|-------------------------------------|
//!   | Idempotency-Key with CRLF / control-byte injection    | 400             | protocol.idempotency_key_invalid    |
//!   | APRP body with 1000-level nested object               | 400 (or 422)    | schema.* (parse / structural)       |
//!   | APRP body with a 10 MiB string field                  | 4xx             | (any rejection — no OOM crash)      |
//!   | APRP `agent_id` containing a NUL byte                 | 400             | schema.* (pattern violation)        |
//!
//! These four extend the pre-existing 13 HTTP adversarial tests in
//! `lib.rs::tests` + `integration_auth.rs` + `test_idempotency_race.rs`
//! to a new total of 17 fail-closed paths.

use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode};
use http_body_util::BodyExt;
use sbo3l_server::{reference_policy, router, AppState};
use sbo3l_storage::Storage;
use serde_json::{json, Value};
use tower::ServiceExt;

const APRP_GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../test-corpus/aprp/golden_001_minimal.json"
));

fn build_app() -> axum::Router {
    let storage = Storage::open_in_memory().unwrap();
    let policy = reference_policy();
    router(AppState::new(policy, storage))
}

fn golden_body() -> Value {
    serde_json::from_str(APRP_GOLDEN).unwrap()
}

async fn post_raw(
    app: axum::Router,
    body_bytes: Vec<u8>,
    headers: Vec<(&'static str, HeaderValue)>,
) -> (StatusCode, Vec<u8>) {
    let mut req = Request::builder()
        .method("POST")
        .uri("/v1/payment-requests")
        .header("content-type", "application/json");
    for (k, v) in headers {
        req = req.header(k, v);
    }
    let req = req.body(Body::from(body_bytes)).unwrap();
    let resp = app.oneshot(req).await.expect("server must not panic");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, bytes)
}

fn parse_problem(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice::<Value>(bytes).ok()
}

// ---------------------------------------------------------------------------
// HTTP-1: Idempotency-Key with newline / header-injection attempt.
// ---------------------------------------------------------------------------
//
// Real-world equivalent: an attacker controls the Idempotency-Key value
// passed to the SBO3L daemon by an upstream gateway and tries to inject
// a second header (`X-Injected: bar`) by embedding `\r\n` in the value.
// The HTTP stack (the `http` crate underpinning axum/hyper) refuses
// `\r` / `\n` at `HeaderValue` construction — that's the FIRST line of
// defence and we pin it explicitly. As DEFENCE IN DEPTH we then send a
// header value containing a non-ASCII control byte (0x7F = DEL — the
// `http` crate accepts this byte at construction time but `to_str()`
// rejects it as non-visible-ASCII), which our handler must surface as
// `protocol.idempotency_key_invalid` (400).
#[tokio::test]
async fn http1_idempotency_key_crlf_injection_rejected_by_http_layer_and_handler() {
    // Layer 1: the http crate refuses the literal CRLF injection. A
    // client cannot even construct this HeaderValue — so the bytes
    // never reach the wire / handler at all.
    let crlf = HeaderValue::from_bytes(b"foo\r\nX-Injected: bar");
    assert!(
        crlf.is_err(),
        "CRLF in header values must be refused by the http crate \
         (this is the first line of defence against header injection)"
    );
    let lf_only = HeaderValue::from_bytes(b"foo\nX-Injected: bar");
    assert!(
        lf_only.is_err(),
        "bare LF must also be refused — RFC 7230 disallows it in field values"
    );

    // Layer 2: defence-in-depth — high-bit bytes (0x80..0xFE, the "obs-text"
    // range from RFC 7230) are accepted at the wire layer (the http crate's
    // `from_bytes` permits them) but `to_str()` rejects them as non-ASCII.
    // Our `extract_idempotency_key` handler runs `to_str()` first and
    // surfaces a 400 `protocol.idempotency_key_invalid`. Pinning this
    // is what makes the test "fail closed loud" if a future refactor
    // accidentally swaps `to_str()` for a permissive `String::from_utf8_lossy`.
    let app = build_app();
    let evil_header = HeaderValue::from_bytes(b"abcdefghij\xFAinjected").unwrap();
    let (status, bytes) = post_raw(
        app,
        serde_json::to_vec(&golden_body()).unwrap(),
        vec![("Idempotency-Key", evil_header)],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "non-ASCII / control-byte Idempotency-Key must surface 400; got {status}"
    );
    let problem = parse_problem(&bytes).expect("response must be RFC 7807 JSON");
    assert_eq!(
        problem["code"], "protocol.idempotency_key_invalid",
        "got: {problem}"
    );
}

// ---------------------------------------------------------------------------
// HTTP-2: deeply-nested JSON (recursion-depth attack).
// ---------------------------------------------------------------------------
//
// Real-world equivalent: an attacker submits a JSON body whose nesting
// depth would either (a) blow the parser's stack or (b) force the
// schema validator into worst-case O(depth) work. serde_json caps
// recursion at 128 by default — beyond that the parse fails with
// "recursion limit exceeded". Either axum surfaces the parse error as
// 400 / 422, or the body deserialises into a 1000-deep `Value` tree
// and the schema validator rejects it as a `schema.wrong_type` (the
// outer object lacks the required APRP fields) before any recursion
// hits a limit. EITHER outcome is fail-closed; what we pin is "no 5xx,
// no 200 — the daemon does NOT swallow this and emit a receipt."
#[tokio::test]
async fn http2_deeply_nested_json_rejected_without_panic() {
    let app = build_app();
    // Build `{"a":{"a":{"a":...}}}` 1000 levels deep. We hand-roll the
    // bytes so we don't depend on serde_json being able to serialise
    // this depth — the point is to test the SERVER's tolerance, not
    // the test-side serialiser's.
    let depth = 1000usize;
    let mut bytes = Vec::with_capacity(depth * 6 + 10);
    for _ in 0..depth {
        bytes.extend_from_slice(b"{\"a\":");
    }
    bytes.extend_from_slice(b"1");
    for _ in 0..depth {
        bytes.extend_from_slice(b"}");
    }

    let (status, body_bytes) = post_raw(app, bytes, vec![]).await;
    assert!(
        status.is_client_error(),
        "deeply-nested JSON must surface a 4xx (parse or schema rejection); got {status}"
    );
    assert_ne!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "must NOT 500 — the parser/schema layer absorbed the attack"
    );
    // Best-effort body inspection: the response is either a parse-rejection
    // problem or a schema-rejection problem; both are fine.
    if let Some(problem) = parse_problem(&body_bytes) {
        // If the body parsed as JSON, sanity-check there's no `receipt`
        // / `audit_event_id` (those would prove pipeline progress).
        assert!(
            problem.get("receipt").is_none(),
            "rejection must carry no receipt; got: {problem}"
        );
        assert!(
            problem.get("audit_event_id").is_none(),
            "rejection must carry no audit_event_id; got: {problem}"
        );
    }
}

// ---------------------------------------------------------------------------
// HTTP-3: very long string field (memory-exhaustion attack).
// ---------------------------------------------------------------------------
//
// Real-world equivalent: an attacker sets a user-controlled string
// (here `task_id`) to 10 MiB of `'A'` characters, hoping to either
// (a) exhaust the daemon's heap before the schema validator gets a
// chance to reject the field, or (b) DoS via repeated 10 MiB
// concurrent submissions. The fail-closed contract is the same as for
// HTTP-2: 4xx, no panic, no 5xx, no receipt emitted. Axum's default
// body limit is 2 MiB — anything larger is rejected before the JSON
// parser even runs. The test pins that behaviour; if a future config
// raises the body limit and the schema layer's `maxLength` check
// happens to STILL catch this (task_id is `minLength: 1` with no
// upper bound today, so the attack would actually slip past the schema
// → it's the body-size cap that saves us), the test becomes our
// regression detector for any future relaxation.
#[tokio::test]
async fn http3_oversized_string_field_rejected_without_oom() {
    let app = build_app();
    let mut body = golden_body();
    let huge = "A".repeat(10 * 1024 * 1024); // 10 MiB
    body["task_id"] = Value::String(huge);
    let body_bytes = serde_json::to_vec(&body).unwrap();
    assert!(
        body_bytes.len() > 10 * 1024 * 1024,
        "test setup: serialised body must exceed 10 MiB"
    );

    let (status, _) = post_raw(app, body_bytes, vec![]).await;
    assert!(
        status.is_client_error() || status == StatusCode::PAYLOAD_TOO_LARGE,
        "10 MiB body must surface a 4xx (body-size cap or schema); got {status}"
    );
    assert_ne!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "must NOT 500 — the body-size / schema layer absorbed the attack"
    );
    assert_ne!(
        status,
        StatusCode::OK,
        "10 MiB body must NEVER reach a 200 / receipt path"
    );
}

// ---------------------------------------------------------------------------
// HTTP-4: NUL byte in agent_id.
// ---------------------------------------------------------------------------
//
// The APRP schema's `agent_id` pattern is `^[a-z0-9][a-z0-9_-]{2,63}$`
// — NUL (` `) is not in the character class, so the schema
// validator rejects the body with `schema.*` and the server returns
// 400. This catches the prototypical "log-poisoning / log-injection
// via embedded control char" attack at the boundary, before any
// downstream sink (audit log, metrics, CLI) is exposed to the byte.
#[tokio::test]
async fn http4_nul_byte_in_agent_id_rejected_at_schema_validation() {
    let app = build_app();
    let mut body = golden_body();
    // JSON literal ` ` is 6 source chars; serde_json embeds the
    // raw NUL byte in the resulting String. The schema's pattern
    // disallows NUL; the validator rejects the request.
    body["agent_id"] = json!("foo\u{0000}bar");
    let body_bytes = serde_json::to_vec(&body).unwrap();
    // serde_json escapes the NUL as the 6-char sequence ` ` in the
    // wire payload. The server-side parse decodes it back to the literal
    // NUL byte, at which point the schema pattern catches it. Pin the
    // wire-form so the test setup doesn't drift away from the threat.
    // `foo bar` is 12 bytes once written to JSON
    // (the backslash-u-0000 escape sequence is 6 chars).
    assert!(
        body_bytes.windows(12).any(|w| w == b"foo\\u0000bar"),
        "test setup: wire payload must carry the JSON-escaped NUL"
    );

    let (status, resp_bytes) = post_raw(app, body_bytes, vec![]).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "NUL byte in agent_id must surface 400; got {status}"
    );
    let problem = parse_problem(&resp_bytes).expect("response must be RFC 7807 JSON");
    let code = problem["code"].as_str().unwrap_or("");
    assert!(
        code.starts_with("schema."),
        "expected a schema.* error class; got code={code:?} problem={problem}"
    );
    // No pipeline progress on rejection.
    assert!(problem.get("receipt").is_none());
    assert!(problem.get("audit_event_id").is_none());
}
