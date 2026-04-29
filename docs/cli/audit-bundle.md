# `sbo3l audit export` / `verify-bundle`

> *SBO3L does not just decide. It leaves behind verifiable proof — directly from its daemon storage.*

A SBO3L decision already produces three signed artefacts: the **policy receipt** (returned in the `POST /v1/payment-requests` response), the **audit event** (appended to the hash-chained log), and the **chain linkage** between successive events. The bundle commands package those artefacts into a single self-contained JSON file that anyone can re-verify offline.

## Export

The export command takes a signed receipt plus an audit chain source. **Exactly one** of `--chain` or `--db` must be supplied:

### From SQLite storage (production-style)

```bash
sbo3l audit export \
  --receipt   path/to/receipt.json \
  --db        path/to/sbo3l.sqlite \
  --receipt-pubkey <hex> \
  --audit-pubkey   <hex> \
  --out       path/to/bundle.json
```

This is the realistic path: a SBO3L daemon writes its audit log to SQLite (`SBO3L_DB`); the receipt comes back to the agent in the `POST /v1/payment-requests` response. The CLI opens the same SQLite file, slices the chain prefix from genesis through `receipt.audit_event_id`, **pre-flights the chain integrity** with `verify_chain` against `--audit-pubkey`, **pre-flights the receipt signature** against `--receipt-pubkey`, and only then writes the bundle. A failure on any of those checks aborts before the bundle file is created — so a downstream consumer never sees an unverifiable bundle.

### From a JSONL chain file

```bash
sbo3l audit export \
  --receipt   path/to/receipt.json \
  --chain     path/to/chain.jsonl \
  --receipt-pubkey <hex> \
  --audit-pubkey   <hex> \
  --out       path/to/bundle.json
```

Use this when you already have an exported chain JSONL (for example, from a fixture or an air-gapped environment without the live daemon).

### Common flags

- `--receipt` — the `receipt` field from a `POST /v1/payment-requests` response, saved as JSON.
- `--chain` — the audit log as JSONL (one `SignedAuditEvent` per line). Must include the genesis event (seq=1) through the event referenced by `receipt.audit_event_id`, in seq order. **Mutually exclusive with `--db`.**
- `--db` — path to the SBO3L daemon's SQLite store. The CLI reads the chain prefix through `receipt.audit_event_id` directly. **Mutually exclusive with `--chain`.**
- `--receipt-pubkey` / `--audit-pubkey` — 32-byte Ed25519 public keys (hex). For the hackathon dev signers these are the deterministic seeds wired into `AppState::new`; production deployments substitute via `AppState::with_signers(...)`.
- `--out` — output file. If omitted, the bundle is written to stdout (good for piping into `jq`).

### Failure modes (DB-backed export)

| Condition | Behaviour |
|---|---|
| `--db` path does not exist | Exits non-zero, error names the missing path; no bundle written. |
| `receipt.audit_event_id` not present in DB | Exits non-zero, error echoes the missing id; no bundle written. |
| Audit chain in DB is tampered (e.g. a row was rewritten outside `audit_append`) | Pre-flight `verify_chain` rejects; exits non-zero with `audit chain pre-flight failed: …`; no bundle written. |
| Wrong `--audit-pubkey` (does not match the daemon's audit signer) | Same path as tampered chain — pre-flight signature check fails. |
| Wrong `--receipt-pubkey` | Pre-flight receipt-signature check fails before the bundle is written. |

## Verify

```bash
sbo3l audit verify-bundle --path path/to/bundle.json
```

Re-derives every claim in the bundle:
1. Verifies the receipt signature under the recorded receipt-signer public key.
2. Verifies the standalone audit event signature.
3. Re-runs `verify_chain` over `audit_chain_segment` (recomputes every `event_hash`, walks `prev_event_hash`, re-verifies every signature).
4. Confirms `receipt.audit_event_id` matches the standalone event and is present in the chain.
5. Re-derives the bundle's `summary` block and compares it field-by-field to the body, so a tampered summary cannot misrepresent what the receipt or chain actually says.

Exit codes:
- `0` — bundle verified.
- `1` — verification failed (signature, chain linkage, or summary mismatch). A short diagnostic is printed to stderr.
- `2` — I/O or JSON-parse error.

## Bundle shape

```json
{
  "bundle_type": "sbo3l.audit_bundle.v1",
  "version": 1,
  "exported_at": "2026-04-28T...Z",
  "receipt":            { /* PolicyReceipt with embedded Ed25519 signature */ },
  "audit_event":        { /* SignedAuditEvent referenced by receipt.audit_event_id */ },
  "audit_chain_segment":[ /* SignedAuditEvent[] from genesis through audit_event */ ],
  "verification_keys":  { "receipt_signer_pubkey_hex": "...", "audit_signer_pubkey_hex": "..." },
  "summary":            { /* decision, deny_code, request_hash, policy_hash, audit_event_id, audit_event_hash, chain root + latest */ }
}
```

## What the bundle does and does not prove

**Proves**, given that you trust the two recorded public keys:
- the receipt's `request_hash`, `policy_hash`, `decision`, `deny_code` and `audit_event_id` were signed by the receipt-signer key (any tampering breaks the signature);
- the standalone audit event was signed by the audit-signer key, has a matching `event_hash`, and sits at a specific position in the audit chain;
- every event in the chain segment links cleanly to its predecessor and was signed by the same audit-signer key.

**Does not prove** (out of scope for the v1 bundle):
- *who* the recorded public keys belong to — that is an identity question handled separately by the ENS adapter (`sbo3l-identity`);
- the absence of audit events past `audit_chain_segment.last()` — exporting a partial chain is fine for "this decision happened", but completeness proofs need a Merkle commitment that we have not built yet;
- agent reasoning before the request reached SBO3L — APRP is a wire-level contract, not a behavioural one.

## Limitations

- The chain segment must be the full prefix from genesis. Pruned / Merkle-proof variants are tracked separately.
- The bundle does not include the original APRP payload — it includes the canonical `request_hash` only, which is enough to *re-verify* a request the verifier already has but not enough to *reconstruct* the request from the bundle alone. (Future revision: optional embedded APRP.)
- No revocation / expiry semantics on the bundle itself; if you need expiring proofs, set `PolicyReceipt.expires_at` upstream.
- The `--db` mode opens the SQLite file in normal mode (not read-only). Concurrent access against a running daemon is safe in WAL mode but the CLI does not enforce read-only; running it against a live daemon is fine but document it as such for production audits.
