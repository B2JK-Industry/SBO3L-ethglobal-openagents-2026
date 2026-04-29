# `mandate-mcp` — Mandate MCP stdio JSON-RPC server (Passport P3.1)

> *MCP turns Mandate from a daemon into infrastructure other agents can call.*

`mandate-mcp` is a line-delimited JSON-RPC 2.0 server over stdio that
exposes Mandate's policy, capsule, and audit primitives as MCP tools.
Every tool **wraps** an existing primitive — no new business logic, no
new schemas — so the wire contract any MCP client gets is exactly the
same truth the CLI and HTTP API already publish.

- Source: [`crates/mandate-mcp/`](../../crates/mandate-mcp/).
- Demo: [`demo-scripts/sponsors/mcp-passport.sh`](../../demo-scripts/sponsors/mcp-passport.sh) — exercises every tool and writes a transcript to `demo-scripts/artifacts/mcp-transcript.json`.
- Tests: [`crates/mandate-mcp/tests/jsonrpc_integration.rs`](../../crates/mandate-mcp/tests/jsonrpc_integration.rs) — 29 integration tests across in-process dispatch, stdio child-process transport, and path-sandbox escape coverage.

## Wire format

JSON-RPC 2.0 requests and responses, **one JSON object per line**.
That's the entire transport — no Content-Length header framing. Stdin
EOF closes the server cleanly. Tracing logs go to stderr; stdout is the
JSON-RPC channel only, so MCP-aware clients reading stdout are never
interleaved with diagnostic output.

