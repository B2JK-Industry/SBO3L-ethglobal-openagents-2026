# Consumer-side adapter shape — for KH issue #56

This file shows what `submit_live_to` would look like once [KeeperHub/cli#56](https://github.com/KeeperHub/cli/issues/56) documents the max payload size for workflow webhook submissions. Doc-only; no behavioral change.

## Status: draft. Blocked on KeeperHub/cli#56.

## What changes

### Pre-POST envelope size check

Today (`crates/sbo3l-keeperhub-adapter/src/lib.rs:280-295`):

```rust
let body = serde_json::Value::Object(body);
// ... straight to POST, no size check ...
let resp = client.post(webhook_url).bearer_auth(token).json(&body).send()?;
```

Proposed (once #56 publishes the limit):

```rust
const MAX_ENVELOPE_BYTES: usize = 1_048_576;  // 1 MB per #56
const ENVELOPE_SOFT_WARN: usize = 900_000;    // log warning above this

let body = serde_json::Value::Object(body);
let body_bytes = serde_json::to_vec(&body)
    .map_err(|e| ExecutionError::ProtocolError(format!("envelope serialize: {e}")))?;

if body_bytes.len() > MAX_ENVELOPE_BYTES {
    // Fail loud BEFORE wasting upload bandwidth on a guaranteed-fail
    // request. The error names the limit + actual so adapter callers
    // can decide whether to retry with a stripped envelope (e.g. swap
    // inline passport capsule for a URI).
    return Err(ExecutionError::EnvelopeTooLarge {
        limit_bytes: MAX_ENVELOPE_BYTES,
        actual_bytes: body_bytes.len(),
    });
}

if body_bytes.len() > ENVELOPE_SOFT_WARN {
    tracing::warn!(
        bytes = body_bytes.len(),
        limit = MAX_ENVELOPE_BYTES,
        "envelope approaching KH webhook size limit"
    );
}

let resp = client
    .post(webhook_url)
    .bearer_auth(token)
    .body(body_bytes)
    .header("Content-Type", "application/json")
    .send()
    .map_err(|e| ExecutionError::ProtocolError(format!("HTTP send failed: {e}")))?;
```

### New `ExecutionError` variant

`crates/sbo3l-core/src/execution.rs`:

```rust
pub enum ExecutionError {
    NotApproved(Decision),
    BackendOffline(String),
    Integration(String),
    ProtocolError(String),

    // NEW: pre-POST size check failure. Distinct from ProtocolError
    // (which fires on actual HTTP errors) so callers can branch on it.
    #[error("envelope too large: {actual_bytes} bytes (limit {limit_bytes})")]
    EnvelopeTooLarge {
        limit_bytes: usize,
        actual_bytes: usize,
    },
}
```

### Inline-vs-URI passport capsule selection

This is what the size check unlocks. Currently `build_envelope` (`lib.rs:165`) carries only the IP-1 fields — the passport capsule is referenced by URI elsewhere in the agent's flow. With a documented size budget:

```rust
fn build_envelope_with_capsule(
    receipt: &PolicyReceipt,
    passport_capsule_bytes: &[u8],
) -> Sbo3lEnvelope {
    // If the encoded capsule fits comfortably under the limit (with
    // headroom for the rest of the envelope), embed it inline.
    // Otherwise emit a URI and a content hash so KH can fetch on demand.
    let capsule_b64 = base64::encode(passport_capsule_bytes);
    let other_envelope_bytes = 2_000;  // request_hash, policy_hash, signature, etc.
    let headroom = MAX_ENVELOPE_BYTES.saturating_sub(other_envelope_bytes + capsule_b64.len());

    if headroom > 100_000 {  // at least 100KB headroom — embed
        Sbo3lEnvelope::with_inline_capsule(receipt, &capsule_b64)
    } else {
        let capsule_hash = hex::encode(blake3::hash(passport_capsule_bytes).as_bytes());
        Sbo3lEnvelope::with_capsule_uri(receipt, &capsule_hash)
    }
}
```

This is the path requested in [issue #50](https://github.com/KeeperHub/cli/issues/50) — fully self-contained verifiable evidence inline when feasible.

## Test additions

```rust
#[test]
fn envelope_under_limit_submits_normally() {
    let body = small_envelope();  // < 1 KB
    let result = submit_live_to(&body_to_request(&body), ...);
    assert!(result.is_ok());
}

#[test]
fn envelope_over_limit_fails_pre_post() {
    let body = oversize_envelope(2_000_000);  // 2 MB
    let result = submit_live_to(&body_to_request(&body), ...);
    let err = result.unwrap_err();
    matches!(err, ExecutionError::EnvelopeTooLarge { .. });
    // Verify NO HTTP request was made (mock server's request count == 0).
}

#[test]
fn envelope_near_limit_submits_with_warning() {
    let body = near_limit_envelope(950_000);
    let result = submit_live_to(&body_to_request(&body), ...);
    assert!(result.is_ok());
    // Inspect tracing output for the warn-level log.
}
```

## Why this is a draft today

The `1_048_576` limit + `900_000` soft warn are placeholder numbers — the real values come from KH publishing the documented limit (#56). This PR converts to a real diff once those numbers exist. Until then, embedding a passport capsule inline is unsafe (we'd risk silently exceeding an unknown ceiling).
