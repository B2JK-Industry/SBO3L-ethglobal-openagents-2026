# Consumer-side adapter shape — for KH issue #55

This file shows what `submit_live_to` would look like once [KeeperHub/cli#55](https://github.com/KeeperHub/cli/issues/55) lands `X-KeeperHub-Schema-Version` response header + `Accept-KeeperHub-Schema` request header. Doc-only; no behavioral change.

## Status: draft. Blocked on KeeperHub/cli#55.

## What changes

### Submit side — pin the request schema

Today (`crates/sbo3l-keeperhub-adapter/src/lib.rs:290-295`):

```rust
let resp = client
    .post(webhook_url)
    .bearer_auth(token)
    .json(&body)
    .send()
    .map_err(|e| ExecutionError::ProtocolError(format!("HTTP send failed: {e}")))?;
```

Proposed:

```rust
const PINNED_SCHEMA: &str = "2026-05-01";  // bumped intentionally per release

let resp = client
    .post(webhook_url)
    .bearer_auth(token)
    .header("Accept-KeeperHub-Schema", PINNED_SCHEMA)
    .json(&body)
    .send()
    .map_err(|e| ExecutionError::ProtocolError(format!("HTTP send failed: {e}")))?;
```

When KH ships a new envelope shape, this adapter version keeps speaking the pinned shape. We bump `PINNED_SCHEMA` in a separate release with explicit migration notes.

### Response side — verify schema and remove the `executionId` → `id` fallback

Today (`crates/sbo3l-keeperhub-adapter/src/lib.rs:316-319`):

```rust
let execution_id = parsed
    .get("executionId")
    .or_else(|| parsed.get("id"))    // legacy fallback — undocumented
    .and_then(|v| v.as_str())
    .ok_or_else(...)?;
```

Proposed:

```rust
// Verify response schema matches what we requested. If KH responds
// with a different schema (gateway misconfig, mid-deploy roll, etc.),
// fail loud rather than parse against the wrong key set.
let response_schema = resp.headers()
    .get("X-KeeperHub-Schema-Version")
    .and_then(|h| h.to_str().ok())
    .unwrap_or("");
if response_schema != PINNED_SCHEMA {
    let allow_drift = std::env::var("KH_ALLOW_SCHEMA_DRIFT")
        .ok()
        .as_deref() == Some("1");
    if !allow_drift {
        return Err(ExecutionError::ProtocolError(format!(
            "schema drift: pinned {PINNED_SCHEMA}, got {response_schema}. \
             Set KH_ALLOW_SCHEMA_DRIFT=1 to ignore (NOT recommended)."
        )));
    }
}

// With pinned schema, we know exactly which key holds the execution id.
// No fallback needed.
let execution_id = parsed
    .get("executionId")
    .and_then(|v| v.as_str())
    .ok_or_else(...)?;
```

## What this unlocks

- **Safe refactors on KH side.** KH can ship a new envelope shape (e.g. nested `data.executionId`, snake_case `execution_id`) without breaking pinned adapters. Pinned adapters keep working until they explicitly bump their schema.
- **Migration signals.** When KH bumps the schema, every adapter using the old version sees the drift error in their logs immediately — they don't have to wait for prod traffic to surface a regression.
- **Vendor-tracking precedent.** Stripe, GitHub, and Auth0 all use date-based version headers for the same reason. Adapter authors familiar with those patterns will feel at home.

## Test additions

```rust
#[test]
fn submit_sends_accept_keeperhub_schema_header() {
    let mock = mockito::Server::new();
    let mock_endpoint = mock.mock("POST", "/api/workflows/test/webhook")
        .match_header("Accept-KeeperHub-Schema", PINNED_SCHEMA)
        .with_status(200)
        .with_header("X-KeeperHub-Schema-Version", PINNED_SCHEMA)
        .with_body(r#"{"executionId": "kh-test-001"}"#)
        .create();

    let result = submit_live_to(&request, &receipt, &mock.url(), "wfb_test");
    assert!(result.is_ok());
    mock_endpoint.assert();
}

#[test]
fn submit_rejects_response_with_drifted_schema() {
    let mock = mockito::Server::new();
    let _m = mock.mock("POST", "/api/workflows/test/webhook")
        .with_status(200)
        .with_header("X-KeeperHub-Schema-Version", "2099-01-01")  // future
        .with_body(r#"{"executionId": "kh-test-001"}"#)
        .create();

    let result = submit_live_to(&request, &receipt, &mock.url(), "wfb_test");
    let err = result.unwrap_err();
    assert!(format!("{err}").contains("schema drift"));
}
```

## Why this is a draft today

`PINNED_SCHEMA = "2026-05-01"` is just a placeholder string — the real value comes from KH publishing the schema versioning convention (issue #55). When it lands, this PR converts to a real diff against `submit_live_to` + adds the two tests above.
