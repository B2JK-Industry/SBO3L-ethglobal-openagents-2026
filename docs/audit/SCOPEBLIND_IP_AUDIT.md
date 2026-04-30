# ScopeBlind / Veritas Acta IP audit — Verdict: **LOW** risk

**Audit date:** 2026-04-29  
**Auditor:** SBO3L Dev B (research-only; no counsel review)  
**Subject:** Whether ScopeBlind Pty Ltd's published IP (Australian provisional patents, IETF draft, gateway code) creates infringement exposure for SBO3L's hackathon submission.

**Bottom line:** SBO3L can ship its current submission and cite ScopeBlind / `draft-farley-acta-signed-receipts` as related work without further action. There is no granted patent in any jurisdiction; the four Australian provisional applications are early-stage, unpublished, AU-only; three of the four cover technologies SBO3L does not implement; the fourth (Offline Enforcement) has thematic overlap but its specific claims are non-public until grant; the shared primitives (Ed25519, JCS, SHA-256) are royalty-free RFC standards. Re-audit if SBO3L commercialises in Australia.

---

## 1. What ScopeBlind actually claims

### 1.1 The codebase
- `github.com/scopeblind/scopeblind-gateway` — **MIT licensed**. Free to use, modify, distribute. **No patent grant attached to the MIT-licensed gateway code.** The verification tool subset is dual-licensed Apache-2.0, which carries the standard Apache §3 patent grant for that subset only.
- A `PATENTS.md` file in the same repo enumerates the patent posture (see §1.3).

### 1.2 The IETF draft
- **`draft-farley-acta-signed-receipts-01`** — "Signed Decision Receipts for Machine-to-Machine Access Control."
- Author: Tom Farley, ScopeBlind (Veritas Acta). Last revision **25 April 2026** (Expires 27 October 2026).
- **Status: individual submission**, not WG-adopted, "not endorsed by the IETF and has no formal standing in the IETF standards process."
- **No IPR disclosures filed against the draft on IETF datatracker.**
- Normatively defines: signed envelope (`payload` + `signature`), Ed25519 (RFC 8032), JCS (RFC 8785), `previousReceiptHash` chain via SHA-256, six receipt types (Access Decision, Restraint, Arena Battle, Agent Lifecycle, Spending Authority, Formal Debate), JWK Set at `/.well-known/acta-keys.json`, optional Merkle commitment mode (RFC 6962 domain-separated hashing) for selective disclosure, agent manifest with `previous_version` chain, trust tier system (unknown / signed-known / evidenced / privileged).
- A second draft (`draft-farley-acta-knowledge-units-00`, "Knowledge Units for Multi-Model Deliberation") exists but is out of scope for this audit — it covers consensus tracking across multi-model debates, not access control or payment authorisation.

### 1.3 The patent posture (per `PATENTS.md`, authoritative)

| # | Title | Filed | Jurisdiction | Status |
|---|-------|-------|--------------|--------|
| 1 | VOPRF Metering | ~Oct 2025 | AU | Provisional |
| 2 | Verifier Nullifiers | ~Oct 2025 | AU | Provisional |
| 3 | Offline Enforcement | ~Oct 2025 | AU | Provisional |
| 4 | Decision Receipts with Configurable Disclosure | Mar 2026 | AU | Provisional |

- **No specific application numbers** are published.
- **Zero USPTO filings** found by inventor `Farley` + assignee `ScopeBlind` / `Veritas Acta` (Patent Public Search returned no matches as of 2026-04-29).
- The README's "5 Australian provisional patents pending" line is **inconsistent** with `PATENTS.md`'s authoritative count of **4**. Treat `PATENTS.md` as canonical.
- `PATENTS.md` self-narrows the scope: patents cover *"specific server-side issuance methods"* — verification code is explicitly out of scope.

### 1.4 Structural facts about Australian provisional applications
- A provisional confers **no enforceable rights**. Only a granted standard patent does.
- Provisionals **expire 12 months from filing** unless converted to a complete application (PCT or AU standard). The Oct 2025 batch therefore lapses ~Oct 2026 absent conversion.
- Provisionals are **not published** by IP Australia, so the actual claim language is unobservable from outside until/unless the conversion publishes.
- Even after grant, an Australian patent is **only enforceable in Australia**.

---

## 2. Overlap analysis vs SBO3L wire format

### 2.1 Shared primitives (royalty-free, RFC-published)
- Ed25519 signatures — RFC 8032, royalty-free.
- JCS canonicalisation — RFC 8785, royalty-free.
- SHA-256 hash chain over canonicalised prior record — used by Certificate Transparency (RFC 6962), git, hash-chained logs since the 1990s. Prior art is dense and predates ScopeBlind.

