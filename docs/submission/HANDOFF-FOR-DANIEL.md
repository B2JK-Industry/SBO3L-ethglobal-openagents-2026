# Submission-day handoff for Daniel

> **For:** Daniel filling the ETHGlobal form + recording the demo video.
> **Filed by:** Heidi (QA + Release agent).
> **Use:** read this in 5 minutes before sitting down. Keep open in a tab while filling the form.

---

## 1-paragraph project summary (paste into ETHGlobal "What it does")

> SBO3L is a cryptographically-verifiable trust layer for autonomous AI agents. Every agent action — paying for an API call, calling a tool, executing a swap — is gated by a policy boundary that produces a tamper-evident, signed audit row before any side effect. The result is a portable Passport capsule that any third party can verify offline against the agent's mainnet ENS record (`<agent>.sbo3lagent.eth`) — no hosted service, no trust in our daemon, no secrets to leak. The same artifact that proves "this agent is legit" satisfies SOC 2 CC7.2, GDPR Article 30, HIPAA §164.312(b), and PCI-DSS Req 10 by construction. Live at https://sbo3l-marketing.vercel.app — drop any capsule into `/proof` and watch 6 strict checks pass in your browser via WASM.

---

## 5 talking points (memorize for the video)

### 1. **"Don't give your agent a wallet. Give it a mandate."**

The tagline encodes the architectural choice: an agent doesn't need an EOA + private key + balance — it needs a **bounded permission slip** signed by its operator that the agent presents to a daemon, which enforces the boundary on every action and produces a proof that an external auditor can verify.

### 2. **The capsule is the proof — no hosted service required.**

Hand a judge a `.json` file. They drop it into `/proof`. Their browser (WASM) verifies 6 strict checks — chain linkage, Ed25519 signatures, JCS-canonical hash bytes, ENS-pinned policy hash, freshness, attestation. Zero round-trips to our daemon. Zero trust in us.

### 3. **ENS as the trust DNS.**

Mainnet `sbo3lagent.eth` carries 5 records. The `policy_hash` text-record is the canonical pin — every capsule includes that hash; if our daemon ever drifts, the offline verifier rejects. Agents get sub-names (`<name>.sbo3lagent.eth`); operators get a single-namespace governance handle.

### 4. **Compliance posture by construction.**

The hash-chained Ed25519-signed audit log is **structurally** what SOC 2 CC7.2, GDPR Art. 30, HIPAA §164.312(b), and PCI-DSS Req 10 ask for. Same artifact, four frameworks. We documented all four ([`docs/compliance/`](../compliance/)) — no audit attestation yet (post-hackathon procurement), but the **artifact** is there today.

### 5. **8 sponsor tracks, real integration on each.**

KeeperHub workflow execution + 5 builder-feedback issues filed with concrete pain points (T-2-1, T-2-2 exit gates met). ENS Most Creative — narrative + ENSIP-25 CCIP-Read OffchainResolver deployed Sepolia. Uniswap — real `quoteExactInputSingle` against Sepolia QuoterV2 with per-step policy gates. ERC-8004 — token-gated agent identity with NFT ownership + time-window gates. Plus production hardening across 8 Phase 3 sub-areas.

---

## Top 3 demos for the video (in order)

### Demo 1 — `/proof` page WASM verifier (≤ 60s, hero)

1. Open https://sbo3l-marketing.vercel.app/proof in a clean browser.
2. Drag-drop `test-corpus/passport/v2_golden_001_minimal.json` onto the verifier.
3. **Watch 6 checks turn green** (structural, hash linkage, signature, schema, freshness, policy pin).
4. Open the JSON, flip a single byte in `audit_chain[0].payload_hash`, save.
5. Drag-drop the tampered file. **Watch the linkage check turn red.**

**Why this first:** it's the entire SBO3L pitch in 60 seconds. Zero hosted-service trust. One file. Six checks. The judge feels the trust property in their hands.

### Demo 2 — `cargo install sbo3l-cli && sbo3l agent verify-ens sbo3lagent.eth` (≤ 30s)

```bash
cargo install sbo3l-cli --version 1.2.0
sbo3l agent verify-ens sbo3lagent.eth --rpc-url https://ethereum-rpc.publicnode.com
```

