# KeeperHub live-integration spike

**Status:** docs spike, not implemented. No code in this PR.
**Owner:** Developer B (this PR), pending API spec + credentials from KeeperHub.
**Sister doc:** [`FEEDBACK.md`](../FEEDBACK.md) §KeeperHub.

This document is the read-only design for moving SBO3L's KeeperHub
adapter from `KeeperHubExecutor::local_mock()` to a real
`KeeperHubExecutor::live()` once a stable workflow webhook schema and
test credentials are available. It exists so that a future "wire it up"
PR has a single artefact to consult, and so judges/sponsors can see the
production path without us claiming it is in place today.

The hackathon build remains:

> `KeeperHubExecutor::local_mock()` returns a deterministic `kh-<ULID>`
> `execution_ref` and prints `mock: true` in demo output.

Nothing here changes that.

## Why a spike instead of code

Three blockers prevent the live wiring landing today:

1. **No public schema for the action-submission / result envelope.** Captured as a feedback request in [`FEEDBACK.md`](../FEEDBACK.md) §KeeperHub → "Suggested improvements".
2. **No KeeperHub credentials in this repo.** None — verified by `git grep` for `kh_*`, `wfb_*`, `KEEPERHUB_TOKEN`, `KEEPERHUB_API_KEY` under `crates/`, `demo-scripts/`, `demo-fixtures/`, `test-corpus/` (all return zero matches). We will not commit credentials, ever.
3. **No live network in CI.** Tests must stay deterministic and offline. The live path is implementable, but its CI coverage is a mock HTTP server inside the test, not a real KeeperHub call.

When all three clear, the live path is a single Rust constructor body
change in `crates/sbo3l-keeperhub-adapter/src/lib.rs` plus a thin
configuration shim. The bulk of the work is in this doc, not the code.

## Truthfulness invariants (must hold during and after the spike)

The live integration **must not** weaken any of these:

- Denied receipts never call KeeperHub. The check stays at the top of
  `KeeperHubExecutor::execute()`, before any I/O.
- Demo runner output continues to label every mock as `mock: true`. The
  live path emits `mock: false` and a real `executionId`, never both.
- No KeeperHub credentials, tokens, secrets, or webhook URLs land in the
  repo. They are environment-only.
- The SBO3L audit chain remains canonical. KeeperHub's `executionId`
  is recorded as the SBO3L `ExecutionReceipt.execution_ref` after the
  audit event is appended; if KeeperHub fails, SBO3L's audit log
  already contains the Allow decision and the failure is reported as an
  `ExecutionError`, not a silent retry.
- The audit bundle (`sbo3l audit export`) carries `execution_ref` as
  an opaque field today; the live `executionId` flows through unchanged.

If any of these invariants is at risk, the live PR is blocked, not patched.

## Target shape of `KeeperHubExecutor::live()`

This is a sketch of the live constructor — **not** committed code. It
exists here so a future PR can be reviewed against a known target.

```rust
// crates/sbo3l-keeperhub-adapter/src/lib.rs (target shape — not landed)

pub struct KeeperHubLiveConfig {
    pub webhook_url: String,
    pub bearer_token: String,
    pub timeout: std::time::Duration,
}

impl KeeperHubLiveConfig {
    /// Read configuration from the environment. Returns Err if the
    /// required env vars are absent — the caller decides whether to
    /// fall back to `local_mock()` or fail closed.
    pub fn from_env() -> Result<Self, ExecutionError> {
        let webhook_url = std::env::var("SBO3L_KEEPERHUB_WEBHOOK_URL")
            .map_err(|_| ExecutionError::Configuration(
                "SBO3L_KEEPERHUB_WEBHOOK_URL not set".into()))?;
        let bearer_token = std::env::var("SBO3L_KEEPERHUB_TOKEN")
            .map_err(|_| ExecutionError::Configuration(
                "SBO3L_KEEPERHUB_TOKEN not set".into()))?;
        Ok(Self {
            webhook_url,
            bearer_token,
            timeout: std::time::Duration::from_secs(15),
        })
    }
}

impl KeeperHubExecutor {
    pub fn live(cfg: KeeperHubLiveConfig) -> Self { /* … */ }
}

impl GuardedExecutor for KeeperHubExecutor {
    fn execute(
        &self,
        request: &PaymentRequest,
        receipt: &PolicyReceipt,
    ) -> Result<ExecutionReceipt, ExecutionError> {
        if !matches!(receipt.decision, Decision::Allow) {
            return Err(ExecutionError::NotApproved(receipt.decision.clone()));
        }
        match &self.mode {
            KeeperHubMode::LocalMock => { /* unchanged */ }
            KeeperHubMode::Live(cfg) => self.execute_live(cfg, request, receipt),
        }
    }
}
```

`execute_live` is the only function that actually makes the network call,
and is the only place a hypothetical cassette-based test would intercept.

## Wire format the adapter intends to send

Until KeeperHub publishes a schema, this is what SBO3L would send.
KeeperHub's actual schema will override; the four `sbo3l_*` fields are
the only ones we need first-class on KeeperHub's side (per `FEEDBACK.md`
§KeeperHub → "Suggested improvements").