The Rust MCP SDK is too churn-heavy for a hackathon deliverable; per
the P3.1 backlog risk note we ship a minimal protocol and document it
here. Clients that already speak JSON-RPC over stdio (Claude Code, Cursor,
the demo script's `bash + jq` setup) can call any tool without an MCP
SDK shim.

### Request

```jsonc
{
  "jsonrpc": "2.0",
  "id":      1,                    // any JSON value; echoed in the response
  "method":  "mandate.validate_aprp",
  "params":  { "aprp": { /* APRP body */ } }
}
```

### Response — success

```jsonc
{
  "jsonrpc": "2.0",
  "id":      1,
  "result":  { "ok": true, "request_hash": "835469d807bb…" }
}
```

### Response — error

```jsonc
{
  "jsonrpc": "2.0",
  "id":      1,
  "error": {
    "code":    -32000,                 // tool-level error namespace
    "message": "schema.value_out_of_range at /amount.value: …",
    "data":    { "code": "aprp_invalid" }   // stable, machine-readable
  }
}
```

Pre-tool envelope errors use the standard JSON-RPC 2.0 codes:

| Code     | Meaning |
| -------- | ------- |
| `-32700` | Parse error. Stdin contained a non-JSON line. `id` is null. |
| `-32600` | Invalid Request. JSON parsed but lacks the JSON-RPC envelope. |
| `-32000` | Tool-level error. `data.code` carries a stable string (see below). |

### Stable `data.code` values

Tests pin these exact strings — they are part of the wire contract.

| `data.code`                   | Meaning |
| ----------------------------- | ------- |
| `params_invalid`              | Required parameter missing / wrong shape / unknown method. |
| `schema_violation`            | Capsule failed JSON-Schema 2020-12 validation. |
| `aprp_invalid`                | APRP body failed `schemas/aprp_v1.json`. |
| `policy_not_active`           | DB has no active policy row (run `mandate policy activate` first). |
| `policy_load_failed`          | Active policy row no longer parses (storage corruption). |
| `pipeline_failed`             | The in-process payment-requests pipeline returned an unexpected error. |
| `executor_failed`             | The mock executor (KeeperHub / Uniswap) returned an error. |
| `requires_human_unsupported`  | Policy returned `requires_human`; the capsule schema only encodes allow/deny. |
| `live_mode_rejected`          | `mandate.run_guarded_execution` was called with `mode: "live"`. P5.1+. |
| `capsule_io_failed`           | Capsule path could not be read. |
| `capsule_invalid`             | Capsule failed P1.1 cross-field invariants (`message` carries `(capsule.<code>)`). |
| `capsule_not_deny`            | `mandate.explain_denial` was called on an allow capsule. |
| `audit_event_not_found`       | `audit_lookup`: the event_id is not in the chain. |
| `audit_event_id_mismatch`     | `audit_lookup`: receipt's `audit_event_id` ≠ requested `audit_event_id`. |
| `bundle_build_failed`         | `audit_bundle::build` returned an error (e.g. chain segment doesn't include the receipt's event). |
| `storage_failed`              | SQLite open / read failure. |
| `capsule.path_escape`         | Round 0 path sandbox: a `db` or `path` argument resolves outside `MANDATE_MCP_ROOT` (or its symlink-resolved canonical form does). |

Pipeline failures additionally surface the HTTP-level Problem code
verbatim under `data.code` (e.g. `schema.value_out_of_range`,
`policy.nonce_replay`) — same string an HTTP API caller branches on.

## Path sandbox (`MANDATE_MCP_ROOT`)

Every filesystem path argument the dispatcher accepts (`db` on
`mandate.decide` / `mandate.run_guarded_execution` / `mandate.audit_lookup`,
`path` on `mandate.verify_capsule` / `mandate.explain_denial`) is
resolved against the `MANDATE_MCP_ROOT` env var and rejected if it
escapes that root.

| Behaviour | Detail |
| --- | --- |
| Default | If `MANDATE_MCP_ROOT` is unset (or empty), the server uses its working directory at startup. |
| Resolution | Relative paths resolve against the process working directory (standard filesystem semantics), absolute paths are used as-is. |
| Canonicalization | The result is canonicalized via `Path::canonicalize`, which collapses `..` segments **and follows symlinks**. A symlink whose target lives outside the root is treated as escape, not as an in-root path. |
| Non-existent files | If the target path doesn't yet exist (e.g. a fresh SQLite DB filename), the server canonicalizes the *parent directory* and reattaches the filename. The parent must still resolve inside the root. |
| Failure mode | Rejected paths surface as `data.code = "capsule.path_escape"` with a stderr-quality diagnostic (`path X escapes MANDATE_MCP_ROOT Y`). |

Operators running the server under `MANDATE_MCP_ROOT=/var/mandate-mcp`
get a hard guarantee: a prompt-injected MCP client cannot read or write
files outside that subtree, no matter how it crafts the path argument.

The sponsor demo (`demo-scripts/sponsors/mcp-passport.sh`) demonstrates
the operator pattern: it `export`s `MANDATE_MCP_ROOT` to its tempdir
before spawning the server, so every `db` argument resolves cleanly.

## Tool catalogue

Six tools today; every one of them wraps an existing primitive:

| Tool                                | Wraps |
| ----------------------------------- | ----- |
| `mandate.validate_aprp`             | `mandate_core::schema::validate_aprp` |
| `mandate.decide`                    | `mandate_server::router` (oneshot pattern) |
| `mandate.run_guarded_execution`     | `mandate_server` + `KeeperHubExecutor::local_mock` / `UniswapExecutor::local_mock` |
| `mandate.verify_capsule`            | `mandate_core::passport::verify_capsule` |
| `mandate.explain_denial`            | `verify_capsule` + structured projection |
| `mandate.audit_lookup`              | `Storage::audit_chain_prefix_through` + `mandate_core::audit_bundle::build` |

Plus one meta method `tools/list` that returns this catalogue with
input/output JSON schemas. Useful for MCP clients that auto-discover
tools.

### `mandate.validate_aprp`

Validate an APRP body against `schemas/aprp_v1.json` and (on success)
return its canonical JCS SHA-256 hash.

```jsonc
// request
{ "method": "mandate.validate_aprp", "params": { "aprp": { /* body */ } } }
// success
{ "result": { "ok": true, "request_hash": "835469d807bb…" } }
// failure
{ "error": { "code": -32000, "data": { "code": "aprp_invalid" }, "message": "…" } }
```

### `mandate.decide`

Drive the offline payment-requests pipeline (APRP → policy → budget →
audit → signed receipt) in-process via the same axum oneshot pattern
`mandate passport run` uses. Returns the same `PaymentRequestResponse`
the HTTP `POST /v1/payment-requests` API would.

```jsonc
// request
{ "method": "mandate.decide",
  "params": { "aprp": { /* body */ }, "db": "/path/to/mandate.sqlite" } }
// success — allow path
{ "result": { "status": "auto_approved",
              "decision": "allow",
              "request_hash": "…",
              "policy_hash":  "…",
              "audit_event_id": "evt-…",
              "receipt": { /* signed PolicyReceipt */ } } }
// success — deny path
{ "result": { "status": "rejected",
              "decision": "deny",
              "deny_code": "policy.deny_unknown_provider", … } }
```

The DB must already have an active policy (run `mandate policy activate
<file> --db <path>` once). Empty DB → `policy_not_active`.

### `mandate.run_guarded_execution`

Run `mandate.decide` and, on the allow path only, call the chosen mock
executor.

- Allow path: invokes `KeeperHubExecutor::local_mock` or
  `UniswapExecutor::local_mock`; returns `execution.status="submitted"`
  with the executor's `execution_ref` (`kh-<ULID>` / `uni-<ULID>`).
- Deny path: executor is **never** called.
  `execution.status="not_called"`, `execution.execution_ref=null`. Hard
  truthfulness invariant from P1.1's tampered_001 fixture — same rule
  `mandate passport run` enforces.

```jsonc
// request
{ "method": "mandate.run_guarded_execution",
  "params": { "aprp": {...}, "db": "...", "executor": "keeperhub" } }
```

`mode: "live"` is rejected with `live_mode_rejected`; live integration
lands in P5.1 (`KeeperHubExecutor::live`) and P6.1 (Uniswap quote
evidence).

### `mandate.verify_capsule`

Run the P1.1 verifier (schema + 8 cross-field invariants) on a capsule.
Accepts either `{ "capsule": { /* inline */ } }` or `{ "path": "…" }`
to read from disk.

```jsonc
// request
{ "method": "mandate.verify_capsule",
  "params": { "path": "demo-scripts/artifacts/passport-allow.json" } }
// success
{ "result": { "ok": true, "schema": "mandate.passport_capsule.v1" } }
// failure (cross-field)
{ "error": { "code": -32000, "data": { "code": "capsule_invalid" },
             "message": "capsule.request_hash_mismatch: … (capsule.request_hash_mismatch)" } }
```

The verifier's `(capsule.<code>)` discriminator (request_hash_mismatch,
deny_with_execution, schema_invalid, …) is preserved verbatim in the
error message so MCP clients can branch on the same key as
`mandate passport verify` callers.