These primitives appear in both systems and in dozens of unrelated systems. Co-occurrence does not constitute infringement of anything.

### 2.2 Per-patent overlap matrix

| ScopeBlind provisional | SBO3L implements? | Overlap |
|------------------------|-------------------|---------|
| **VOPRF Metering** | No. SBO3L's budget tracker is a plaintext in-memory `HashMap<BudgetKey, BudgetState>` keyed on `agent_id` (`crates/sbo3l-server/src/lib.rs`). No VOPRF, no oblivious pseudorandom function, no issuer-blind metering. | **None.** |
| **Verifier Nullifiers** | No. SBO3L verification is direct Ed25519 with the disclosed signer pubkey (`crates/sbo3l-core/src/signer.rs::Verifier`). No nullifier scheme, no issuer-blindness — the receipt's `key_id` and signer identity are explicit. | **None.** |
| **Offline Enforcement** | Partial thematic overlap. SBO3L's daemon enforces policy locally and emits a signed receipt that a third party can verify offline (`sbo3l passport verify --path capsule.json`). However, the specific claim language for ScopeBlind's "Offline Enforcement" is non-public (provisional). | **Thematic only — claim-level overlap is unknowable until grant.** Mitigation in §4. |
| **Decision Receipts with Configurable Disclosure** | No. SBO3L's `PolicyReceipt` is fully disclosed — every field is plaintext, signed once, no zero-knowledge proof, no Merkle selective-disclosure tree, no commitment mode. SBO3L's Passport capsule's `verification.offline_verifiable` flag is a structural boolean, not a ZK proof. | **None.** |

### 2.3 Architectural divergence
SBO3L and ScopeBlind solve different problems with overlapping cryptographic primitives:

| Axis | ScopeBlind gateway | SBO3L |
|------|--------------------|-------|
| Trust subject | MCP tool-call decisions (ALLOW/DENY a tool invocation) | Agent payment authorisation (sign or refuse a payment request) |
| Wire format | ACTA receipt envelope (`payload`+`signature`, six receipt types) | APRP (`agent payment request protocol`) + `sbo3l.passport_capsule.v1` capsule |
| Identity model | Agent manifest with `previous_version` chain, trust tiers, JWK Set discovery via `/.well-known/acta-keys.json` | `agent_id` + dev signer (hackathon) → KMS-backed signer (production path); no manifest/JWK Set |
| Selective disclosure | Optional Merkle commitment mode, RFC 6962 hashing | None — full plaintext receipt |
| Audit chain | `previousReceiptHash` per receipt | `prev_event_hash` per audit event in `mandate.audit_bundle.v1` / `sbo3l.audit_bundle.v1` |
| Domain integrations | Generic MCP gateway + Cedar/OPA/JSON policy engines | Sponsor-specific adapters: KeeperHub, ENS, Uniswap (`crates/sbo3l-execution/`), 0G storage upload |
| Visual proof | "Sigil" visual commitments | Static trust-badge HTML viewer (`trust-badge/`) — no commitment, just human-readable receipt panels |

The cryptographic primitives are common; the wire formats, capsule schema, sponsor surface, and trust model are independently designed. SBO3L does not import ScopeBlind code, and the two receipt formats are not byte-compatible nor schema-compatible.

### 2.4 Prior-art context for "Offline Enforcement"
Even narrowing to the single thematically-overlapping provisional, "policy enforcement that emits an offline-verifiable signed receipt" is not novel:
- **OAuth 2.0 RFC 6749 + JWT RFC 7519** — bearer assertions verifiable offline against issuer public key; published 2012/2015.
- **Macaroons** (Birgisson et al., NDSS 2014) — locally-verifiable, offline-attenuable authorisation tokens.
- **Biscuit** (open-source, 2020) — Datalog policy + offline-verifiable token.
- **SPIFFE / SPIRE** (CNCF, 2018+) — workload-identity SVID verifiable against trust bundle.
- **Certificate Transparency RFC 6962** (2013) — hash-chained signed log entries, offline-verifiable.

A specific patent on "offline enforcement" would have to claim something narrower than this prior art to be granted. Until the provisional is converted and published, the SBO3L position is: we use the same RFC primitives the prior art used, in a sponsor-payment context that is its own design.

---

## 3. Risk verdict — **LOW** — with reasoning

