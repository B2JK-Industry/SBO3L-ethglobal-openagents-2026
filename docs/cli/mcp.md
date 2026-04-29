# `sbo3l-mcp` — SBO3L MCP stdio JSON-RPC server (Passport P3.1)

> *MCP turns SBO3L from a daemon into infrastructure other agents can call.*

`sbo3l-mcp` is a line-delimited JSON-RPC 2.0 server over stdio that
exposes SBO3L's policy, capsule, and audit primitives as MCP tools.
Every tool **wraps** an existing primitive — no new business logic, no
new schemas — so the wire contract any MCP client gets is exactly the
same truth the CLI and HTTP API already publish.

- Source: [`crates/sbo3l-mcp/`](../../crates/sbo3l-mcp/).
- Demo: [`demo-scripts/sponsors/mcp-passport.sh`](../../demo-scripts/sponsors/mcp-passport.sh) — exercises every tool and writes a transcript to `demo-scripts/artifacts/mcp-transcript.json`.
- Tests: [`crates/sbo3l-mcp/tests/jsonrpc_integration.rs`](../../crates/sbo3l-mcp/tests/jsonrpc_integration.rs) — 29 integration tests across in-process dispatch, stdio child-process transport, and path-sandbox escape coverage.

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
  "method":  "sbo3l.validate_aprp",
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
| `policy_not_active`           | DB has no active policy row (run `sbo3l policy activate` first). |
| `policy_load_failed`          | Active policy row no longer parses (storage corruption). |
| `pipeline_failed`             | The in-process payment-requests pipeline returned an unexpected error. |
| `executor_failed`             | The mock executor (KeeperHub / Uniswap) returned an error. |
| `requires_human_unsupported`  | Policy returned `requires_human`; the capsule schema only encodes allow/deny. |
| `live_mode_rejected`          | `sbo3l.run_guarded_execution` was called with `mode: "live"`. P5.1+. |
| `capsule_io_failed`           | Capsule path could not be read. |
| `capsule_invalid`             | Capsule failed P1.1 cross-field invariants (`message` carries `(capsule.<code>)`). |
| `capsule_not_deny`            | `sbo3l.explain_denial` was called on an allow capsule. |
| `audit_event_not_found`       | `audit_lookup`: the event_id is not in the chain. |
| `audit_event_id_mismatch`     | `audit_lookup`: receipt's `audit_event_id` ≠ requested `audit_event_id`. |
| `bundle_build_failed`         | `audit_bundle::build` returned an error (e.g. chain segment doesn't include the receipt's event). |
| `storage_failed`              | SQLite open / read failure. |
| `capsule.path_escape`         | Round 0 path sandbox: a `db` or `path` argument resolves outside `SBO3L_MCP_ROOT` (or its symlink-resolved canonical form does). |

Pipeline failures additionally surface the HTTP-level Problem code
verbatim under `data.code` (e.g. `schema.value_out_of_range`,
`policy.nonce_replay`) — same string an HTTP API caller branches on.

## Path sandbox (`SBO3L_MCP_ROOT`)

Every filesystem path argument the dispatcher accepts (`db` on
`sbo3l.decide` / `sbo3l.run_guarded_execution` / `sbo3l.audit_lookup`,
`path` on `sbo3l.verify_capsule` / `sbo3l.explain_denial`) is
resolved against the `SBO3L_MCP_ROOT` env var and rejected if it
escapes that root.

| Behaviour | Detail |
| --- | --- |
| Default | If `SBO3L_MCP_ROOT` is unset (or empty), the server uses its working directory at startup. |
| Resolution | Relative paths resolve against the process working directory (standard filesystem semantics), absolute paths are used as-is. |
| Canonicalization | The result is canonicalized via `Path::canonicalize`, which collapses `..` segments **and follows symlinks**. A symlink whose target lives outside the root is treated as escape, not as an in-root path. |
| Non-existent files | If the target path doesn't yet exist (e.g. a fresh SQLite DB filename), the server canonicalizes the *parent directory* and reattaches the filename. The parent must still resolve inside the root. |
| Failure mode | Rejected paths surface as `data.code = "capsule.path_escape"` with a stderr-quality diagnostic (`path X escapes SBO3L_MCP_ROOT Y`). |

