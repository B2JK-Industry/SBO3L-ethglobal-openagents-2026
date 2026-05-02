# Consumer-side adapter shape — for KH issue #52

This file shows what `submit_live_to` in `crates/sbo3l-keeperhub-adapter/src/lib.rs` would look like once [KeeperHub/cli#52](https://github.com/KeeperHub/cli/issues/52) lands a documented HTTP error code catalog + adapter retry semantics. Doc-only; no behavioral change.

## Status: draft. Blocked on KeeperHub/cli#52.

## Proposed `ExecutionError` enum split

Today (`crates/sbo3l-core/src/execution.rs:19-37`):

```rust
pub enum ExecutionError {
    NotApproved(Decision),
    BackendOffline(String),
    Integration(String),
    ProtocolError(String),  // catch-all — 4xx, 5xx, network, parse
}
```

Proposed shape once #52 catalog lands:

```rust
pub enum ExecutionError {
    NotApproved(Decision),
    BackendOffline(String),
    Integration(String),

    // NEW: split ProtocolError into typed variants matching the
    // documented KH catalog. Permanent vs transient is the actionable
    // distinction the agent / executor needs.
    Permanent {
        code: KhErrorCode,    // e.g. KhErrorCode::EnvelopeMalformed
        http_status: u16,
        body_snippet: String,
    },
    Transient {
        code: KhErrorCode,    // e.g. KhErrorCode::ServerTransient
        http_status: u16,
        body_snippet: String,
        retry_after: Option<Duration>,  // honors RFC 9110 Retry-After
    },
}

pub enum KhErrorCode {
    EnvelopeMalformed,        // 400
    AuthTokenInvalid,         // 401
    AuthTokenRevoked,         // 403
    WorkflowNotFound,         // 404
    IdempotencyConflict,      // 409 — needs #51 + #52
    EnvelopeSchemaMismatch,   // 422 — needs #48 + #52
    RateLimit,                // 429
    ServerTransient,          // 5xx
    Unknown(u16),             // any uncatalogued code
}
```

## Proposed `submit_live_to` retry shape

```rust
fn submit_live_to(...) -> Result<ExecutionReceipt, ExecutionError> {
    // ... existing token / envelope / body construction ...

    let mut attempt = 0u8;
    let max_retries = 3;
    loop {
        attempt += 1;
        let resp = client.post(webhook_url).bearer_auth(token).json(&body).send();

        match resp {
            Ok(r) if r.status().is_success() => return parse_success(r),
            Ok(r) => {
                let status = r.status().as_u16();
                let kh_code = KhErrorCode::from_status_and_body(status, &r);
                let body = r.text().unwrap_or_default();
                let snippet = body.chars().take(200).collect();

                if kh_code.is_transient() && attempt < max_retries {
                    let retry_after = parse_retry_after(&r.headers())
                        .unwrap_or_else(|| jittered_backoff(attempt));
                    std::thread::sleep(retry_after);
                    continue;
                }
                return if kh_code.is_transient() {
                    Err(ExecutionError::Transient {
                        code: kh_code, http_status: status, body_snippet: snippet,
                        retry_after: parse_retry_after(&r.headers()),
                    })
                } else {
                    Err(ExecutionError::Permanent {
                        code: kh_code, http_status: status, body_snippet: snippet,
                    })
                };
            }
            Err(e) if e.is_timeout() => {
                if attempt < max_retries {
                    std::thread::sleep(jittered_backoff(attempt));
                    continue;
                }
                return Err(ExecutionError::Transient {
                    code: KhErrorCode::ServerTransient, http_status: 0,
                    body_snippet: format!("timeout after {attempt} attempts: {e}"),
                    retry_after: None,
                });
            }
            Err(e) => return Err(ExecutionError::Permanent {
                code: KhErrorCode::Unknown(0), http_status: 0,
                body_snippet: format!("network: {e}"),
            }),
        }
    }
}
```

## Test additions (once we can derive the catalog)

Each `KhErrorCode` variant gets a unit test against a `mockito::Server` that returns the corresponding HTTP status + body, asserting:
- Correct variant (`Permanent` vs `Transient`)
- Correct retry behavior (no retries on permanent; up to 3 with jittered backoff on transient)
- `Retry-After` header honored on 429

Existing 18 `submit_live_to` tests stay green; ~10 new tests added.

## Why this is a draft today

The `KhErrorCode` enum requires the published catalog from #52 — without it, every variant is our guess. We could ship the enum with our own values, but that defeats the purpose: the whole point is that adapter authors converge on KH's documented codes.

This PR will be marked ready as soon as #52 publishes the canonical catalog, at which point this doc becomes the inline diff against `crates/sbo3l-keeperhub-adapter/src/lib.rs` + `crates/sbo3l-core/src/execution.rs`.
