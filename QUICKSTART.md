# Quickstart — SBO3L in 5 minutes

> *Don't give your agent a wallet. Give it a mandate.*

SBO3L is a local **policy + budget + receipt + audit firewall** for autonomous agents. Your agent never holds a private key — it sends an intent (an `APRP`) to SBO3L over HTTP, SBO3L decides allow/deny, signs an Ed25519 policy receipt, appends to a hash-chained audit log, and only then routes to a sponsor executor (KeeperHub, Uniswap, etc.).

This page gets you from `git clone` to a signed receipt in **under 5 minutes**. For the full vertical demo + 13 verification gates, see [`README.md`](README.md). For the architecture deep-dive, see [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md).

## Prerequisites

- **Rust 1.85+** ([rustup.rs](https://rustup.rs/)) — `rustc --version` should print 1.85 or later.
- `curl` and `jq` (used in the example below; both are standard in macOS / Linux distros).

## Step 1 — Run the daemon

```bash
git clone https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026
cd SBO3L-ethglobal-openagents-2026
cargo run --bin sbo3l-server
```

The daemon listens on `127.0.0.1:8730` with an in-process SQLite DB (`sbo3l.db`). First build is ~2 minutes; subsequent runs are seconds.

## Step 2 — Send your agent's intent (one curl)

In a second terminal:

```bash
curl -s http://127.0.0.1:8730/v1/payment-requests \
  -X POST -H "Content-Type: application/json" \
  -d @test-corpus/aprp/golden_001_minimal.json | jq
```

## Step 3 — What you got back

A **signed policy receipt** plus the audit-event id:

```json
{
  "status": "auto_approved",
  "decision": "allow",
  "matched_rule_id": "allow-small-x402-api-call",
  "request_hash":  "c0bd2fab4a7d4686d686edcc9c8356315cd66b820a2072493bf758a1eeb500db",
  "policy_hash":   "e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf",
  "audit_event_id": "evt-01KQFV2V3JJV5NZA7NRFR46YAH",
  "receipt": {
    "receipt_type": "sbo3l.policy_receipt.v1",
    "agent_id": "research-agent-01",
    "decision": "allow",
    "issued_at": "2026-04-30T18:40:40Z",
    "signature": {
      "algorithm": "ed25519",
      "key_id": "decision-signer-v1",
      "signature_hex": "ffc597b1…"
    }
  }
}
```

Three things just happened, fail-closed:

1. **Schema validation** — APRP is rejected with `400 schema.*` if any field is missing or unknown (`serde(deny_unknown_fields)` end-to-end).
2. **Policy decision** — deterministic eval of the active reference policy. `request_hash` is JCS-canonical SHA-256.
3. **Signed receipt + audit append** — Ed25519 signature over the canonical receipt body, hash-chained into the audit log via `prev_event_hash`.

## Step 4 — Try a denial (prompt-injection request)

```bash
curl -s http://127.0.0.1:8730/v1/payment-requests \
  -X POST -H "Content-Type: application/json" \
  -d @test-corpus/aprp/deny_prompt_injection_request.json | jq
```

You'll see `"decision": "deny"` + `"deny_code": "policy.deny_unknown_provider"`. The sponsor executor is **never called** on the deny path — that's the hard truthfulness rule the Passport capsule schema enforces.

## Step 5 — Verify a Passport capsule offline (no daemon, no network)

The repo ships a self-contained proof artefact you can verify offline against the agent's published Ed25519 pubkey alone:

```bash
# Generate fresh allow + deny capsules from the production-shaped runner.
bash demo-scripts/run-production-shaped-mock.sh

# Verify the allow capsule (structural + 8 cross-field invariants).
cargo run -p sbo3l-cli -- passport verify \
  --path demo-scripts/artifacts/passport-allow.json
```

The default mode runs schema + cross-field truthfulness invariants (deny→no execution, live→evidence, request/policy hash internal-consistency). Pass `--strict` with `--policy / --receipt-pubkey / --audit-bundle` to additionally re-derive the canonical hashes and the Ed25519 signature from the capsule alone.

## Where to go next

- **Full vertical demo** (13 gates, ~10s): `bash demo-scripts/run-openagents-final.sh`
- **MCP server tools** for Claude Desktop / Cursor — `crates/sbo3l-mcp/` and [`docs/mcp-integration-guide.md`](docs/mcp-integration-guide.md)
- **KeeperHub adoption paths** (IP-1..IP-5) — [`docs/keeperhub-integration-paths.md`](docs/keeperhub-integration-paths.md)
- **Live integrations** — KeeperHub webhook, ENS mainnet, Uniswap Sepolia QuoterV2 — see [`README.md`](README.md#status) (all env-gated, demo defaults stay on `local_mock` / offline fixtures).
- **Schemas** — `schemas/` (policy receipt, audit bundle, passport capsule, all `serde(deny_unknown_fields)`).
- **Reference docs** — [`docs/cli/`](docs/cli/) (per-CLI page), [`SECURITY_NOTES.md`](SECURITY_NOTES.md) (production-deployment limits).

## Production caveats

The daemon's default config is **demo-only** — read [`SECURITY_NOTES.md`](SECURITY_NOTES.md) before pointing it at a public network:

- Bound to `127.0.0.1` by default; `SBO3L_LISTEN` lets you change it but **there is no built-in auth middleware**. Any production deployment needs an upstream reverse proxy with mTLS or bearer-token auth.
- The receipt-signing key is a **deterministic dev seed** committed in source. Production deployments inject real signers via `AppState::with_signers` (TEE/HSM-backed).
- Budget tracker is in-memory; it resets on restart. Production path is SQLite-backed budget rows.
- `Idempotency-Key` cache lookup-then-write is non-atomic; concurrent same-key requests can race. Tracked, not yet hardened.

These are documented limits, not new bugs — the demo surface is intentionally honest about what's production-ready vs scope-cut.