### `mandate.explain_denial`

Reads + verifies a capsule and returns a deny-only structured
projection. Returns `capsule_not_deny` if the capsule is an allow.

```jsonc
// success
{ "result": {
    "schema": "mandate.passport_capsule.v1",
    "decision": { "result": "deny", "matched_rule": "…", "deny_code": "…" },
    "audit":    { "audit_event_id": "evt-…" },
    "policy":   { "policy_hash": "…", "policy_version": 1 } } }
```

### `mandate.audit_lookup` (IP-3 alignment)

The IP-3 sister tool. Given a Mandate `audit_event_id` plus the signed
`PolicyReceipt` plus signer pubkeys, returns the corresponding
`mandate.audit_bundle.v1`. See
[**docs/keeperhub-integration-paths.md §IP-3**](../keeperhub-integration-paths.md)
for the cross-side composition story.

```jsonc
// request
{ "method": "mandate.audit_lookup",
  "params": {
    "audit_event_id": "evt-01KQCKM35K3E7DMXMBSNG250HQ",
    "db":             "/path/to/mandate.sqlite",
    "receipt":        { /* signed PolicyReceipt JSON */ },
    "receipt_pubkey": "ea4a6c63…",
    "audit_pubkey":   "66be7e33…"
  }
}
// success
{ "result": {
    "ok": true,
    "bundle": {
      "bundle_type": "mandate.audit_bundle.v1",
      "version": 1,
      "exported_at": "2026-04-29T…Z",
      "receipt": {…},
      "audit_event": {…},
      "audit_chain_segment": [{…}, {…}],
      "verification_keys": { "receipt_signer_pubkey_hex": "…", "audit_signer_pubkey_hex": "…" },
      "summary": { "decision": "allow", … }
    }
  } }
// failure modes
{ "error": { "data": { "code": "audit_event_not_found" }, … } }
{ "error": { "data": { "code": "audit_event_id_mismatch" }, … } }
```