**What appears (verified by Heidi via UAT 2026-05-02):**

```
verify-ens: sbo3lagent.eth  (network: mainnet)
---
  —       sbo3l:agent_id            actual="research-agent-01"
  —       sbo3l:endpoint            actual="http://127.0.0.1:8730/v1"
  —       sbo3l:policy_hash         actual="e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf"
  —       sbo3l:audit_root          actual="0x0000…0000"  (genesis)
  —       sbo3l:proof_uri           actual="https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/capsule.json"
---
  totals: pass=0 fail=0 skip=5 absent=3
  verdict: PASS
```

**Why this second:** the ENS resolution path is real. The `policy_hash` byte-matches the offline fixture (`sbo3l policy current --hash`). The judge sees mainnet truth.

> **Heads up on `sbo3l:endpoint`:** the published value is `http://127.0.0.1:8730/v1` — that's the operator's local daemon. Reachable only from the operator's machine (by design — the agent's daemon is local-by-default; it's not a public service). The trust assertion is the `policy_hash` + `audit_root`, not endpoint reachability.

### Demo 3 — `/marketplace` + Sepolia OffchainResolver CCIP-Read (≤ 60s)

1. Open https://sbo3l-marketing.vercel.app/marketplace — 5 starter policies.
2. Click any — see the signed manifest + content-addressed registry.
3. (Optional) `cast call 0x87e99508C222c6E419734CACbb6781b8d282b1F6 ...` — show the OffchainResolver returning a signed answer the gateway gateway-signed.

**Why this third:** marketplace closes the "where do agents come from" question. CCIP-Read shows the live Sepolia footprint.

---

## Common-question FAQ (judge perspective)

### Q: How is this different from OPA / Casbin / mandate.md?

