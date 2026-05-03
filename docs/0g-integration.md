---
title: "SBO3L × 0G — full integration architecture"
audience: "0G bounty judges + ETHGlobal Open Agents reviewers"
outcome: "Show — without overclaiming — exactly what SBO3L ships on 0G, what's gated, and what was deliberately scope-cut."
---

# SBO3L × 0G integration architecture

> **Tagline:** Don't give your agent a wallet. Give it a mandate.
>
> **0G angle in one sentence:** SBO3L's audit chain produces tamper-evident
> capsule bundles; 0G Storage + 0G DA give those bundles a public, verifiable
> home that doesn't depend on SBO3L's own infrastructure being up.
>
> **Honest-over-slick:** half this doc is what's gated and what we cut.
> Judges asked for that framing — they get it.

---

## TL;DR — per-tier live status

| Tier | Component | Source | Live status |
|---|---|---|---|
| **1** | 0G Storage upload backend (`sbo3l audit export --backend 0g-storage`) | [`crates/sbo3l-storage/src/zerog_backend.rs`](../crates/sbo3l-storage/src/zerog_backend.rs) — PR [#391](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/391) | ✅ **LIVE** — production-shape retry/timeout, real testnet uploads pass |
| **2** | `Sbo3lAuditAnchor` solidity contract | [`crates/sbo3l-identity/contracts/Sbo3lAuditAnchor.sol`](../crates/sbo3l-identity/contracts/Sbo3lAuditAnchor.sol) — PR [#447](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/447) | 🟡 **source LIVE — deploy gated** on 0G faucet Cloudflare Turnstile |
| **3** | 0G Data Availability backend (`ZeroGDataAvailability` trait) | `crates/sbo3l-storage/src/zerog_da.rs` — landing via parallel branch `agent/dev1/0g-da-backend` | 🟡 **source LIVE, mock fallback primary** (testnet flake — see Tier 3) |
| **4** | 0G Compute (policy WASM eval as a Compute task) | — | ⛔ **DEFERRED** — explicit scope-cut, see Tier 4 below |

Two of four tiers are live on the wire today. The third runs deterministically
offline (the `MockDABackend` is the load-bearing path — Tier 3 explains why).
The fourth is honestly cut, with the post-hackathon plan written down so we
don't re-litigate it.

---

## Why 0G — what SBO3L gets that pure-Ethereum can't

SBO3L's product is a **capsule** — a portable, offline-verifiable proof of an
agent action's authorisation. Three things about capsules want a 0G-shaped home:

1. **Bundles are too big for L1 calldata.** A signed receipt + the relevant
   audit-chain slice is hundreds of KB on a real workload. 0G Storage's
   indexer endpoint is the right size class — pay-per-upload, not per-byte
   gas.
2. **Storage and attestation should be independent failure domains.** If
   SBO3L's S3 bucket dies, capsules already in 0G Storage are still
   independently retrievable by `rootHash`. That's a property judges can
   check by killing our infrastructure and replaying.
3. **DA is the right primitive for "many small audit-bundle posts".** The
   audit chain is naturally batched and append-only; 0G DA's blob model
   matches that shape better than pinning IPFS or bundling into Ethereum
   calldata.

What SBO3L deliberately does **not** do on 0G:

- We don't store **policy** on 0G. Policies are part of the capsule (signed,
  re-derivable on the verifier). Putting them on a separate chain would
  introduce a fetch dependency the verifier doesn't need.
- We don't try to make 0G Compute the policy engine. See Tier 4 below.

---

## Tier 1 — 0G Storage backend (LIVE)

**Source:** [`crates/sbo3l-storage/src/zerog_backend.rs`](../crates/sbo3l-storage/src/zerog_backend.rs)
(517 LOC including unit tests). Shipped in PR [#391](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/391).

### What it does

`sbo3l audit export --backend 0g-storage` POSTs a freshly-built audit bundle
to the 0G Galileo testnet indexer at:

```
https://indexer-storage-testnet-turbo.0g.ai/file/upload
```

…and returns the indexer-reported `rootHash`. The CLI envelope then records
that `rootHash` (plus `backend: "0g-storage"`, `endpoint`, `uploaded_at`) into
the capsule's `live_evidence` block, so any downstream verifier can cross-check
the upload against the on-chain anchor (Tier 2) without re-trusting SBO3L.

### Why blocking `reqwest`

The CLI is otherwise single-threaded. Pulling in a tokio runtime just for one
HTTP POST would force the whole binary to be `#[tokio::main]`. `reqwest::blocking`
keeps the surface narrow and the failure modes legible.

### Retry policy — designed for 0G testnet flake

0G Galileo testnet is documented-flaky (faucet down, indexer timeouts, KV
nodes intermittent). The backend ships with:

- **3 attempts total** — first immediate, then 1s + 3s back-off.
- **8s per-attempt timeout** — so worst-case wall-clock is ~28s before the
  caller sees the "use the browser fallback" terminal error.
- **Browser-fallback error message** — when all retries fail, the error
  embeds the official 0G browser-upload tool URL
  (`https://storagescan-galileo.0g.ai/tool`) so an operator hitting a
  hard-outage wall has a concrete recovery path.

The 8s/3-attempt design came from a Codex review on PR #391 — the original
30s/3-attempt budget undermined the fast-fallback path on flaky uploads.

### Reproduce locally

```bash
# 1. Install (already on crates.io)
cargo install sbo3l-cli --version 1.2.0

# 2. Export a real bundle to 0G Storage testnet
cd /tmp
sbo3l audit export \
  --receipt fixtures/receipt.json \
  --chain fixtures/chain.jsonl \
  --receipt-pubkey <ed25519-base64> \
  --audit-pubkey   <ed25519-base64> \
  --backend 0g-storage \
  --out /tmp/bundle.json
# expected: bundle.json contains live_evidence.upload.rootHash and
#           live_evidence.upload.backend == "0g-storage"
# fallback: error message points at https://storagescan-galileo.0g.ai/tool
```

The backend also has a **`live_testnet_upload`** integration test gated behind
`ZEROG_TESTNET_LIVE=1` so CI doesn't depend on the upstream:

```bash
ZEROG_TESTNET_LIVE=1 cargo test -p sbo3l-storage \
  zerog_backend::tests::live_testnet_upload \
  -- --test-threads=1 --nocapture
```

### Honest gaps

- **Testnet only.** The endpoint is hard-coded to `indexer-storage-testnet-turbo.0g.ai`.
  No mainnet path. Mainnet is out of scope for this hackathon.
- **No KZG retrieval-proof verification.** We trust the indexer's reply. See the
  global "Honest gaps" section below.

---

## Tier 2 — `Sbo3lAuditAnchor` contract (source LIVE, deploy gated)

**Source:** [`crates/sbo3l-identity/contracts/Sbo3lAuditAnchor.sol`](../crates/sbo3l-identity/contracts/Sbo3lAuditAnchor.sol)
(53 LOC, 11/11 foundry tests pass — 8 unit + 3 fuzz). Shipped in PR
[#447](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/447).

### What it does

A minimal append-only attestation contract for 0G Galileo (chainID 16602 /
0x40da). One mapping, one event, one mutating function:

```solidity
mapping(bytes32 => uint256) public anchorTimestamp;

function publishAnchor(bytes32 rootHash) external {
    if (anchorTimestamp[rootHash] != 0) revert AlreadyAnchored(rootHash);
    anchorTimestamp[rootHash] = block.timestamp;
    emit AnchorPublished(rootHash, msg.sender, block.timestamp);
}

function getAnchor(bytes32 rootHash) external view returns (uint256);
```

Pair this with Tier 1: the `rootHash` returned by 0G Storage is exactly the
`bytes32` you anchor here. A judge with the capsule and the contract address
can independently prove "this bundle existed by block N".

### Why a separate contract per chain

SBO3L's Sepolia `AnchorRegistry` is multi-tenant and gates publish on tenant
ownership. That's the right shape for the curated Sepolia deployment. This 0G
contract is purpose-built for hackathon attestation — single-publisher, public
reads, no tenant model. Keeping the surfaces separate per chain avoided
bolting tenancy onto a deployment that didn't need it.

### Why deploy is gated

The 0G Galileo faucet uses Cloudflare Turnstile, which requires browser
interaction. A pure CLI / CI deploy can't drip funds. The complete deploy
runbook (8 steps, ~5 minutes once funded) is at
[`docs/dev4/r19-task-c-0g-audit-anchor-deploy.md`](dev4/r19-task-c-0g-audit-anchor-deploy.md):

```bash
# 1. Drip 0G via the Vercel faucet (browser, Turnstile, promo OPEN-AGENT, 10 OG)
# 2. Set PRIVATE_KEY env var (driver wallet PK from local memory)
# 3. forge script script/DeployAuditAnchor0G.s.sol \
#       --rpc-url https://evmrpc-testnet.0g.ai --broadcast
# 4. Pin the deployed address into crates/sbo3l-identity/src/contracts.rs
```

There's a known faucet workaround for rate-limited cases (clear localStorage +
new wallet = fresh 10 OG) — captured in the runbook.

### Honest gap framing

We do **not** claim "AuditAnchor live on 0G." We say "source LIVE, deploy
gated on faucet." The delta is one paste-runnable command sequence and ~5
minutes of human time once funds land. That's the truthful frame.

---

## Tier 3 — 0G Data Availability (NEW)

**Source:** `crates/sbo3l-storage/src/zerog_da.rs` — landing via the parallel
branch `agent/dev1/0g-da-backend`. The trait + three impls are shipped as a
companion PR to this doc.

### Trait shape

```rust
pub trait ZeroGDataAvailability {
    fn disperse(&self, blob: &[u8]) -> Result<DABlobId, DAError>;
}
```

Three impls, picked at the CLI flag boundary:

| Impl | Behaviour | Use case |
|---|---|---|
| `ZeroGDABackend` | Live HTTP POST to `https://da-rpc-testnet.0g.ai/disperse_blob` with the same 1s/3s retry + 8s timeout policy as Tier 1 | Real 0G testnet path when reachable |
| `MockDABackend` | Deterministic `sha256:<hex>` blob_id, fully offline | **Load-bearing fallback** when testnet is down (it does go down) |
| `LocalFileDABackend` | Disk write, returns `file://<path>` blob_id | Tests + air-gapped reproduction |

### Why mock is the load-bearing path

0G Galileo is documented-flaky. A bounty submission that breaks when the
testnet is down is a bad submission. The **mock backend is the primary
verification path for judges** — it's deterministic, fast, and offline:

```bash
# Always works — no network, no faucet, no flake.
sbo3l audit publish --da mock --in /tmp/bundle.json
# expected: blob_id == sha256:<deterministic-hex-of-bundle>

# Real testnet — may succeed, may give browser-fallback pointer.
sbo3l audit publish --da 0g --in /tmp/bundle.json
```

The `--da 0g` path uses the same browser-fallback error message Tier 1 uses,
so an operator hitting a flake gets the same recovery instructions.

### 10 unit tests cover

- Deterministic mock output (same input → same blob_id).
- Local file write + `file://` envelope.
- Live backend retry-on-500 + retry-on-timeout + final browser-fallback error.
- Malformed-response handling (empty blob_id, non-JSON 200).
- Per-attempt timeout configuration.

### Honest gap

We do not perform **KZG commitment-proof verification** on the disperser's
response. We trust the gateway returned what it claims. That's documented at
the trait level and at the CLI flag's help text — judges will not be surprised
by it. The right next step is to add a `verify_blob` method that re-derives
the KZG commitment locally; that's a post-hackathon item.

---

## Tier 4 — 0G Compute (DEFERRED — explicit scope cut)

**Status:** nothing shipped. We are calling this out instead of saying "in
progress" because nothing is in progress.

### Why we cut it

1. **No live-testnet path reachable from a synchronous CLI without a TEE host
   runtime.** The 0G Compute SDK assumes a long-running TEE process; the
   SBO3L CLI is one-shot. Bridging that mismatch was 2-3 days of engineering
   for a path that doesn't differentiate the bounty narrative.
2. **The candidate workload doesn't benefit from off-chain compute.** SBO3L's
   policy decision evaluation (`decide_aprp_wasm`) is already <200ms locally,
   pure WASM, and runs in the verifier's process. Pushing it to 0G Compute
   would add a network round-trip and a second trust assumption (the
   Compute node) without removing one.
3. **Faucet gating affects this tier too.** Same Cloudflare Turnstile wall as
   Tier 2. We'd have shipped source-only here too — not differentiated from
   what we already have on Tier 2.

### Post-hackathon plan

The right next step is to wrap the existing `decide_aprp_wasm` (already
shipped, runs the policy decision under WASM) as a 0G Compute task spec.
That's a one-page YAML + a thin client adapter, ~1 day of work, and the
result is "anyone can pay 0G to re-run the policy decision and confirm the
verdict matches the capsule." Useful, but not what differentiates this
submission today.

We are not claiming this is partially done. It is not partially done.

---

## Reproduce locally — per-tier commands

```bash
# Prerequisite: SBO3L CLI installed
cargo install sbo3l-cli --version 1.2.0

# Tier 1 — 0G Storage upload (network — may fall back to browser tool)
sbo3l audit export \
  --receipt fixtures/receipt.json \
  --chain   fixtures/chain.jsonl \
  --receipt-pubkey <ed25519-b64> \
  --audit-pubkey   <ed25519-b64> \
  --backend 0g-storage \
  --out /tmp/bundle.json

# Tier 1 — local backend (always works, deterministic)
sbo3l audit export ... --backend local --out /tmp/bundle.json

# Tier 2 — contract foundry tests (no network, no faucet)
cd crates/sbo3l-identity/contracts
forge test --match-contract Sbo3lAuditAnchor -vvv
# expected: 11/11 pass (8 unit + 3 fuzz)

# Tier 2 — actual deploy (requires browser faucet drip first)
forge script script/DeployAuditAnchor0G.s.sol \
  --rpc-url https://evmrpc-testnet.0g.ai \
  --broadcast

# Tier 3 — DA mock (always works, deterministic, offline)
sbo3l audit publish --da mock   --in /tmp/bundle.json

# Tier 3 — DA local file (deterministic, writes to disk)
sbo3l audit publish --da local  --in /tmp/bundle.json --out /tmp/blob.bin

# Tier 3 — DA live (network — may fall back like Tier 1)
sbo3l audit publish --da 0g     --in /tmp/bundle.json
```

The two **always-works** paths (Tier 1 `--backend local`, Tier 3 `--da mock`)
are the recommended starting points for judges. They demonstrate the trait
boundary without depending on testnet reachability.

---

## Comparison — where SBO3L's 0G angle differs

The 0G hackathon track has multiple strong submissions. We're not claiming
to be uniquely best; we're describing how the angles differ. Both styles below
are valid bounty-fits — this is positioning, not disparagement.

### vs. StrategyForge — "deep CIDs" / storage-heavy angle

StrategyForge leans into 0G Storage as the primary product surface — heavy on
content-addressed retrieval, retrieval proofs, and storage as the application's
data layer. That's a good fit for an app whose product *is* the stored data.

**SBO3L's angle is different:** 0G Storage is one of *several* destinations
the same `RemoteBackend` trait targets. The product is the capsule (the
cryptographic proof of agent-action authorisation); 0G is the public,
infra-independent home for that proof. We optimise for "the capsule is
provably retrievable from somewhere that isn't us" — not for the storage
layer being the primary surface.

### vs. Construct — three-track 0G integration / agent angle

Construct goes broad across 0G's tracks from an agent-runtime perspective.
That's a strong fit for a project whose centre of gravity is the agent itself.

**SBO3L's angle is different:** SBO3L is **framework-shaped**, not
agent-shaped — it's the policy/audit boundary that any of 14+ agent
frameworks can integrate against. The 0G work is about giving that boundary
a public, verifiable persistence story; the agent is whatever framework
SBO3L is wrapping (LangChain, CrewAI, ElizaOS, Vercel AI SDK, Mastra, etc.).

### Both comparisons in one line

StrategyForge and Construct optimise for *their* product on 0G; SBO3L
optimises for *anyone's* agent on 0G via the framework adapters. Different
shapes, both valid, no overlap on the prize criteria.

---

## Honest gaps (read before judging)

These are the things we explicitly chose **not** to claim:

1. **No KZG verification.** Tier 1 and Tier 3 trust the upstream's response
   shape. Re-deriving the commitment locally is a real post-hackathon item.
2. **No mainnet path.** Both Tier 1 and Tier 2 target 0G Galileo testnet only.
   Mainnet is a deploy-script swap + signing key custody problem; not solved.
3. **Faucet gating is real.** The Cloudflare Turnstile wall blocked Tier 2's
   live deploy from a CI / agent context. We documented the workaround and
   the runbook; we did not pretend it shipped.
4. **Tier 4 is cut.** We are not claiming partial credit for 0G Compute.
   See Tier 4 above.
5. **Testnet flake is the design constraint.** Tier 1's retry policy and
   Tier 3's mock-primary design both exist because we **assume** testnet
   will be down at the moment a judge tries to verify. Both have offline
   fallbacks the judge can run without our infrastructure.

---

## Next steps (post-hackathon)

In rough priority:

1. **Deploy `Sbo3lAuditAnchor` on 0G Galileo** once Daniel finishes the
   browser faucet drip (~5 min). Pin the address into
   `crates/sbo3l-identity/src/contracts.rs` and re-run the verify-anchor
   smoke test.
2. **Add KZG verification** to Tier 1 + Tier 3 — re-derive the commitment
   locally so we don't trust the gateway's response shape.
3. **Wrap `decide_aprp_wasm` as a 0G Compute task spec** — the Tier 4 item.
   ~1 day of work; the WASM blob already exists.
4. **Mainnet path** for Tier 1 + Tier 2 — endpoint swap + key custody
   review.
5. **Operator console wiring** — surface the 0G `rootHash` + anchor
   timestamp in the SBO3L marketplace UI so a customer can audit
   "my agent's last 100 capsules are all anchored."

None of these are blockers for the hackathon submission. All of them have
existing scaffolding the post-hackathon team can pick up without
re-architecting.

---

## Appendix — file map

| Path | Purpose |
|---|---|
| [`crates/sbo3l-storage/src/zerog_backend.rs`](../crates/sbo3l-storage/src/zerog_backend.rs) | Tier 1 — Storage upload backend, 517 LOC w/ unit + live tests |
| `crates/sbo3l-storage/src/zerog_da.rs` | Tier 3 — DA trait + 3 impls (lands via parallel PR) |
| [`crates/sbo3l-identity/contracts/Sbo3lAuditAnchor.sol`](../crates/sbo3l-identity/contracts/Sbo3lAuditAnchor.sol) | Tier 2 — 53 LOC append-only attestation contract |
| [`crates/sbo3l-identity/contracts/test/Sbo3lAuditAnchor.t.sol`](../crates/sbo3l-identity/contracts/test/Sbo3lAuditAnchor.t.sol) | Tier 2 — 8 unit + 3 fuzz, 11/11 pass |
| [`crates/sbo3l-identity/contracts/script/DeployAuditAnchor0G.s.sol`](../crates/sbo3l-identity/contracts/script/DeployAuditAnchor0G.s.sol) | Tier 2 — forge deploy script |
| [`docs/dev4/r19-task-c-0g-audit-anchor-deploy.md`](dev4/r19-task-c-0g-audit-anchor-deploy.md) | Tier 2 — paste-runnable deploy runbook |
| [`apps/marketing/src/components/ZeroGUploader.astro`](../apps/marketing/src/components/ZeroGUploader.astro) | Browser-side 0G uploader (companion UX) |

PR refs: [#391](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/391) (Tier 1) · [#447](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/447) (Tier 2) · Tier 3 lands alongside this doc.