Operators running the server under `SBO3L_MCP_ROOT=/var/sbo3l-mcp`
get a hard guarantee: a prompt-injected MCP client cannot read or write
files outside that subtree, no matter how it crafts the path argument.

The sponsor demo (`demo-scripts/sponsors/mcp-passport.sh`) demonstrates
the operator pattern: it `export`s `SBO3L_MCP_ROOT` to its tempdir
before spawning the server, so every `db` argument resolves cleanly.

## Tool catalogue

Six tools today; every one of them wraps an existing primitive:

| Tool                                | Wraps |
| ----------------------------------- | ----- |
| `sbo3l.validate_aprp`             | `sbo3l_core::schema::validate_aprp` |
| `sbo3l.decide`                    | `sbo3l_server::router` (oneshot pattern) |
| `sbo3l.run_guarded_execution`     | `sbo3l_server` + `KeeperHubExecutor::local_mock` / `UniswapExecutor::local_mock` |
| `sbo3l.verify_capsule`            | `sbo3l_core::passport::verify_capsule` |
| `sbo3l.explain_denial`            | `verify_capsule` + structured projection |
| `sbo3l.audit_lookup`              | `Storage::audit_chain_prefix_through` + `sbo3l_core::audit_bundle::build` |

Plus one meta method `tools/list` that returns this catalogue with
input/output JSON schemas. Useful for MCP clients that auto-discover
tools.

### `sbo3l.validate_aprp`

Validate an APRP body against `schemas/aprp_v1.json` and (on success)
return its canonical JCS SHA-256 hash.

```jsonc
// request
{ "method": "sbo3l.validate_aprp", "params": { "aprp": { /* body */ } } }
// success
{ "result": { "ok": true, "request_hash": "835469d807bb…" } }
// failure
{ "error": { "code": -32000, "data": { "code": "aprp_invalid" }, "message": "…" } }
```

### `sbo3l.decide`

Drive the offline payment-requests pipeline (APRP → policy → budget →
audit → signed receipt) in-process via the same axum oneshot pattern
`sbo3l passport run` uses. Returns the same `PaymentRequestResponse`
the HTTP `POST /v1/payment-requests` API would.

```jsonc
// request
{ "method": "sbo3l.decide",
  "params": { "aprp": { /* body */ }, "db": "/path/to/sbo3l.sqlite" } }
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

The DB must already have an active policy (run `sbo3l policy activate
<file> --db <path>` once). Empty DB → `policy_not_active`.

### `sbo3l.run_guarded_execution`

Run `sbo3l.decide` and, on the allow path only, call the chosen mock
executor.

- Allow path: invokes `KeeperHubExecutor::local_mock` or
  `UniswapExecutor::local_mock`; returns `execution.status="submitted"`
  with the executor's `execution_ref` (`kh-<ULID>` / `uni-<ULID>`).
- Deny path: executor is **never** called.
  `execution.status="not_called"`, `execution.execution_ref=null`. Hard
  truthfulness invariant from P1.1's tampered_001 fixture — same rule
  `sbo3l passport run` enforces.

```jsonc
// request
{ "method": "sbo3l.run_guarded_execution",
  "params": { "aprp": {...}, "db": "...", "executor": "keeperhub" } }
```

`mode: "live"` is rejected with `live_mode_rejected`; live integration
lands in P5.1 (`KeeperHubExecutor::live`) and P6.1 (Uniswap quote
evidence).

### `sbo3l.verify_capsule`

Run the P1.1 verifier (schema + 8 cross-field invariants) on a capsule.
Accepts either `{ "capsule": { /* inline */ } }` or `{ "path": "…" }`
to read from disk.

```jsonc
// request
{ "method": "sbo3l.verify_capsule",
  "params": { "path": "demo-scripts/artifacts/passport-allow.json" } }