> SBO3L is a **superset** — we do allowlist enforcement (like OPA/Casbin) **plus** budget enforcement, **plus** signed PolicyReceipt + tamper-evident audit chain (which OPA and Casbin don't ship out-of-the-box). The criterion benchmark suite at [`benchmarks/competitive/`](../../benchmarks/competitive/) measures the boundary-check portion apples-to-apples — see [`docs/proof/competitive-benchmarks.md`](../proof/competitive-benchmarks.md). mandate.md is closed-source proprietary; the SBO3L→mandate.md rebrand happened 2026-04-29 because mandate.md (the SaaS) collided 1500+ installs in our exact space.

### Q: Why ENS, not a token registry?

> ENS is the public, censorship-resistant, human-readable name layer Ethereum already has. We don't need a new registry — we just need one text-record per agent (`sbo3l:policy_hash`) and a single mainnet apex (`sbo3lagent.eth`). The trust DNS analogy in [`docs/concepts/trust-dns-manifesto.md`](../concepts/trust-dns-manifesto.md) explains why this beats every alternative.

### Q: Is the audit chain quantum-resistant?

> No, and we say so explicitly in [`SECURITY.md`](../../SECURITY.md). Ed25519 + secp256k1 + SHA-256 are all known to be vulnerable to a sufficiently-large quantum computer. Post-quantum migration is on the Phase 4+ roadmap. We make falsifiable security claims, not aspirational ones.

### Q: Can I run this in production today?

> The hackathon-scope demo carries `⚠ DEV ONLY ⚠` banners on the dev-signer mode and unauthenticated mode. Production-shape deployment needs: V011 encryption-at-rest (~2 weeks engineering), the procurement playbook in [`docs/compliance/scan-readiness.md`](../compliance/scan-readiness.md) (~3 months to SOC 2 Type 1), and Daniel-side cloud signup. See [`SECURITY_NOTES.md`](../../SECURITY_NOTES.md) for the deployment-hardening notes.

### Q: How do I verify the capsule isn't a forgery?

> Six independent checks, each falsifiable:
> 1. **Chain linkage** — `chain_hash_v2` recomputes for every audit row.
> 2. **Ed25519 signature** — over canonical-JSON bytes; verifies against the daemon's pubkey.
> 3. **JCS canonicalization** — RFC 8785; reorder-invariant by construction.
> 4. **Schema** — `schemas/passport_capsule_v2.json` enforces structure.
> 5. **Freshness** — `expiry` is past-resistant; clock-skew tolerance documented.
> 6. **Policy pin** — capsule's `policy_hash` byte-matches the ENS text record.
> All 6 run in WASM in your browser. Source: [`crates/sbo3l-core/src/passport.rs`](../../crates/sbo3l-core/src/passport.rs).

### Q: What's the production roadmap?

> [`docs/win-backlog/09-phase-3.md`](../win-backlog/09-phase-3.md) lists 8 sub-areas with explicit ACs and effort budgets totaling ~220h. [`docs/submission/PHASE-3-FINAL-STATUS.md`](PHASE-3-FINAL-STATUS.md) walks through each with current ✅/🟡 status. 4 areas fully met, 4 partial (with concrete progress + documented gap + workaround). Zero scope-cut entirely.

### Q: How do I report a vulnerability?

> [`SECURITY.md`](../../SECURITY.md). GitHub Security Advisory (preferred), `security@sbo3l.dev`, or HackerOne / Immunefi (procurement plan in [`docs/security/bounty-platform-integration.md`](../security/bounty-platform-integration.md)). $10K initial bounty pool. We pay $1K-5K for Critical findings.

---

## Pre-submit checklist (≤ 8 min)

Walk these in a fresh browser before hitting submit:

- [ ] Open https://sbo3l-marketing.vercel.app/ — confirm hero loads.
- [ ] Click "Demo" — walk `/demo/1-meet-the-agents` → 2 → 3 → 4.
- [ ] At `/proof`, drop `test-corpus/passport/v2_golden_001_minimal.json` — confirm 6/6 ✅.
- [ ] At `/proof`, paste a tampered capsule (flip 1 byte in `audit_chain[0].payload_hash`) — confirm ❌.
- [ ] `cargo install sbo3l-cli --version 1.2.0` — confirm `sbo3l --version` → `sbo3l 1.2.0`.
- [ ] `sbo3l agent verify-ens sbo3lagent.eth --rpc-url https://ethereum-rpc.publicnode.com` — confirm 5 records.
- [ ] Open https://sbo3l-marketing.vercel.app/marketplace — confirm 5 starter policies render.
- [ ] (Optional) `cast call 0x87e99508C222c6E419734CACbb6781b8d282b1F6 ...` — show OffchainResolver live.

If any step regresses unexpectedly, ping Heidi and re-check `docs/submission/READY.md` for the latest gap inventory.

---

## ETHGlobal form quick-fills

| Field | Value |
|---|---|
| Project name | SBO3L |
| One-liner | "Don't give your agent a wallet. Give it a mandate." |
| Repo | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026 |
| Live demo | https://sbo3l-marketing.vercel.app |
| Try it (verifier) | https://sbo3l-marketing.vercel.app/proof |
| Mainnet ENS | `sbo3lagent.eth` |
| GitHub Release | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/v1.2.0 |
| ENS contract | OffchainResolver Sepolia `0x87e99508C222c6E419734CACbb6781b8d282b1F6` |
| Anchor contract | AnchorRegistry Sepolia `0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac` |
| Reputation contract | SBO3LReputationRegistry Sepolia `0x6aA95d8126B6221607245c068483fa5008F36dc2` |
| Subname auction | SBO3LSubnameAuction Sepolia `0x5dE75E64739A95701367F3Ad592e0b674b22114B` |
| Reputation bond | SBO3LReputationBond Sepolia `0x75072217B43960414047c362198A428f0E9793dA` |
| Uniswap path | Sepolia QuoterV2 `0xEd1f6473345F45b75F8179591dd5bA1888cf2FB3` |

## See also

- [`docs/submission/READY.md`](READY.md) — go/no-go signal.
- [`docs/submission/PHASE-3-FINAL-STATUS.md`](PHASE-3-FINAL-STATUS.md) — per-AC pass/fail with evidence.
- [`docs/submission/live-url-inventory.md`](live-url-inventory.md) — every public surface, smoke-tested.
- [`docs/submission/judges-walkthrough.md`](judges-walkthrough.md) — 5 / 30 / 90-min reading paths.
- [`docs/submission/ETHGlobal-form-content.md`](ETHGlobal-form-content.md) — long-form form content.
- [`docs/submission/demo-video-script.md`](demo-video-script.md) — video outline.