The tool wraps `Storage::audit_chain_prefix_through(event_id)` and
`audit_bundle::build` verbatim — **no storage changes** vs. main, no
`audit_event_id` index, no new fields. The receipt is taken as input
because Mandate doesn't currently store receipts (they live with the
agent that received them); auditors compose
`keeperhub.lookup_execution(execution_id)` (KeeperHub side, IP-3) →
`mandate.audit_lookup(audit_event_id, receipt)` (Mandate side, this
tool) for one round-trip per side end-to-end verification.

## IP-3 cross-side composition

Mandate Passport's wire-level claim is "any signed Mandate decision can
be re-verified offline by anyone who has the bundle and the public
keys." IP-3 makes that one-tool-call from an MCP client by pairing two
symmetric tools:

```
auditor                                                                  result
   │
   ├──── keeperhub.lookup_execution(execution_id) ──────────────►  KeeperHub MCP
   │      ◄── { status, mandate_audit_event_id,                    (their side)
   │           mandate_request_hash, mandate_receipt_signature, … }
   │
   ├──── mandate.audit_lookup(audit_event_id, receipt, …) ───────►  Mandate MCP
   │      ◄── { bundle: mandate.audit_bundle.v1 }                   (this server)
   │
   └────  bundle.audit_event_id == mandate_audit_event_id?  ─►  ✓ end-to-end
          bundle.summary.request_hash == mandate_request_hash?      auditability
```

KeeperHub builds the left-hand tool on their side
(`keeperhub.lookup_execution` per
[`docs/keeperhub-integration-paths.md` §IP-3](../keeperhub-integration-paths.md));
Mandate ships the right-hand `mandate.audit_lookup` here. The
catalogue's "what we are asking for" stays exactly that — KeeperHub
reviews the contract; we already have the symmetric tool.

## Running the server

```bash
cargo build --release --bin mandate-mcp

# As a stdio server, e.g. behind an MCP client:
./target/release/mandate-mcp

# As a one-shot dialog, piping NDJSON:
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | ./target/release/mandate-mcp
```

The server is **stateless** between calls — each tool that needs SQLite
opens a fresh `Storage` handle against the supplied `db` path and drops
it on return. SQLite WAL mode tolerates the sequential opens.

## Demo

```bash
bash demo-scripts/sponsors/mcp-passport.sh
```

Builds both binaries, activates the reference policy in a tempfile DB,
exercises every tool over a stdin/stdout pipe, and writes a complete
transcript to `demo-scripts/artifacts/mcp-transcript.json`. Exit 0
iff every step returned the expected result.

The 13-gate hackathon demo (`demo-scripts/run-openagents-final.sh`)
and the production-shaped runner (`demo-scripts/run-production-shaped-mock.sh`,
26 real / 0 mock / 1 skipped) are **untouched** — the MCP demo lives
on its own surface.

## Out of scope on this server (Passport P3.1)

- No live network calls. `mode: "live"` is rejected. Real KeeperHub
  submission lands in P5.1; live ENS resolution in P4.1; Uniswap quote
  evidence in P6.1.
- No HTTP transport. Stdio only.
- No new schemas. Every tool reads/writes the existing APRP / receipt /
  capsule / audit-bundle shapes.
- No `keeperhub.lookup_execution` — that's KeeperHub's side of IP-3.
- No `mandate.lookup_passport_capsule` — out of scope per the IP-3
  catalogue (the symmetric tool returns the audit bundle, not the
  capsule). Capsule lookup, if useful, can land in a follow-up PR.
