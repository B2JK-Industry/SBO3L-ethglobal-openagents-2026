# Consumer-side adapter shape — for KH issue #54

This file shows what `submit_live_to` would look like once [KeeperHub/cli#54](https://github.com/KeeperHub/cli/issues/54) publishes the workflow webhook timeout SLO (p50 / p95 / p99). Doc-only; no behavioral change.

## Status: draft. Blocked on KeeperHub/cli#54.

## What changes

### Default timeout: 5s → 10s (under SLO p99)

Today (`crates/sbo3l-keeperhub-adapter/src/lib.rs:283-289`):

```rust
// 5-second hard timeout — long enough for transient slowness, short
// enough that an unresponsive backend doesn't block the executor.
// Operators who need a longer ceiling can wrap `submit_live` with
// their own retry/timeout policy upstream.
let client = reqwest::blocking::Client::builder()
    .timeout(std::time::Duration::from_secs(5))
    .build()
    ...;
```

Proposed (once #54 publishes p99 = 5s SLO):

```rust
// Timeout: 2 × KH's documented p99 (#54) → 10s default.
// Rationale: at 1 × p99, ~1% of legitimate requests would be aborted
// (since p99 is by definition the 99th percentile latency). 2 × p99
// gives ~3-4 nines of headroom while still bounding worst-case wait.
//
// Override per-deployment via env: SBO3L_KEEPERHUB_TIMEOUT_SECS.
const DEFAULT_TIMEOUT_SECS: u64 = 10;
const TIMEOUT_ENV: &str = "SBO3L_KEEPERHUB_TIMEOUT_SECS";

let timeout_secs: u64 = std::env::var(TIMEOUT_ENV)
    .ok()
    .and_then(|s| s.parse().ok())
    .unwrap_or(DEFAULT_TIMEOUT_SECS);

let client = reqwest::blocking::Client::builder()
    .timeout(std::time::Duration::from_secs(timeout_secs))
    .build()
    ...;
```

### Status check before submit (optional)

If KH ships a status page (`status.keeperhub.com`) with a JSON endpoint, adapters can do a quick liveness check before burning a real submit:

```rust
fn check_kh_status(timeout_ms: u64) -> Result<KhStatus, ExecutionError> {
    // GET https://status.keeperhub.com/api/v1/component/workflow-webhook
    // returns: { "status": "operational" | "degraded" | "outage", "p99_ms_24h": 3200 }
    //
    // On "outage", return BackendOffline immediately — no point burning a submit.
    // On "degraded" with p99_ms_24h > our timeout * 1000, log a warning.
    ...
}
```

This is opt-in (env-gated) — adds a hop on every submit, so only valuable in environments where the operator wants fail-fast over best-effort.

## What this unlocks

Adapter authors today pick timeouts by eyeballing their own latency measurements. Documented SLO + a status page lets:

- The adapter set timeout = 2 × published p99 with confidence
- Operators of an SBO3L deployment know "10s ceiling, with a 1% chance of falsely-aborted retries when KH's tail latency spikes" instead of "5s, hope for the best"
- Sponsor reviewers reading our adapter PR can audit the timeout choice against a public number

## Test additions

```rust
#[test]
fn timeout_env_override_respected() {
    std::env::set_var(TIMEOUT_ENV, "20");
    let timeout = read_timeout_from_env();
    assert_eq!(timeout, Duration::from_secs(20));
}

#[test]
fn timeout_env_garbage_falls_back_to_default() {
    std::env::set_var(TIMEOUT_ENV, "not-a-number");
    let timeout = read_timeout_from_env();
    assert_eq!(timeout, Duration::from_secs(10));
}
```

## Why this is a draft today

The 10s default + the `2 × p99` rule rest on KH's published p99 number. Until #54 lands, our 5s is a hand-picked guess and bumping it to 10s is just a different guess. The PR sits draft; ready as soon as #54 publishes the SLO.
