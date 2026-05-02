# Phase 3 — Production hardening + ecosystem composition (Days 61-100+)

> **Goal:** turn SBO3L from "the trust layer for autonomous AI agents — at hackathon scale" into "the trust layer for autonomous AI agents — at production scale, multi-tenant, composable across protocols, compliance-shaped." Exit gate locks **post-hackathon production-readiness signal** for partner-facing pilots.

Phase 3 is structured as **eight sub-areas** (3.1 — 3.8). Each sub-area has a scope, dependency list, and exit-gate criteria written so a third party can verify pass/fail without ambiguity. Sub-areas are mostly parallelisable; the dependency graph is explicit per-area.

This document is the AC source-of-truth. Tickets land per-area as the cascade progresses; per-AC pass/fail is tracked in `docs/win-backlog/phase-3-readiness.md` (created post-Phase-2 close).

## Phase 3 sub-area index

| Area | Title | Effort target | Depends on |
|---|---|---|---|
| **3.1** | Audit chain anchoring (on-chain + off-chain checkpoints) | ~30h | Phase 2 closed (T-3-3 fleet, T-4-1 CCIP) |
| **3.2** | Multi-tenant production isolation | ~40h | F-5 KMS, audit V010 (#208) |
| **3.3** | Agent marketplace (publish + discover + reputation gating) | ~25h | T-4-3 reputation, T-4-1 CCIP, T-3-4 cross-agent attestation |
| **3.4** | 10K TPS sustained-load performance | ~20h | F-3 idempotency state machine, F-2 budget store |
| **3.5** | Token-gated agent identity (NFT + time-window verification) | ~20h | T-4-2 ERC-8004 |
| **3.6** | Cross-protocol composition (single capsule spans N protocols) | ~30h | T-5-3 Smart Wallet, KH adapter, Uniswap adapter |
| **3.7** | Compliance posture (SOC 2 readiness + GDPR data subject paths) | ~25h | F-1 auth, F-5 KMS, audit chain (Phase 1 + 3.1) |
| **3.8** | Self-hosted operator console (multi-tenant routing + RBAC) | ~30h | 3.2 multi-tenant, hosted-app slice 3 |

**Total Phase 3 effort:** ~220h. Parallelisable across all 4 dev slots = ~28 days at the post-Phase-2 cascade velocity.

---

## [3.1] Audit chain anchoring

> **Why:** the audit chain is local-by-default (SQLite). For partner-facing audits at production scale, the chain head must be cryptographically anchored to a public chain — so a third party with `audit_root` and the agent's pubkey can prove the chain hasn't been silently rewritten.

### Scope

- On-chain anchor schedule: every N events OR every M minutes, whichever first. Default `N=1000, M=60`.
- Anchor target: ENS text record `sbo3l:audit_root` (already part of Phase 2) + EAS attestation on Sepolia/mainnet.
- Off-chain anchor: pinned to IPFS (or 0G storage for Track A) with content-hash committed on-chain.
- Cross-chain coverage: the same chain-head value must be verifiable against ENS (mainnet), EAS (Sepolia + mainnet), IPFS pin (CID).
- Anchor history retention: 30 days minimum on-chain history (cheap on Sepolia; mainnet bounded by gas budget).
- Backfill: command `sbo3l audit anchor-backfill --since <date>` walks unencrypted history + emits anchor events.

### Acceptance criteria

- [ ] `cargo run -p sbo3l-cli -- audit anchor --network sepolia` writes EAS attestation with `audit_root` matching `audit checkpoint create` output
- [ ] Anchor scheduler emits exactly one anchor event per `(N events, M minutes)` boundary (test via fast-forward fixture)
- [ ] `sbo3l audit anchor-history --network sepolia --since '30 days ago'` lists ≥ 1 anchor per day
- [ ] IPFS CID for the latest checkpoint resolves and matches `audit checkpoint verify` output
- [ ] Cross-chain consistency check: ENS `sbo3l:audit_root` byte-matches the latest EAS attestation byte-matches the latest IPFS CID content hash
- [ ] No mainnet anchor exceeds $5 gas at 50 gwei (gas budget documented per anchor)

### Exit gate

`bash demo-scripts/anchor-end-to-end.sh` runs anchor → wait → verify-from-three-sources → exit 0. Output captured to `demo-scripts/artifacts/anchor-cross-chain-evidence.json`.

---

## [3.2] Multi-tenant production isolation

> **Why:** a single SBO3L deployment must serve N agents from M organisations with cryptographic isolation. Today the audit chain is per-DB; per-tenant isolation arrived at the storage layer in #208 (V010). Production needs the rest of the surface.

### Scope

- **Audit-chain isolation:** `audit_*_for_tenant(tenant_id)` queries + tenant scope on every audit append (#208 lands the schema; this AC validates the surface).
- **Budget isolation:** budgets keyed on `(agent_id, tenant_id)`; cross-tenant reads denied with `policy.tenant_isolation_breach`.
- **KMS isolation:** per-tenant signer keys derived from a tenant root; one-tenant compromise must not leak another tenant's signing key.
- **Policy isolation:** per-tenant active policy with cross-tenant deny: agent in tenant A cannot reference policy from tenant B.
- **Idempotency isolation:** idempotency keys are scoped per-tenant; tenant A's `Idempotency-Key: foo` cannot collide with tenant B's `foo`.
- **Tenant scope queries:** `GET /v1/audit/events?tenant=foo` returns only that tenant's chain.

### Acceptance criteria

- [ ] `cargo test --test multi_tenant_isolation` covers all 5 isolation surfaces above with negative tests (tenant-A-reads-tenant-B → deny)
- [ ] Per-tenant audit chain has its own genesis event (no cross-tenant `prev_event_hash` linkage)
- [ ] KMS test: rotate tenant A's key; tenant B's signing keeps working unchanged
- [ ] HTTP API: `Authorization: Bearer <token>` carries tenant claim; mismatched tenant → 403 `tenant.isolation_breach`
- [ ] Postman / OpenAPI spec includes `X-Tenant-Id` header on every endpoint that reads tenant-scoped data
- [ ] Daniel-manual review: 2-tenant production simulation script runs both side-by-side for 1h with no cross-contamination in audit / budget / receipt

### Exit gate

`bash demo-scripts/multi-tenant-isolation.sh` runs the 2-tenant simulation + 5 negative-test assertions + output to `demo-scripts/artifacts/multi-tenant-evidence.json`.

---

## [3.3] Agent marketplace

> **Why:** at v1.2.0 close the project ships 60+ named agents on Sepolia (T-3-4 amplifier). Production needs a discovery surface where agents can publish capabilities, discover peers, and gate delegations on reputation thresholds.

### Scope

- **Publish:** `sbo3l agent publish --name foo.sbo3lagent.eth --capabilities <json>` emits ENS text-record update + indexed marketplace entry.
- **Discover:** `sbo3l agent search --capability x402-purchase --min-reputation 80 --max-results 10` returns ranked agents.
- **Reputation gating:** delegations to agents with reputation < threshold are denied with `policy.reputation_threshold_unmet`.
- **Marketplace endpoint:** `GET /v1/marketplace/agents` returns the registry; signed by SBO3L marketplace signer; clients verify against marketplace pubkey.
- **Anti-Sybil:** 1 ENS subname = 1 marketplace entry; reputation cannot be transferred between subnames.

### Acceptance criteria

- [ ] `sbo3l agent publish` emits ENS update + marketplace index entry; both visible via 2 separate verify paths
- [ ] `sbo3l agent search` returns N≥1 agent for a known-published capability
- [ ] Reputation gate: an agent with reputation 50 cannot delegate to an agent requiring threshold 80; deny code surfaces correctly
- [ ] Sybil test: cannot create marketplace entry for an ENS name without resolver-write permission (proven via tx-revert test)
- [ ] Marketplace endpoint signature verifies against the published marketplace pubkey
- [ ] Discovery freshness: published-now agent visible in search within 60s

### Exit gate

`bash demo-scripts/marketplace-end-to-end.sh` runs publish → search → reputation-gated delegation (allow + deny path) + output to `demo-scripts/artifacts/marketplace-evidence.json`.

---

## [3.4] 10K TPS sustained-load performance

> **Why:** today's daemon handles ~50 concurrent same-key POSTs cleanly (#102 + chaos-04). Production needs sustained 10K TPS with known-bound p99 latency, no memory leaks, no audit-chain growth runaway.

### Scope

- **Target:** 10K TPS sustained for 1h with p99 ≤ 50ms on commodity hardware (8-core, 32GB RAM, NVMe SSD)
- **Memory budget:** RSS growth ≤ 100MB over the hour (leak detection)
- **Audit-chain growth:** 36M events written to disk over 1h; no transactional throughput dip after 30M events
- **Idempotency cleanup:** garbage-collected rows older than 24h; idempotency-table size bounded
- **Nonce store retention:** purgeable after expiry-window + grace; size bounded
- **Profiling outputs:** flamegraph, perf top, sqlite EXPLAIN QUERY PLAN for hot queries

### Acceptance criteria

- [ ] `bash demo-scripts/load-10k-tps.sh` runs 1h sustained 10K TPS; reports p50/p95/p99/p999 latency + RSS growth + audit count
- [ ] p99 ≤ 50ms; p999 ≤ 200ms (on commodity hardware spec above)
- [ ] RSS growth ≤ 100MB
- [ ] No audit-chain throughput dip > 20% after 30M events (verified by 5-min throughput windows)
- [ ] `sbo3l-storage` exposes `idempotency_gc(now, ttl)` + `nonce_gc(now)` callable from CLI; both bounded-time queries
- [ ] Flamegraph captured to `demo-scripts/artifacts/load-10k-flamegraph.svg`; no single function > 30% of CPU

### Exit gate

`bash demo-scripts/load-10k-tps.sh` exits 0; artifacts in `demo-scripts/artifacts/load-10k-evidence/`.

---

## [3.5] Token-gated agent identity

> **Why:** some delegations should require the requesting agent to hold a specific NFT (e.g. partner-membership token). Token-gating + time-window verification turn ENS identity from "any name" to "this name AND holds X NFT".

### Scope

- **NFT verification:** policy rule `requires_nft(<contract>, <token_id>)` checks the agent's wallet at decision time.
- **Time-window verification:** rule `requires_held_for(<contract>, <token_id>, <duration>)` requires continuous holding for ≥ duration (computed via Transfer events).
- **Cache:** NFT-ownership lookups cached per-block; cache invalidated on Transfer events targeting the agent.
- **Multi-chain support:** NFT contract chain ID parametrised; mainnet + Sepolia + Polygon supported.

### Acceptance criteria

- [ ] Policy rule `requires_nft` parses + evaluates against fixture wallet
- [ ] `requires_held_for` rule rejects an agent that received the NFT within the duration window
- [ ] Cross-chain test: same agent holding NFT on mainnet but NOT Polygon → mainnet-required passes, Polygon-required denies
- [ ] Cache invalidation: Transfer event in fixture block → next decision reads fresh ownership
- [ ] Performance: NFT lookup adds ≤ 10ms to policy decision (cached) / ≤ 200ms (uncached, single RPC)

### Exit gate

`cargo test --test token_gated_identity` exits 0; covers the 5 ACs.

---

## [3.6] Cross-protocol composition

> **Why:** today a Passport capsule documents one agent action. Real agent flows touch multiple protocols (KH webhook → Uniswap swap → ENS reputation update → on-chain anchor). The capsule should compose: one capsule, N inner steps, each cryptographically linked.

### Scope

- **Composition shape:** single capsule with `composition.steps[]` array; each step is a full sub-receipt (request_hash + policy_hash + decision + audit_event_id linkage)
- **Per-step gating:** each step runs through SBO3L's policy boundary independently; per-step deny aborts the composition
- **Atomic semantics:** if step N denies, steps 1..N-1 are recorded as `compensated` (executor-defined rollback) OR `accepted` (executor-defined commit-on-success); the capsule records which
- **Step-by-step verification:** `sbo3l passport verify --strict --step N` verifies a single step's claim
- **Whole-capsule verification:** `sbo3l passport verify --strict --composition` verifies the chain of inner audit-event linkages

### Acceptance criteria

- [ ] Composition capsule schema published as `sbo3l.composition_capsule.v1.json`
- [ ] Test: 3-step composition (KH webhook → Uniswap quote → ENS reputation update) all-allow → composition capsule with 3 inner sub-receipts; each verifies independently
- [ ] Test: 3-step composition with step 2 deny → capsule records step 1 `accepted` (or `compensated` if executor opts in) + step 2 `denied` + no step 3
- [ ] Whole-capsule verifier walks the inner-audit-event chain; tampering with one inner audit_event_hash → reject
- [ ] Browser /proof page handles composition capsules — drops in show per-step status

### Exit gate

`bash demo-scripts/cross-protocol-composition.sh` runs the 3-step composition + verification + output to `demo-scripts/artifacts/composition-evidence.json`.

---

## [3.7] Compliance posture

> **Why:** for partner pilots (KeeperHub, Uniswap, ENS-app integrations), SBO3L needs documented SOC 2 + GDPR readiness — not actual SOC 2 cert (out of scope for hackathon timeline) but the technical posture that makes cert-day a documentation exercise rather than a rebuild.

### Scope

- **SOC 2 (security trust principle):** access logging on every API endpoint; key rotation runbook; incident-response runbook; encrypted-at-rest verified for SQLite + KMS-backed signers; audit-chain immutability proofs.
- **SOC 2 (availability):** uptime probe at 99.9% target across the rolling 30-day window; alerting on degradation.
- **SOC 2 (confidentiality):** secrets handling matrix (env vars, GitHub Secrets, KMS); no secrets in logs (grep-asserted).
- **GDPR Article 17 (right to erasure):** `sbo3l audit redact-subject --agent-id <id>` removes all audit events for that agent_id while preserving the hash chain (proof of redaction event itself recorded).
- **GDPR Article 20 (right to portability):** `sbo3l audit export --agent-id <id> --format json` produces a machine-readable bundle + signed manifest.
- **Data residency:** SBO3L deployable region-locked (operator chooses US/EU/etc.); documented per-region operational manual.

### Acceptance criteria

- [ ] `docs/compliance/soc-2-readiness.md` lists every relevant control + evidence pointer (CI workflow run, code reference, etc.)
- [ ] `docs/compliance/gdpr-data-paths.md` documents data-flow per personal-data category
- [ ] `sbo3l audit redact-subject` removes target events + writes a redaction event of its own; full chain still verifies cryptographically
- [ ] `sbo3l audit export --agent-id <id>` produces a signed bundle that the standalone verifier accepts
- [ ] No secret-pattern (e.g. `wfb_[A-Za-z0-9]{20,}`) appears in any production log file (grep-checked in CI)
- [ ] Encrypted-at-rest: SQLite WAL + main file proven via `file <db>` showing encrypted bytes (when SQLCipher driver is enabled — Phase 3.7 adds the SQLCipher feature flag)

### Exit gate

`docs/compliance/{soc-2-readiness,gdpr-data-paths,incident-response,key-rotation}.md` all complete + linked from the SBO3L docs site landing page. `cargo test --test compliance_redaction_paths` exits 0.

---

## [3.8] Self-hosted operator console

> **Why:** the Phase 2 hosted preview at `app.sbo3l.dev` is a single-tenant judges-facing surface. Production needs self-hostable + multi-tenant + RBAC.

### Scope

- **Self-hostable:** `docker compose up sbo3l-console` ships a turnkey deploy; no SBO3L-side hosting required
- **Multi-tenant routing:** subdomain-per-tenant (e.g. `<tenant>.console.sbo3l.dev`) OR header-based routing (`X-Tenant-Slug`)
- **RBAC:** roles `admin`, `auditor`, `operator`, `viewer`; permissions matrix in `docs/console/rbac.md`
- **SSO:** SAML 2.0 + OIDC; tested against Keycloak + Auth0 fixtures
- **Audit-chain UI:** per-tenant audit timeline with filter + search + export
- **Real-time KMS status:** rotation events visible per-tenant; lockout state per-role
- **Flag management UI:** `/admin/flags` (already shipped at #213); RBAC-gated to `admin` role only

### Acceptance criteria

- [ ] `docker compose -f docker-compose.console.yml up` brings up the full console with SQLite DB + 2 tenant fixtures + 4 roles
- [ ] Subdomain routing: `tenant-a.console.sbo3l.dev` shows tenant A's audit chain; tenant B inaccessible
- [ ] RBAC negative test: `viewer` role attempting to access `/admin/flags` → 403
- [ ] SSO: Keycloak fixture login with tenant-A credentials → console shows tenant-A surface
- [ ] Audit timeline UI: filter by `decision=deny` shows only deny events; CSV export round-trips
- [ ] Performance: console page load ≤ 1.5s on cold load (Lighthouse perf ≥ 90)

### Exit gate

`bash demo-scripts/console-self-hosted.sh` brings the stack up + runs the 6-AC negative + positive tests + output to `demo-scripts/artifacts/console-evidence.json`.

---

## Phase 3 done bar

Phase 3 is **declared closed** when:
- 3.1, 3.2, 3.4, 3.6 all ✅ (these are the production-shaped technical claims)
- 3.3, 3.5, 3.7, 3.8 at least 🟡 (in flight with named PR or merged)
- Per-area exit-gate scripts under `demo-scripts/` all green
- `docs/win-backlog/phase-3-readiness.md` shows pass/fail matrix with no 🔴 in the technical-claim half

When the bar flips, Heidi opens v1.3.0 release PR (or v2.0.0 if the surface changes are breaking) per the same shape as v1.2.0.

## Per-area dependency graph

```
3.1 anchoring        ← Phase 2 closed
3.2 multi-tenant     ← #208 audit V010 (LANDED) + F-5 KMS
3.3 marketplace      ← T-4-3 reputation, T-4-1 CCIP, T-3-4 cross-agent
3.4 10K TPS          ← F-3 idempotency, F-2 budget
3.5 token-gated      ← T-4-2 ERC-8004
3.6 composition      ← T-5-3 Smart Wallet, KH adapter, Uniswap adapter
3.7 compliance       ← F-1 auth, F-5 KMS, audit chain (P1 + 3.1)
3.8 console          ← 3.2 multi-tenant, hosted-app slice 3
```

3.1, 3.4, 3.5 can start immediately post-Phase-2-close. 3.2, 3.3, 3.6 wait on each other in places. 3.7, 3.8 are the broadest-base — they ship after the technical claims they depend on are real, not just promised.

## What this document is NOT

- **Not a sprint plan.** Tickets are sized in `docs/win-backlog/0X-phase-3-tickets.md` (created when each sub-area opens).
- **Not a SOC 2 audit.** 3.7 is *readiness posture* — the technical claims that make audit-day a doc exercise. Actual cert is post-Phase-3 commercial work.
- **Not a multi-cloud deploy plan.** 3.8 ships self-hostable Docker compose; multi-cloud (Helm charts for k8s, Terraform modules for AWS/GCP) is post-Phase-3 partner-pilot scope.

## Refresh cadence

This document updates when an AC adds/changes. Heidi maintains; Daniel signs off on any AC removal or new sub-area addition. PRs touching this file follow the same review path as any other backlog doc.
