# `mandate audit export` / `verify-bundle`

> *Mandate does not just decide. It leaves behind verifiable proof.*

A Mandate decision already produces three signed artefacts: the **policy receipt** (returned in the `POST /v1/payment-requests` response), the **audit event** (appended to the hash-chained log), and the **chain linkage** between successive events. The bundle commands package those artefacts into a single self-contained JSON file that anyone can re-verify offline.

## Export

```bash
mandate audit export \
  --receipt   path/to/receipt.json \
  --chain     path/to/chain.jsonl \
  --receipt-pubkey <hex> \
  --audit-pubkey   <hex> \
  --out       path/to/bundle.json
```

- `--receipt` — the `receipt` field from a `POST /v1/payment-requests` response, saved as JSON.
- `--chain` — the audit log as JSONL (one `SignedAuditEvent` per line). Must include the genesis event (seq=1) through the event referenced by `receipt.audit_event_id`, in seq order.
- `--receipt-pubkey` / `--audit-pubkey` — 32-byte Ed25519 public keys (hex). For the hackathon dev signers these are the deterministic seeds wired into `AppState::new`; production deployments substitute via `AppState::with_signers(...)`.
- `--out` — output file. If omitted, the bundle is written to stdout (good for piping into `jq`).

## Verify

```bash
mandate audit verify-bundle --path path/to/bundle.json
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
  "bundle_type": "mandate.audit_bundle.v1",
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
- *who* the recorded public keys belong to — that is an identity question handled separately by the ENS adapter (`mandate-identity`);
- the absence of audit events past `audit_chain_segment.last()` — exporting a partial chain is fine for "this decision happened", but completeness proofs need a Merkle commitment that we have not built yet;
- agent reasoning before the request reached Mandate — APRP is a wire-level contract, not a behavioural one.

## Limitations

- The chain segment must be the full prefix from genesis. Pruned / Merkle-proof variants are tracked separately.
- The bundle does not include the original APRP payload — it includes the canonical `request_hash` only, which is enough to *re-verify* a request the verifier already has but not enough to *reconstruct* the request from the bundle alone. (Future revision: optional embedded APRP.)
- No revocation / expiry semantics on the bundle itself; if you need expiring proofs, set `PolicyReceipt.expires_at` upstream.
