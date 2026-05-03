# Don't give your agent a wallet. Don't make KeeperHub guess what's authorized either.

> **Composing SBO3L's policy boundary with KeeperHub's execution layer.**
>
> *For: agent-platform engineers + KeeperHub workflow authors evaluating end-to-end safety.*

---

A research agent sees a paid API endpoint, decides "this is worth $0.05," and reaches for a tool to fire the payment. Two things have to happen between intent and execution:

1. **Decide:** is this payment authorized? Within budget? Recipient allowlisted? Right chain? Right risk class?
2. **Execute:** translate the authorized intent into a signed transaction (or a webhook, or an x402 call) and dispatch it.

Most "agent payment" stacks today try to do both in one product. SBO3L doesn't. KeeperHub doesn't. Together they form the cleanest composition we've shipped — a policy boundary that emits a signed receipt, and an execution layer that consumes the receipt and actually fires the call.

This post walks through the composition: why two products beat one monolith, what the receipt contract between them looks like, and the five integration paths (IP-1..IP-5) that compose them tighter still. By the end you'll be able to wire `@sbo3l/langchain-keeperhub` into your own agent in five lines and reason about every byte of evidence the round-trip emits.

---

## 1. The composability story — agent intent → SBO3L decide → KH execute

Why split the decision and the execution into two products?

Because they have **different audiences** and **different operational contracts**.

The decision layer is the boundary between "what the agent wants to do" and "what the operator allows the agent to do." Its audience is the operator: a CFO, a compliance officer, an SRE on call. They edit YAML to express budget caps, allowlist providers, set risk thresholds. They want the policy engine to be auditable, deterministic, and slow-moving. They want every decision logged in a tamper-evident way. They don't want it coupled to whichever execution backend is in fashion this quarter.

The execution layer is the bridge between "an authorized payment intent" and "the rails it actually settles on." Its audience is the developer of the workflow: someone wiring KeeperHub up to a Stripe webhook, an x402 endpoint, a Uniswap router, a Discord bot. They want the execution layer to be flexible, fast-evolving, integration-rich. They want first-class support for the latest workflow runner, not a slow decision engine slowing them down.

Bundle them and you compromise both. The decision engine starts shipping integrations to chase the latest payment rail. The execution layer starts adding policy DSLs to chase compliance reviews. Both products get worse at their core jobs.

Split them and each can grow on its own clock — *if* the contract between them is right.

---

## 2. The signed receipt as the contract between layers

The contract is a **signed PolicyReceipt**.

When SBO3L decides on an APRP (Agent Payment Request Protocol body), it emits a 14-field receipt:

```json
{
  "receipt_type": "sbo3l.policy_receipt.v1",
  "version": 1,
  "agent_id": "research-agent-01",
  "decision": "allow",
  "deny_code": null,
  "matched_rule_id": "allow-low-risk-x402-keeperhub",
  "request_hash": "c0bd2fab…",     // 32-byte SHA-256 of canonicalised APRP
  "policy_hash": "e044f13c…",      // hash of the policy YAML that decided
  "policy_version": 1,
  "audit_event_id": "evt-01HTAW…",  // ULID into the hash-chained audit log
  "execution_ref": null,           // populated by the executor on success
  "issued_at": "2026-04-29T10:00:00Z",
  "expires_at": null,
  "signature": {
    "algorithm": "ed25519",
    "key_id": "decision-signer-v1",
    "signature_hex": "1f…128 chars…"
  }
}
```

Three things about this receipt make it the right contract:

1. **It's content-addressable.** The `request_hash` pins the exact APRP bytes that were decided on. KeeperHub can re-derive the hash from the body it receives and refuse to execute if it doesn't match. No "the agent edited the request between decision and execution" attack.

2. **It's offline-verifiable.** Anyone with the policy signer's public key can verify the signature without contacting SBO3L. KH can verify before executing. An auditor reading the audit log months later can verify too. There's no "trust SBO3L" step.

3. **It carries the audit pointer.** `audit_event_id` references a node in SBO3L's hash-chained Ed25519 audit log. KH can echo this ID back on its execution row, giving an auditor a single ID to walk both directions: the decision side and the execution side, end-to-end.