// success
{ "result": { "ok": true, "schema": "sbo3l.passport_capsule.v1" } }
// failure (cross-field)
{ "error": { "code": -32000, "data": { "code": "capsule_invalid" },
             "message": "capsule.request_hash_mismatch: … (capsule.request_hash_mismatch)" } }
```

The verifier's `(capsule.<code>)` discriminator (request_hash_mismatch,
deny_with_execution, schema_invalid, …) is preserved verbatim in the
error message so MCP clients can branch on the same key as
`sbo3l passport verify` callers.

### `sbo3l.explain_denial`

Reads + verifies a capsule and returns a deny-only structured
projection. Returns `capsule_not_deny` if the capsule is an allow.

```jsonc
// success
{ "result": {
    "schema": "sbo3l.passport_capsule.v1",
    "decision": { "result": "deny", "matched_rule": "…", "deny_code": "…" },
    "audit":    { "audit_event_id": "evt-…" },
    "policy":   { "policy_hash": "…", "policy_version": 1 } } }
```

### `sbo3l.audit_lookup` (IP-3 alignment)

The IP-3 sister tool. Given a SBO3L `audit_event_id` plus the signed
`PolicyReceipt` plus signer pubkeys, returns the corresponding
`sbo3l.audit_bundle.v1`. See
[**docs/keeperhub-integration-paths.md §IP-3**](../keeperhub-integration-paths.md)
for the cross-side composition story.

```jsonc
// request
{ "method": "sbo3l.audit_lookup",
  "params": {
    "audit_event_id": "evt-01KQCKM35K3E7DMXMBSNG250HQ",
    "db":             "/path/to/sbo3l.sqlite",
    "receipt":        { /* signed PolicyReceipt JSON */ },
    "receipt_pubkey": "ea4a6c63…",
    "audit_pubkey":   "66be7e33…"
  }
}
// success
{ "result": {
    "ok": true,
    "bundle": {
      "bundle_type": "sbo3l.audit_bundle.v1",
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
because SBO3L doesn't currently store receipts (they live with the
agent that received them); auditors compose
`keeperhub.lookup_execution(execution_id)` (KeeperHub side, IP-3) →
`sbo3l.audit_lookup(audit_event_id, receipt)` (SBO3L side, this
tool) for one round-trip per side end-to-end verification.

## IP-3 cross-side composition

SBO3L Passport's wire-level claim is "any signed SBO3L decision can
be re-verified offline by anyone who has the bundle and the public
keys." IP-3 makes that one-tool-call from an MCP client by pairing two
symmetric tools:

```
auditor                                                                  result
   │
   ├──── keeperhub.lookup_execution(execution_id) ──────────────►  KeeperHub MCP
   │      ◄── { status, sbo3l_audit_event_id,                    (their side)
   │           sbo3l_request_hash, sbo3l_receipt_signature, … }
   │
   ├──── sbo3l.audit_lookup(audit_event_id, receipt, …) ───────►  SBO3L MCP
   │      ◄── { bundle: sbo3l.audit_bundle.v1 }                   (this server)
   │
   └────  bundle.audit_event_id == sbo3l_audit_event_id?  ─►  ✓ end-to-end
          bundle.summary.request_hash == sbo3l_request_hash?      auditability
```

KeeperHub builds the left-hand tool on their side
(`keeperhub.lookup_execution` per
[`docs/keeperhub-integration-paths.md` §IP-3](../keeperhub-integration-paths.md));
SBO3L ships the right-hand `sbo3l.audit_lookup` here. The
catalogue's "what we are asking for" stays exactly that — KeeperHub
reviews the contract; we already have the symmetric tool.

## Running the server

```bash
cargo build --release --bin sbo3l-mcp

# As a stdio server, e.g. behind an MCP client:
./target/release/sbo3l-mcp

# As a one-shot dialog, piping NDJSON:
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | ./target/release/sbo3l-mcp
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
- No `sbo3l.lookup_passport_capsule` — out of scope per the IP-3
  catalogue (the symmetric tool returns the audit bundle, not the
  capsule). Capsule lookup, if useful, can land in a follow-up PR.
