# KeeperHub × SBO3L — concrete integration paths

> *"Yes, complementary layers are in scope (more so if they integrate or can be merged into KH). … We prefer real integrations over demos. Something we can actually merge or build on scores much higher than a polished mock."* — Luca, KeeperHub team (paraphrased from a hackathon Discord exchange).

This document is the answer to that prompt. It is **not** a marketing pitch; it is a list of **specific shapes** the KeeperHub team could adopt or build on, each with a pointer to the place in this repo where the corresponding work lives. Every shape is independently small, independently reviewable, and explicitly scoped — you can take any subset (or none) without taking the rest.

The naming convention below is `IP-#` ("Integration Path #") so the team can reference items by number in office hours or PR review.

---

## TL;DR — five shapes, ranked by adoption-cost

| # | Shape | What you get | Where it lives today | Adoption cost |
|---|---|---|---|---|
| **IP-1** | **`sbo3l_*` upstream-proof envelope fields** on the workflow webhook | A KeeperHub `executionId` row links cryptographically to the upstream SBO3L decision, with no out-of-band correlation. | Documented end-to-end in [`docs/keeperhub-live-spike.md` §Wire format](keeperhub-live-spike.md) and [`FEEDBACK.md` §KeeperHub](../FEEDBACK.md). | Schema-level: 4–5 optional string fields. |
| **IP-2** | **Public submission/result envelope JSON Schema** | Third-party policy engines (us, others) validate locally before submission; mismatches surface at the policy boundary, not over the wire. | We propose schema shape; KeeperHub publishes canonical version. | Spec-only: one JSON Schema file under your docs. |
| **IP-3** | **`keeperhub.lookup_execution(execution_id)` MCP tool** | Operators / auditors connect a KeeperHub execution row directly to the upstream SBO3L audit bundle in one tool call. | Reference shape under [`docs/cli/audit-bundle.md`](cli/audit-bundle.md); a **functional `sbo3l-mcp` stdio JSON-RPC server** at [`crates/sbo3l-mcp/`](../crates/sbo3l-mcp/) (PR #46) already exposes the symmetric `sbo3l.audit_lookup` tool. | MCP tool definition + thin adapter on your side. |
| **IP-4** | **Standalone SBO3L adapter crate** | KeeperHub (or any agent framework) can depend on `sbo3l-keeperhub-adapter` and get a `GuardedExecutor` that posts signed receipts to a workflow webhook with the IP-1 envelope. | Lives today as [`crates/sbo3l-keeperhub-adapter/`](../crates/sbo3l-keeperhub-adapter/), re-exported by `sbo3l-execution` for back-compat; crates.io publication remains target. | Repo-level crate exists; KeeperHub reviews / optionally lists it. |
| **IP-5** | **SBO3L Passport capsule (`sbo3l.passport_capsule.v1`)** | A single self-contained JSON file per execution: APRP body, signed receipt, audit chain prefix, KeeperHub `executionId`, and (target) checkpoint. KeeperHub's audit log can attach the capsule URI as one extra string column. | Schema + verifier landed in PR [#42](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/42); productisation tracked in [`docs/product/SBO3L_PASSPORT_BACKLOG.md`](product/SBO3L_PASSPORT_BACKLOG.md). | Storage-level: one URI column in your execution row, OR full-bundle storage if you want it inline. |

The rest of this document expands each path with concrete pointers, schemas, and the smallest reviewable PR shape.

---

## IP-1 — `sbo3l_*` upstream-proof envelope fields

**The problem you would solve.** Today, an auditor reading a KeeperHub execution row has no cryptographic link back to the policy decision that approved it. They have to trust whoever produces the row to correlate honestly with whatever upstream system authorised the action. With four (target: five) `sbo3l_*` fields on the submission envelope, an offline auditor can take a KeeperHub execution log line, a SBO3L audit bundle, and verify end-to-end that the executed action is the one SBO3L signed off on — without trusting either side.

**The fields, with semantics.**

| Field | Type | Semantics |
|---|---|---|
| `sbo3l_request_hash` | hex SHA-256 | JCS-canonical (RFC 8785) SHA-256 of the APRP body. SBO3L's canonical request hash. |
| `sbo3l_policy_hash` | hex SHA-256 | Canonical hash of the policy that authorised the action. Drift means the same agent produced this request under a different rulebook. |
| `sbo3l_receipt_signature` | hex Ed25519 | Signature on the policy receipt. Verifiable against the receipt-signer pubkey published in the Passport / capsule / ENS. |
| `sbo3l_audit_event_id` | ULID | Position of the decision in SBO3L's hash-chained audit log. Lets the auditor pull the chain prefix and re-derive `event_hash`. |
| `sbo3l_passport_capsule_hash` (target) | hex SHA-256 | Content hash of the Passport capsule once IP-5 lands on `main`. Optional today; first-class once Passport ships. |

**Where it lives in our repo.**

- Wire format sketch: [`docs/keeperhub-live-spike.md` §Wire format the adapter intends to send](keeperhub-live-spike.md).
- Concrete `KeeperHubLiveConfig` + `execute_live` shape: [`docs/keeperhub-live-spike.md` §Target shape](keeperhub-live-spike.md).
- Builder feedback covering the same fields: [`FEEDBACK.md` §KeeperHub → Suggested improvements](../FEEDBACK.md).

**Smallest adoption shape on KeeperHub side.** The fields are optional strings on the workflow webhook submission body. KeeperHub doesn't have to validate them. You only have to **echo them back** on `executionId → status` lookups (or in workflow run logs) so an auditor can fetch them later. That single passthrough turns IP-1 from "we put fields in" into "any auditor can re-verify."

---

## IP-2 — Public submission/result envelope JSON Schema

**The problem you would solve.** Today the workflow webhook contract is documented through the in-product workflow editor. Third-party policy engines (us, but also future builders) cannot validate locally before posting; mismatches surface as 4xx responses instead of policy-boundary errors with stable error codes. A published JSON Schema 2020-12 file under `docs.keeperhub.com/schemas/` (or similar) makes the contract first-class.

**Concrete shape we'd validate against today.**

```jsonc
// what SBO3L's KeeperHubExecutor::live() would post
{
  "$schema": "https://docs.keeperhub.com/schemas/workflow_submission_v1.json",
  "aprp":              { /* JCS-canonical APRP body */ },
  "policy_receipt":    { /* signed PolicyReceipt JSON */ },
  "sbo3l_request_hash":      "…",
  "sbo3l_policy_hash":       "…",
  "sbo3l_receipt_signature": "…",
  "sbo3l_audit_event_id":    "evt-…",
  "sbo3l_passport_capsule_hash": "…"   // optional, target
}
```

**Where it lives in our repo.** Our own contracts are under [`schemas/`](../schemas/) — six JSON Schema 2020-12 files (`aprp_v1`, `policy_v1`, `policy_receipt_v1`, `decision_token_v1`, `audit_event_v1`, `x402_v1`) plus an OpenAPI 3.1 spec at [`docs/api/openapi.json`](api/openapi.json). They are validated in CI by [`scripts/validate_schemas.py`](../scripts/validate_schemas.py) and [`scripts/validate_openapi.py`](../scripts/validate_openapi.py). We are happy to PR a draft `workflow_submission_v1.json` against KeeperHub's docs repo if that is useful.

**Smallest adoption shape on KeeperHub side.** A single JSON Schema file under your docs, plus one paragraph naming where the canonical version lives. We will validate against it on every commit; we will file issues if we find mismatches.

---

## IP-3 — `keeperhub.lookup_execution(execution_id)` MCP tool

**The problem you would solve.** Once an agent has submitted an action and gotten an `executionId`, the operator needs to reconcile that id against (a) the KeeperHub status (`submitted` / `running` / `succeeded` / `failed`), (b) the SBO3L audit-bundle position that authorised it, and (c) any echoed `sbo3l_*` fields. Today the operator has to compose three different surfaces. An MCP tool collapses that into one call any MCP-aware client (Claude, Cursor, SBO3L's own operator console) can make.

**Tool signature.**

```jsonc
// keeperhub MCP server registers:
{
  "name": "keeperhub.lookup_execution",
  "description": "Look up status + run-log pointer + echoed upstream proof fields for a given executionId.",
  "input_schema": {
    "type": "object",
    "required": ["execution_id"],
    "properties": {
      "execution_id": { "type": "string" }
    }
  },
  "output_schema": {
    "type": "object",
    "properties": {
      "status":     { "enum": ["submitted","running","succeeded","failed"] },
      "run_log_url": { "type": "string", "format": "uri" },
      "submitted_at": { "type": "string", "format": "date-time" },
      "sbo3l_request_hash":      { "type": "string", "pattern": "^[0-9a-f]{64}$" },
      "sbo3l_policy_hash":       { "type": "string", "pattern": "^[0-9a-f]{64}$" },
      "sbo3l_receipt_signature": { "type": "string", "pattern": "^[0-9a-f]{128}$" },
      "sbo3l_audit_event_id":    { "type": "string", "pattern": "^evt-[0-7][0-9A-HJKMNP-TV-Z]{25}$" }
    }
  }
}
```

**Where it lives in our repo.** Our MCP tool surface is at [`crates/sbo3l-mcp/`](../crates/sbo3l-mcp/) — **functional on `main` from PR #46** (Passport P3.1). The sister SBO3L MCP tool — `sbo3l.audit_lookup(audit_event_id)` — is **already implemented** and takes a `sbo3l_audit_event_id` + receipt + signer pubkeys, returning a verifiable `sbo3l.audit_bundle.v1`. Judge-facing walk-through with a verbatim request/response example is at [`docs/mcp-integration-guide.md`](mcp-integration-guide.md) (Passport P3.2). Calling both tools in sequence lets any MCP client cross-verify a KeeperHub execution against a SBO3L audit bundle in one conversational round; the KeeperHub side of that pair is the IP-3 ask.

**Smallest adoption shape on KeeperHub side.** Adding `keeperhub.lookup_execution` to your MCP server. The schema above is a starting draft we'd happily refine on PR review.

---

## IP-4 — Standalone SBO3L adapter crate

**The problem you would solve.** SBO3L's `KeeperHubExecutor` adapter is now isolated in a one-internal-dependency Rust crate. If KeeperHub wants third-party agent frameworks to bring their own policy layer, they need a single dependency they can add (or, in TypeScript, `npm install`) without taking the rest of the SBO3L workspace. The adapter crate makes that independently consumable.

**Target crate shape.**

```
sbo3l-keeperhub-adapter/
  Cargo.toml         # publishable as sbo3l-keeperhub-adapter
  src/
    lib.rs            # pub use { KeeperHubExecutor, GuardedExecutor }
    config.rs         # KeeperHubLiveConfig::from_env()
    envelope.rs       # IP-1 sbo3l_* fields helper
  examples/
    submit_signed_receipt.rs
  README.md           # 50-line how-to
```

**Where it lives in our repo today.** [`crates/sbo3l-keeperhub-adapter/`](../crates/sbo3l-keeperhub-adapter/) contains the `KeeperHubExecutor` impl plus README, changelog and an example. `sbo3l-execution` re-exports it for back-compat. The trait it implements (`GuardedExecutor`) is the only public surface needed; no `sbo3l-policy` / `sbo3l-storage` / `sbo3l-server` types leak into the adapter signature.

**Smallest adoption shape on KeeperHub side.** A line on your "integrations" page: *"Bring your own policy layer — see the `sbo3l-keeperhub-adapter` crate."* You don't have to maintain the crate; we do. We just need the namespace blessing before crates.io publication.

---

## IP-5 — SBO3L Passport capsule

**The problem you would solve.** Auditors who are not running a SBO3L daemon want a single self-contained file they can verify offline. The Passport capsule packages everything an offline auditor needs — APRP body, signed receipt, audit-chain prefix, KeeperHub `executionId`, optional checkpoint — into one JSON file with a published JSON Schema and a verifier CLI (`sbo3l passport verify`).

**Capsule shape (v1).** The canonical schema lives at [`schemas/sbo3l.passport_capsule.v1.json`](../schemas/sbo3l.passport_capsule.v1.json). Top-level fields:

```jsonc
{
  "schema":        "sbo3l.passport_capsule.v1",
  "generated_at":  "2026-…Z",
  "agent":         { /* ENS-style identity: agent_id, ens_name, resolver, records map */ },
  "request":       { /* APRP body + canonical request_hash + idempotency/nonce */ },
  "policy":        { /* policy_hash + version + activated_at + source */ },
  "decision":      { /* result + matched_rule + deny_code + embedded signed receipt + 128-hex signature */ },
  "execution":     { /* executor + mode + execution_ref + status + sponsor_payload_hash + live_evidence */ },
  "audit":         { /* audit_event_id + prev_event_hash + event_hash + bundle_ref + optional embedded checkpoint */ },
  "verification":  { /* doctor_status + offline_verifiable + live_claims */ }
}
```

Every hash field is constrained to `^[0-9a-f]{64}$`; signatures to `^[0-9a-f]{128}$`; `additionalProperties: false` at every object level. The `audit.bundle_ref` field points back at the `sbo3l.audit_bundle.v1` artefact for callers who want the full hash-chain prefix; the audit chain itself is not duplicated inside the capsule.

**Where it lives in our repo.** Schema + verifier CLI ship in PR [#42 (`feat: add Passport capsule schema and verifier`)](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/pull/42). Productisation is tracked in [`docs/product/SBO3L_PASSPORT_BACKLOG.md`](product/SBO3L_PASSPORT_BACKLOG.md). The Passport one-pagers in [`docs/partner-onepagers/`](partner-onepagers/) describe what each sponsor needs to do for capsule integration.

**Smallest adoption shape on KeeperHub side.** One optional column on the execution row: `sbo3l_passport_uri` (string, default null). When set, it points at a Passport capsule (HTTP(S) or `s3://` or `0g://`). Anyone who wants to audit the execution can fetch the URI to a local file and run `sbo3l passport verify --path <file>` — no SBO3L daemon needed.

---

## How these compose

The five paths stack: any subset gives strictly more value than any smaller subset, and adopting all five gives **end-to-end offline auditability** of every KeeperHub execution that flowed through SBO3L.

```
agent
  ↓ APRP
[SBO3L]
  ↓ signed PolicyReceipt + IP-1 envelope fields
[KeeperHub workflow webhook]              ← IP-2 schema validates here
  ↓ executionId
[KeeperHub execution row]                  ← IP-3 MCP tool reads here
  └ optional sbo3l_passport_uri column   ← IP-5 capsule pointer
```

Anywhere in this chain, an auditor with the right keys can reconstruct *what was authorised*, *who authorised it*, *which policy applied*, and *where the audit chain says it sits* — without trusting any single party.

---

## What this document is NOT

- **Not a claim that any of IP-1 through IP-5 is implemented end-to-end with a live KeeperHub backend in this build.** The SBO3L side has the schemas, the adapter, the audit-bundle codec, the Passport capsule schema and verifier; the live network call to KeeperHub is gated behind unblocking the open questions in [`docs/keeperhub-live-spike.md` §Open questions for the KeeperHub team](keeperhub-live-spike.md). Today the demo always constructs `KeeperHubExecutor::local_mock()`.
- **Not a request for special treatment.** Every claim above is reproducible from a fresh clone in ~5 seconds: `bash demo-scripts/run-openagents-final.sh` (vertical demo, 13 gates) and `bash demo-scripts/run-production-shaped-mock.sh` (operator surface, 26 real / 0 mock / 1 skipped tally). The mock is honestly disclosed in every output.
- **Not a marketing landing page.** This doc lives in `docs/` next to the existing CLI and partner one-pager docs, owns its own filename, and links into the actual Rust source where each shape lives.

---

## What we are asking for, in one sentence

If any of IP-1 through IP-5 is something the KeeperHub team would *consider* adopting, we would like to file a PR or issue against the appropriate KeeperHub repo with a fully-reviewable proposal — and to use office hours to confirm which path (if any) is the right one to start with.

---

## Pointers in this repo

- Adapter source: [`crates/sbo3l-keeperhub-adapter/`](../crates/sbo3l-keeperhub-adapter/)
- Live-integration spike: [`docs/keeperhub-live-spike.md`](keeperhub-live-spike.md)
- Builder feedback: [`FEEDBACK.md` §KeeperHub](../FEEDBACK.md)
- KeeperHub partner one-pager: [`docs/partner-onepagers/keeperhub.md`](partner-onepagers/keeperhub.md)
- Audit-bundle CLI reference: [`docs/cli/audit-bundle.md`](cli/audit-bundle.md)
- SBO3L Passport product plan: [`docs/product/SBO3L_PASSPORT_BACKLOG.md`](product/SBO3L_PASSPORT_BACKLOG.md), [`docs/product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md`](product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md)
- MCP tool surface (functional on `main` from PR #46): [`crates/sbo3l-mcp/`](../crates/sbo3l-mcp/) — judge-facing walk-through at [`docs/mcp-integration-guide.md`](mcp-integration-guide.md).
- Schemas: [`schemas/`](../schemas/) (six JSON Schema 2020-12 files)
- OpenAPI 3.1 contract: [`docs/api/openapi.json`](api/openapi.json)