The `sbo3l_*` block is built and serialised by
[`Sbo3lEnvelope::to_json_payload()`](../crates/sbo3l-core/src/execution.rs)
(landed in P5.1 / PR #51). `sbo3l_request_hash`,
`sbo3l_policy_hash`, and `sbo3l_receipt_signature` come straight off
the signed `PolicyReceipt`; `sbo3l_audit_event_id` is the receipt's
own `audit_event_id` (already pinned to the just-appended audit chain
event when the executor receives the receipt). The optional fifth field
`sbo3l_passport_capsule_hash` is set via
[`Sbo3lEnvelope::with_passport_capsule()`](../crates/sbo3l-core/src/execution.rs)
once a Passport capsule URI is published (P7.1); it is `serde(skip_serializing_if = "Option::is_none")`,
so pre-Passport KeeperHub deployments don't see the field at all.

```http
POST {webhook_url}
Content-Type: application/json
Authorization: Bearer {token}

{
  "aprp": { … the canonical APRP body, JCS-canonical bytes … },
  "policy_receipt": { … the signed PolicyReceipt JSON … },
  "sbo3l_request_hash":           "<jcs-sha256-hex of aprp>",
  "sbo3l_policy_hash":            "<canonical hash of the active policy>",
  "sbo3l_receipt_signature":      "<ed25519 hex>",
  "sbo3l_audit_event_id":         "evt-<ULID>",
  "sbo3l_passport_capsule_hash":  "<jcs-sha256-hex of capsule, omitted pre-Passport>"
}
```

The envelope IS constructed inside `KeeperHubExecutor::execute()`'s
`Live` arm today, even though the live network call still returns
`BackendOffline` (see PR #51's `keeperhub_live_constructs_envelope_via_from_receipt`
test). That ordering — build the wire-format envelope *before* live
submission turns on — is deliberate: it pins the `sbo3l_*` fields
under regression tests in CI, so a future receipt-shape change can't
silently desync the envelope from the receipt that triggered the call.

**Capsule slot for the IP-1 envelope (target).** When live wiring lands
and a real KeeperHub callback returns, the IP-1 envelope above flows
into the Passport capsule via `ExecutionReceipt.evidence` →
`capsule.execution.executor_evidence` (the new mode-agnostic
sponsor-evidence slot introduced in P6.1, alongside Uniswap's
`UniswapQuoteEvidence`). It is **not** the `execution.live_evidence`
slot: that slot is reserved for transport-level proof (HTTP transport
identifier, response reference, block reference) and the verifier's
bidirectional invariant keeps it strictly live-only. The IP-1 envelope
is sponsor-specific *business* data — `sbo3l_request_hash`,
`sbo3l_policy_hash`, etc. — which is exactly what
`executor_evidence` (`additionalProperties: true`) was added for.

Expected response (target shape — see [`FEEDBACK.md`](../FEEDBACK.md) §KeeperHub for the schema-publication ask):

```json
{
  "executionId": "kh-<workflow-native-id>",
  "status": "submitted",
  "submittedAt": "2026-…Z"
}
```

SBO3L's adapter behaviour:

- 2xx with parseable `executionId` → `ExecutionReceipt {
  sponsor: "keeperhub",
  execution_ref: "kh-<id from KeeperHub>",
  mock: false,
  note: "live: submitted to <host>"
}`.
- non-2xx, network error, timeout, or unparseable body → explicit
  `ExecutionError`. **No fallback to `local_mock()`** — the operator must
  notice and decide.

## Test strategy (no live network in CI)

The CI gate must stay deterministic and offline. Three layers:

1. **Unit tests** — `execute_live` against a `Box<dyn HttpClient>` trait
   that the test substitutes with a fake. Cover: 2xx happy path, 4xx
   policy-error, 5xx server-error, network-error, timeout, parse-error.
   No real network.
2. **Integration test** — a tiny in-process HTTP server (e.g. `wiremock`
   or hand-rolled `tokio::net::TcpListener`) that asserts:
   - the four `sbo3l_*` envelope fields are present and match the
     receipt/audit values;
   - the `Authorization: Bearer …` header is set from the env var;
   - the body is JCS-canonical bytes, not pretty-printed.
3. **End-to-end smoke** — left to operators with real credentials,
   gated behind `SBO3L_KEEPERHUB_LIVE=1` env var so that absence of
   the flag preserves today's offline guarantees. CI never sets this.

The mock HTTP server fixture lives under `crates/sbo3l-execution/`'s
test directory, not under `demo-fixtures/`. Demo fixtures are inputs to
the demo runner; this is testing infrastructure.

## Concrete shopping list for the live PR

When KeeperHub publishes a schema and a test webhook (or KeeperHub team
makes a sandbox available), the live PR is approximately:

| File | Change |
|---|---|
| `crates/sbo3l-keeperhub-adapter/Cargo.toml` | Add `reqwest` (or equivalent — minimal-feature, blocking-disabled, rustls-tls). |
| `crates/sbo3l-keeperhub-adapter/src/lib.rs` | Add `KeeperHubLiveConfig`, `KeeperHubMode::Live(cfg)`, `execute_live`. |
| `crates/sbo3l-core/src/execution.rs` (or equivalent) | Add `ExecutionError::Configuration`, `ExecutionError::Network`, `ExecutionError::HttpStatus(u16)`, `ExecutionError::Parse`. |
| `crates/sbo3l-keeperhub-adapter/tests/live_mock_server.rs` | New integration test driving an in-process HTTP server. |
| `crates/sbo3l-keeperhub-adapter/src/lib.rs` | Or extend existing tests — unit cover for `execute_live` with a fake `HttpClient`. |
| `demo-scripts/sponsors/keeperhub-guarded-execution.sh` | Add a one-line `if [ "$SBO3L_KEEPERHUB_LIVE" = "1" ]; then …` branch that calls a new `--execute-keeperhub-live` flag on the research-agent harness; the default still runs `local_mock`. |
| `demo-agents/research-agent/src/main.rs` | New `--execute-keeperhub-live` flag (parallel to the existing `--execute-keeperhub`). Constructs `KeeperHubExecutor::live(KeeperHubLiveConfig::from_env()?)`. |
| `docs/cli/keeperhub-live.md` | Operator-facing how-to: which env vars, expected output, failure modes. |
| `FEEDBACK.md` §KeeperHub | Replace the "What was unclear" bullet about missing schema with "Resolved by …; here is the schema we now consume" once schema is published. |

Estimated diff: ~250 lines of Rust, ~80 lines of new integration test,
~100 lines of operator docs. Very tight scope for one reviewable PR.

## Open questions for the KeeperHub team

These overlap with the suggestions in [`FEEDBACK.md`](../FEEDBACK.md)
§KeeperHub but are listed here so a single doc captures everything a
future implementer needs to look up:

1. **Submission envelope schema.** What is the canonical request body
   shape? Is it documented for third-party callers or only via the
   in-product workflow editor?
2. **Token model.** When does a caller use `kh_*` (KeeperHub-native API
   token) vs `wfb_*` (workflow-webhook token)? Which header does each
   belong in? A worked example in the docs would resolve this in
   minutes.
3. **`executionId` lookup.** Is there a documented GET path or MCP tool
   to query post-submit status / run logs? SBO3L would call this from
   the operator console.
4. **Rate limiting.** What are the per-token submission limits and the
   recommended backoff?
5. **Idempotency.** Does KeeperHub honour an `Idempotency-Key` request
   header on the workflow webhook? SBO3L's PSM-A2 idempotency layer
   sits one level upstream; understanding the intersection avoids
   double-execution on retry.
6. **Async vs sync.** Does the workflow webhook return synchronously
   with a final `executionId`, or is `executionId` returned immediately
   with status delivered via callback? SBO3L can support both, but
   the adapter shape differs.
7. **Optional response headers.** Would KeeperHub be willing to attach
   `X-SBO3L-Receipt-Signature` and `X-SBO3L-Policy-Hash` on signed
   callbacks? (Captured as a feedback ask; here for completeness.)
8. **Webhook signing.** Does KeeperHub sign callback bodies? If yes,
   which scheme (HMAC-SHA256? Ed25519? rolling secret?). If no, the
   adapter should still verify the connection over TLS but cannot bind
   callback authenticity to the original submission.

## Risks

- **Schema drift.** KeeperHub's eventual schema will likely differ from
  the sketch above. The four `sbo3l_*` envelope fields are the only
  things SBO3L strictly needs first-class; everything else is
  flexible.
- **Timeout policy.** A KeeperHub workflow may legitimately take
  minutes. The adapter's 15-second default is wrong for long-running
  workflows; the live PR will need to choose between a configurable
  per-action timeout and an async-callback model. Open question 6
  decides this.
- **Idempotency overlap with PSM-A2.** If KeeperHub also implements
  idempotency keys, SBO3L may end up with two layers. Open question 5
  decides this.
- **Audit-bundle bloat.** Today the bundle carries `execution_ref` only.
  If we want to embed the full KeeperHub callback into the bundle, that
  is a separate schema bump — not in scope for the first live PR.

## What this PR is and isn't

- **Is:** a single Markdown file at `docs/keeperhub-live-spike.md`
  capturing the design for a future live PR.
- **Isn't:** any change to `crates/`, `demo-scripts/`, `demo-fixtures/`,
  `operator-console/`, `trust-badge/`, schemas, or OpenAPI.
- **Isn't:** a claim that KeeperHub live is implemented. It is the
  opposite — a written commitment to what implementation will look like
  *when* the three blockers above clear.

## Acceptance criteria for closing this spike

The spike is "closed" — i.e. a future PR can be reviewed as a literal
implementation of this doc — when:

- KeeperHub publishes (or shares with us) a stable submission/result
  envelope schema (open question 1).
- We have a sandbox / test webhook URL + token we can use under
  `SBO3L_KEEPERHUB_LIVE=1` for the operator-side smoke test (no CI).
- The eight open questions above have answers — even short ones — that
  a Rust adapter author can compile against.

Until then, this doc stays read-only and SBO3L continues to ship
`local_mock()` with `mock: true` honestly disclosed.