This receipt is the only thing the two layers exchange. SBO3L doesn't know what KeeperHub's webhook URL is. KeeperHub doesn't know what policy YAML SBO3L is running. Each side evolves freely as long as the receipt schema holds.

---

## 3. The 5 IP paths (re-explained from scratch)

Once you have a receipt-as-contract, the next question is: how tight can the composition get? Five paths, each independently shippable:

**IP-1 — `sbo3l_*` upstream-proof envelope fields on the workflow webhook.**
SBO3L's KH adapter posts the signed receipt's `request_hash`, `policy_hash`, `policy_version`, `audit_event_id`, and `signature_hex` as five optional `sbo3l_*` fields alongside whatever other body the workflow expects. KH stores them. An auditor reading a KH execution row sees the cryptographic link upstream without having to query SBO3L. Status today: **shipped on the SBO3L side** as `sbo3l_keeperhub_adapter::build_envelope`. Pending KH-side schema adoption (issue [KeeperHub/cli#50](https://github.com/KeeperHub/cli/issues/50)).

**IP-2 — Public submission/result envelope JSON Schema.**
A Draft 2020-12 schema that documents the bidirectional wire shape. Adapter authors stop reverse-engineering response payloads from `curl -v`. Status: filed as issue [KeeperHub/cli#48](https://github.com/KeeperHub/cli/issues/48); SBO3L's adapter validates against the schema once published.

**IP-3 — `keeperhub.lookup_execution(execution_id)` MCP tool.**
A symmetric MCP tool that lets a downstream auditor query an execution's status + run-log + sbo3l_* fields without raw HTTP plumbing. Status: **shipped on the SBO3L side** as `sbo3l.audit_lookup` in `sbo3l-mcp`; pending KH-side MCP tool definition (issue [KeeperHub/cli#49](https://github.com/KeeperHub/cli/issues/49)).

**IP-4 — Standalone `sbo3l-keeperhub-adapter` Rust crate.**
Any third-party agent framework can `cargo add sbo3l-keeperhub-adapter` and get a `GuardedExecutor` that posts the IP-1 envelope, with no transitive dependency on `sbo3l-server`, `sbo3l-policy`, or `sbo3l-storage`. Status: **shipped** at `crates/sbo3l-keeperhub-adapter/`, [live on crates.io at v1.2.0](https://crates.io/crates/sbo3l-keeperhub-adapter).

**IP-5 — SBO3L Passport capsule URI on the execution row.**
A single optional string column on KH's execution table — the URI to a self-contained verifiable bundle (APRP + receipt + audit segment + executor evidence + verification metadata). With this column, an auditor can reconstruct everything offline from one URL. Status: **capsule schema + verifier shipped** on the SBO3L side; pending KH-side column adoption.

Stacking all five gives end-to-end offline auditability of every KeeperHub execution that flowed through SBO3L. An auditor with the right keys can reconstruct who authorized what, under which policy, when, and where the audit chain says it sits — without trusting any single party. Two different products, one verifiable trail.

---

## 4. End-to-end demo

Five lines in TypeScript:

```ts
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lKeeperHubTool } from "@sbo3l/langchain-keeperhub";

const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
const tool = sbo3lKeeperHubTool({ client });
// pass `tool` (or wrap as DynamicTool) into your LangChain agent's tool list
```

What happens when the agent calls `tool.func(JSON.stringify(aprp))`:

1. **POST to SBO3L daemon** at `/v1/payment-requests` with the APRP body
2. **SBO3L decides** allow / deny / requires_human against the loaded policy + budget + nonce + provider trust list
3. **On allow:** SBO3L's `executor_callback` hands the signed receipt to the daemon-side KeeperHub adapter (`SBO3L_KEEPERHUB_WEBHOOK_URL` + `SBO3L_KEEPERHUB_TOKEN` env-var-configured)
4. **KH adapter POSTs the IP-1 envelope** to the workflow webhook, captures `executionId`
5. **Tool returns** `{decision, kh_workflow_id_advisory, kh_execution_ref, audit_event_id, request_hash, policy_hash, deny_code}`

The signed envelope on the wire (curl trace, real values redacted):

```http
POST /api/workflows/m4t4cnpmhv8qquce3bv3c/webhook HTTP/1.1
Host: app.keeperhub.com
Authorization: Bearer wfb_<token>
Content-Type: application/json

{
  "agent_id": "research-agent-01",
  "intent": "purchase_api_call",
  "sbo3l_request_hash": "c0bd2fab…",
  "sbo3l_policy_hash": "e044f13c…",
  "sbo3l_policy_version": 1,
  "sbo3l_audit_event_id": "evt-01HTAW…",
  "sbo3l_signature_hex": "1f…"
}

HTTP/1.1 200 OK
Content-Type: application/json

{ "executionId": "kh-01HTAWX5K3R8YV9NQB7C6P2DGZ" }
```

That's the full round-trip. A single `curl` capture proves SBO3L decided, KH executed, and the receipt connects them.

---

## 5. The 15 GitHub issues — what we'd want KeeperHub to adopt

Building the composition end-to-end surfaced 15 concrete, actionable asks on the KeeperHub side. We filed them all on [KeeperHub/cli](https://github.com/KeeperHub/cli/issues?q=is%3Aissue+author%3AB2JK-Industry):

**Round 1 (#47–#51)** — couldn't-get-it-working frictions: token-prefix split, envelope schema, executionId lookup, sbo3l_* fields adoption, idempotency-key dedup.

**Round 2 (#52–#56)** — post-integration concerns: HTTP error code catalog, public mock fixture suite, webhook timeout SLO publication, schema versioning headers, max payload size documentation.

**Round 3 (#58–#62)** — production-grade reliability concerns: HMAC-SHA256 signature for reverse-direction webhooks, workflow versioning + back-compat policy, response envelope JSON Schema, rate-limiting headers, delivery guarantees documentation.

Each issue carries a worked reproduction (`curl` invocation or unit test), a citation to the exact line in our adapter where the friction surfaces, and a proposed shape for the fix. Five of them have **companion draft PRs** on our repo showing the consumer-side adapter change ready to ship the day KH lands the upstream contract.

The fastest unlock for the broader adapter ecosystem: **#48** (envelope schema, R1) + **#52** (error catalog, R2) + **#55** (schema-version header, R2). Three docs/platform changes that together unblock every subsequent adapter author.

---

## 6. Looking forward — joint product roadmap

Post-hackathon, three directions worth pursuing together:

**Phase 1 — IP-1 + IP-2 land on KeeperHub.** A workflow author can opt into the `sbo3l_*` envelope fields with one checkbox. The submission/result envelope JSON Schema is published. Adapters across the ecosystem (Devendra's `langchain-keeperhub`, ours, future ones) standardise on the schema instead of reverse-engineering it.

**Phase 2 — IP-3 + IP-5 ship.** The MCP tool surface for `keeperhub.lookup_execution` lets agents and auditors query in a vendor-neutral way. The Passport capsule URI column on the execution row makes "show me the proof" a one-click download from the KH UI.

**Phase 3 — Multi-tenant trust DNS.** Every agent that runs through SBO3L → KH gets its own ENS name (e.g. `research-agent-01.sbo3lagent.eth`). A workflow author looking at their execution log sees agent names, not opaque IDs. The trust commitments behind each name (policy hash, signing key, deployer attestation) are resolvable from a single DNS-style query.

This isn't a replacement for either product. It's a clean composition where each layer keeps doing what it's best at, and the contract between them carries enough cryptographic proof to satisfy the most paranoid auditor.

If you're shipping an agent today, you already have an unsolved gate-then-execute problem. Start with the composition. Compose more later.

---

*Written by Daniel Babjak ([@B2JK-Industry](https://github.com/B2JK-Industry)) for ETHGlobal Open Agents 2026. Companion to the [SBO3L → KeeperHub Builder Feedback submission](../submission/bounty-keeperhub-builder-feedback.md). Code: [`@sbo3l/langchain-keeperhub`](https://www.npmjs.com/package/@sbo3l/langchain-keeperhub) (npm), [`sbo3l-langchain-keeperhub`](https://pypi.org/project/sbo3l-langchain-keeperhub/) (PyPI — pending Trusted Publisher registration), [`sbo3l-keeperhub-adapter`](https://crates.io/crates/sbo3l-keeperhub-adapter) (crates.io). All 15 KH issues + 5 companion draft PRs catalogued in [docs/proof/kh-builder-feedback-2026-05-03.md](kh-builder-feedback-2026-05-03.md).*