| Factor | Direction | Weight |
|--------|-----------|--------|
| Patent maturity | Provisional, no granted patent in any jurisdiction | ↓↓ low risk |
| Jurisdiction | AU only; no USPTO filings found | ↓↓ low risk |
| Claim visibility | Provisionals unpublished — claim language unknowable | ↓ uncertainty (but unenforceable until grant either way) |
| Per-patent overlap | 3 of 4 cover tech SBO3L does not implement; 1 (Offline Enforcement) thematic only | ↓ low risk |
| Shared primitives | Ed25519 / JCS / SHA-256 are royalty-free RFC standards | ↓↓ neutral / low risk |
| Code license overlap | We do not import ScopeBlind's MIT or Apache-2.0 code; no copyleft / no MIT-without-patent-grant exposure | ↓↓ low risk |
| IETF IPR exposure | No IPR disclosures filed against the draft; draft is individual submission, not WG-adopted | ↓↓ low risk |
| Hackathon vs commercial | Submission is non-commercial demo; even if a granted AU patent eventually existed, no commercial-Australia operation | ↓↓ low risk |
| Re-evaluation trigger | Conversion of any of the 4 provisionals to PCT / US filing, or grant of an AU standard patent | ↑ would require re-audit |

**Verdict: LOW.** None of the published artefacts (granted patents: zero; AU provisionals: 4, three out-of-scope, one with thematic-only overlap on a claim language we cannot read; IETF draft: individual submission, no IPR disclosure; gateway code: MIT, not imported by us) creates a present infringement exposure for SBO3L's hackathon submission.

**Honest caveat:** "LOW" reflects what is observable from public sources today. The claim language of all four provisionals is non-public, and the README's "5 patents" / `PATENTS.md`'s "4 patents" inconsistency suggests the public narrative is in flux. A LOW verdict is therefore conditional on (a) the public posture remaining roughly as documented and (b) SBO3L not commercialising in Australia. This audit is research-only and is not a substitute for IP-counsel review before commercial launch.

---

## 4. Recommended action

**Action: cite as related work in `README.md` and `SECURITY_NOTES.md`. No further action at hackathon stage.**

Concretely, add a "Related work" line to `README.md` and to `SECURITY_NOTES.md`'s scope section (Dev A-owned files; B2 PR is docs-only and does **not** modify them — flag for Dev A or a follow-up PR):

> SBO3L's signed-receipt design uses Ed25519 (RFC 8032) and JCS (RFC 8785), the same royalty-free primitives used by `draft-farley-acta-signed-receipts` (T. Farley, ScopeBlind, individual submission). The two receipt formats are not byte-compatible and target different problems: ACTA receipts authorise MCP tool calls; SBO3L authorises agent payment requests against sponsor adapters. Citation acknowledges shared primitives, not derivation.

**Do NOT:**
- Adopt the ACTA receipt envelope wholesale (would couple SBO3L to a non-WG-adopted draft and could draw the gateway's MIT-without-patent-grant posture into our boundary).
- Claim SBO3L is an "implementation of `draft-farley-acta-signed-receipts`" (it is not — different wire format, different scope).
- Import ScopeBlind's MIT-licensed gateway source. The gateway's MIT license carries no patent grant, so any code that incorporated it would inherit that boundary condition.

**Re-audit triggers (open these as new tasks if any fires):**
- ScopeBlind converts any AU provisional to PCT or US standard application (publicly visible on IP Australia / WIPO PATENTSCOPE / USPTO PAIR ~18 months after filing).
- A granted AU standard patent is published with claim language overlapping SBO3L's daemon receipt-issuance path.
- SBO3L moves from hackathon demo to a commercial offering with Australian users or AU-jurisdictional operations.
- IETF working-group adoption of `draft-farley-acta-*` (would change the IPR-disclosure landscape — would-be implementers gain stronger leverage).

---

## Sources consulted (2026-04-29)

- `github.com/scopeblind/scopeblind-gateway` — README, `PATENTS.md`.
- `datatracker.ietf.org/doc/draft-farley-acta-signed-receipts/` — draft -01, last revised 25 April 2026.
- `github.com/VeritasActa/drafts` — draft listing, IETF Trust Legal Provisions.
- `ppubs.uspto.gov` — Patent Public Search by inventor `Farley` + assignee `ScopeBlind` / `Veritas Acta` — zero matches.
- `veritasacta.com` — homepage (HTTP 403 from this audit's fetch; no public-facing IP claims captured).
- General web search for "ScopeBlind / Veritas Acta Australian provisional patent" — surfaced only the GitHub repos and the Veritas Acta homepage; no third-party IP-news coverage.

This audit is read-only research and was conducted without contacting ScopeBlind, Tom Farley, or any IP counsel. Findings reflect public sources only.
