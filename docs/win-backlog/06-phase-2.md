# Phase 2 — Depth + Guaranteed Bounty #2 (Days 31-60)

> Goal: ENS depth (Most Creative + AI Agents track-positioned) + Uniswap depth (Best API track-positioned) + framework integrations (LangChain/CrewAI/AutoGen/ElizaOS/LlamaIndex) + hosted preview at sbo3l.dev. Exit gate locks **ENS Most Creative ($1,250 target) + Uniswap Best API ($2,500 target) submissions packaged**.

## Phase 2 ticket index

### Track 3 — ENS Most Creative

| ID | Title | Owner | Effort | Depends |
|---|---|---|---|---|
| T-3-1 | ENS subname issuance via Durin (`agent register --name foo`) | ⛓️ Ivan | 6h | F-5 (signer) |
| T-3-2 | `sbo3l passport resolve <ens-name>` CLI subcommand | 🛠️ Bob | 4h | none |
| T-3-3 | 5+ named agents on Sepolia with full sbo3l:* records | Daniel + ⛓️ Ivan | 6h | T-3-1 |
| T-3-4 | Cross-agent verification protocol (signed attestation) | 🦀 Alice | 10h | T-3-3 |
| T-3-5 | Real-time agent trust visualization (`app.sbo3l.dev/trust-dns`) | 🎨 Eve | 12h | T-3-3, T-3-4, CTI-3-4 |
| T-3-6 | "Trust DNS" 1500-word essay at `docs.sbo3l.dev/trust-dns` | 📚 Frank | 4h | T-3-4 (so essay reflects shipped protocol) |
| T-3-7 | ENS Most Creative submission narrative + demo video | Daniel | 6h | T-3-1..T-3-6 |

### Track 4 — ENS AI Agents

| ID | Title | Owner | Effort | Depends |
|---|---|---|---|---|
| T-4-1 | ENSIP-25 CCIP-Read for off-chain text records | ⛓️ Ivan | 8h | T-3-1 |
| T-4-2 | ERC-8004 Identity Registry integration | ⛓️ Ivan + 🛠️ Bob | 6h | T-3-3 |
| T-4-3 | Cross-agent reputation via `sbo3l:reputation` records | ⛓️ Ivan + 🦀 Alice | 8h | T-3-3 |

### Track 5 — Uniswap Best API

| ID | Title | Owner | Effort | Depends |
|---|---|---|---|---|
| T-5-1 | Uniswap Trading API integration (full swap construction) | 🛠️ Bob | 10h | F-5 |
| T-5-2 | Universal Router pattern with policy guards per step | 🦀 Alice | 6h | T-5-1 |
| T-5-3 | Smart Wallet integration (agent as Smart Account owner) | ⛓️ Ivan + 🛠️ Bob | 10h | T-5-1 |
| T-5-4 | MEV protection in policy (slippage / priority fee / freshness) | 🦀 Alice | 4h | none |
| T-5-5 | Real Sepolia swap with tx hash captured into capsule | Daniel + 🛠️ Bob | 6h | T-5-1, T-5-3 |
| T-5-6 | `examples/uniswap-agent/` working TS+Py demo | 📘 Carol + 🐍 Dave | 4h | T-5-5 |

### Track 1 — KH Best Use (framework integrations for Phase 3 attack)

| ID | Title | Owner | Effort | Depends |
|---|---|---|---|---|
| T-1-1 | LangChain TypeScript plugin (`@sbo3l/langchain`) | 📘 Carol | 8h | F-9 |
| T-1-2 | LangChain Python plugin (`sbo3l-langchain`) | 🐍 Dave | 8h | F-10 |
| T-1-3 | CrewAI middleware | 🐍 Dave | 6h | F-10 |
| T-1-4 | AutoGen adapter | 📘 Carol | 6h | F-9 |
| T-1-5 | ElizaOS plugin | 📘 Carol | 8h | F-9 |
| T-1-6 | LlamaIndex integration | 🐍 Dave | 6h | F-10 |

### Cross-Track Infrastructure 3 — sbo3l.dev surface

| ID | Title | Owner | Effort | Depends |
|---|---|---|---|---|
| CTI-3-1 | Buy `sbo3l.dev` domain | Daniel | 30 min | none |
| CTI-3-2 | Marketing site at `sbo3l.dev` | 🎨 Eve | 12h | CTI-3-1 |
| CTI-3-3 | Documentation site at `docs.sbo3l.dev` | 📚 Frank | 16h | CTI-3-1 |
| CTI-3-4 | Hosted preview at `app.sbo3l.dev` | 🎨 Eve + 🚢 Grace | 24h | CTI-3-1, F-1, F-7 |

**Total Phase 2 effort:** ~228h. With 8 agents in parallel = ~30 days.

---

## Track 3 — ENS Most Creative

### [T-3-1] ENS subname issuance via Durin

**Owner:** ⛓️ Ivan | **Effort:** 6h | **Depends:** F-5 (signer for tx signing)

**Files:**
- `crates/sbo3l-identity/src/durin.rs` (new — Durin issuance client)
- `crates/sbo3l-cli/src/agent.rs` (new CLI subcommand `sbo3l agent register`)
- `crates/sbo3l-cli/Cargo.toml` (add ethers/viem-rs dep)
- `tests/test_durin_issuance.rs` (new)
- `docs/cli/agent.md` (new doc page)

**What:**
Implement Durin per-agent subname issuance. CLI:
```bash
sbo3l agent register --name research-agent --parent sbo3lagent.eth --records '{"sbo3l:agent_id":"research-agent-01",...}'
```

Issues `research-agent.sbo3lagent.eth` via Durin contract on Sepolia (free testnet) or mainnet. Sets full record set in one tx (multicall).

**Acceptance criteria:**
- [ ] `sbo3l agent register --name foo --parent sbo3lagent.eth --records ...` issues subname on Sepolia
- [ ] Multicall sets all 7 sbo3l:* records in one tx (gas-efficient)
- [ ] Dry-run flag `--dry-run` shows tx data without sending
- [ ] Records readable via `LiveEnsResolver` after issuance
- [ ] Doc page `docs/cli/agent.md` complete

**QA Test Plan:**
```bash
# 1. Dry run
cargo run -p sbo3l-cli -- agent register \
  --name test-agent \
  --parent sbo3lagent.eth \
  --network sepolia \
  --records '{"sbo3l:agent_id":"test-agent-01","sbo3l:endpoint":"http://127.0.0.1:8730/v1"}' \
  --dry-run
# expect: prints tx data, no actual tx

# 2. Real issuance (requires Daniel-funded Sepolia wallet)
SBO3L_SEPOLIA_PRIVATE_KEY=$(cat /tmp/sepolia-key) \
cargo run -p sbo3l-cli -- agent register \
  --name test-agent \
  --parent sbo3lagent.eth \
  --network sepolia \
  --records '{"sbo3l:agent_id":"test-agent-01"}'
# expect: prints tx hash, exit 0

# 3. Verify resolution
SBO3L_ENS_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com \
SBO3L_ENS_NAME=test-agent.sbo3lagent.eth \
cargo run -p sbo3l-identity --example ens_live_smoke
# expect: returns sbo3l:agent_id = "test-agent-01"
```

**[D] Daniel review:**
- [ ] Daniel funds Sepolia wallet (~0.5 ETH = ~$150)
- [ ] Daniel registers `sbo3lagent.eth` apex on Sepolia (parent for subnames)
- [ ] Gas costs documented in ticket

---

### [T-3-2] `sbo3l passport resolve <ens-name>` CLI subcommand

**Owner:** 🛠️ Bob | **Effort:** 4h | **Depends:** none

**Files:**
- `crates/sbo3l-cli/src/passport.rs` (add `resolve` subcommand)
- `crates/sbo3l-cli/src/main.rs` (wire)
- `tests/test_passport_resolve.rs`
- `docs/cli/passport.md` (update)

**What:**
Pure ENS-records-only lookup CLI: `sbo3l passport resolve <ens-name> [--rpc-url <url>] [--check-policy <hash>]`. Output:
```
agent identity:    research-agent-01 (ens: research-agent.sbo3lagent.eth)
policy hash:       e044f13c5acb… ✓ matches expected
endpoint:          http://127.0.0.1:8730/v1
audit root:        0x0000…0000
proof URI:         https://b2jk-industry.github.io/.../capsule.json
capability:        x402-purchase, uniswap-swap (read-side)
reputation:        87/100 (computed from audit chain)
verifier-mode:     ens-records-only
```

**Acceptance criteria:**
- [ ] CLI subcommand exists, help text complete
- [ ] Resolves all 7 sbo3l:* records via LiveEnsResolver
- [ ] Optional `--check-policy <hash>` arg validates policy_hash matches
- [ ] Exit code 0 on success, 1 on resolution failure, 2 on policy mismatch
- [ ] Doc page updated

**QA Test Plan:**
```bash
SBO3L_ENS_RPC_URL=https://ethereum-rpc.publicnode.com \
cargo run -p sbo3l-cli -- passport resolve sbo3lagent.eth | grep -q "policy hash:"
# expect: 0

# Policy mismatch
SBO3L_ENS_RPC_URL=https://ethereum-rpc.publicnode.com \
cargo run -p sbo3l-cli -- passport resolve sbo3lagent.eth --check-policy deadbeef...
# expect: rc=2 + "policy mismatch"

cargo test --test test_passport_resolve
```

---

### [T-3-3] 5+ named agents on Sepolia with full sbo3l:* records

**Owner:** Daniel (wallet) + ⛓️ Ivan (script) | **Effort:** 6h | **Depends:** T-3-1

**Files:**
- `scripts/register-agent-fleet.sh` (new)
- `demo-fixtures/sepolia-agent-fleet.json` (new — record of registrations)

**What:**
Register 5 named agents on Sepolia:
- `research-agent.sbo3lagent.eth`
- `trading-agent.sbo3lagent.eth`
- `swap-agent.sbo3lagent.eth`
- `audit-agent.sbo3lagent.eth`
- `coordinator-agent.sbo3lagent.eth`

Each gets full sbo3l:* record set: `agent_id`, `endpoint`, `policy_hash`, `audit_root`, `proof_uri`, `capability` (NEW), `reputation` (NEW, starts at `100/100`).

**Acceptance criteria:**
- [ ] 5 agents registered + indexed on Sepolia
- [ ] 7 records per agent
- [ ] `cargo run -p sbo3l-cli -- passport resolve <name>.sbo3lagent.eth` returns all 7 records
- [ ] `demo-fixtures/sepolia-agent-fleet.json` lists all 5 + records (for offline reproduction)
- [ ] Total cost < 0.1 ETH on Sepolia (free testnet)

**QA Test Plan:**
```bash
for name in research-agent trading-agent swap-agent audit-agent coordinator-agent; do
  SBO3L_ENS_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com \
  cargo run -p sbo3l-cli -- passport resolve ${name}.sbo3lagent.eth
  echo "expected 7 records for ${name}"
done
```

---

### [T-3-4] Cross-agent verification protocol

**Owner:** 🦀 Alice | **Effort:** 10h | **Depends:** T-3-3

**Files:**
- `crates/sbo3l-identity/src/cross_agent.rs` (new)
- `crates/sbo3l-core/src/cross_agent_attestation.rs` (new — schema)
- `schemas/sbo3l.cross_agent_attestation.v1.json` (new)
- `crates/sbo3l-cli/src/cross_agent.rs` (CLI: `sbo3l cross-agent attest`, `verify`)
- `crates/sbo3l-server/src/lib.rs` (accept `cross_agent_attestation` field on APRP)
- `tests/test_cross_agent_verify.rs`
- `docs/cli/cross-agent.md`

**What:**
Protocol where agent A delegates to agent B with signed cross-agent attestation:
1. Agent A queries ENS for agent B's `policy_hash`
2. Agent A validates B's policy_hash matches expected capability
3. Agent A signs attestation `{from_agent, to_agent, delegation_intent, target_policy_hash, expires_at, signature}`
4. Agent A passes attestation to agent B
5. Agent B includes attestation in their APRP to SBO3L
6. SBO3L verifies attestation chain (A's pubkey → B's policy → B's actual decision)

Schema:
```json
{
  "type": "sbo3l.cross_agent_attestation.v1",
  "from_agent_id": "research-agent-01",
  "from_ens": "research-agent.sbo3lagent.eth",
  "to_agent_id": "trading-agent-01",
  "to_ens": "trading-agent.sbo3lagent.eth",
  "delegation_intent": "delegate_swap",
  "target_policy_hash": "abc123...",
  "expires_at": "2026-05-15T10:00:00Z",
  "signature": {
    "algorithm": "ed25519",
    "key_id": "research-agent-key-v1",
    "signature_hex": "..."
  }
}
```

**Acceptance criteria:**
- [ ] Schema published, validates
- [ ] CLI `sbo3l cross-agent attest --from research-agent.sbo3lagent.eth --to trading-agent.sbo3lagent.eth --intent delegate_swap` works against real Sepolia
- [ ] Attestation flows through APRP → policy receipt → audit chain
- [ ] Tampered attestation rejected with `cross_agent.attestation_invalid`
- [ ] Expired attestation rejected with `cross_agent.attestation_expired`
- [ ] Attestation embedded in capsule's `decision.cross_agent_attestation` field

**QA Test Plan:**
```bash
# Attest
ATT=$(cargo run -p sbo3l-cli -- cross-agent attest \
  --from research-agent.sbo3lagent.eth \
  --to trading-agent.sbo3lagent.eth \
  --intent delegate_swap \
  --signer-key /tmp/research-agent-key.pem \
  --expires-in 1h)

# Submit APRP with attestation
PAYLOAD=$(jq --argjson att "$ATT" '. + {"cross_agent_attestation": $att}' test-corpus/aprp/golden_001_minimal.json)
RC=$(curl -sw "%{http_code}" :8730/v1/payment-requests -X POST \
  -H "Authorization: Bearer test" -H "Content-Type: application/json" -d "$PAYLOAD")

# Tamper + reject
TAMPERED=$(echo $ATT | jq '.delegation_intent = "delegate_anything"')
PAYLOAD=$(jq --argjson att "$TAMPERED" '. + {"cross_agent_attestation": $att}' test-corpus/aprp/golden_001_minimal.json)
curl -s :8730/v1/payment-requests -X POST -d "$PAYLOAD" | jq -r .deny_code
# expect: "cross_agent.attestation_invalid"

cargo test --test test_cross_agent_verify
```

---

### [T-3-5] Real-time agent trust visualization

**Owner:** 🎨 Eve | **Effort:** 12h | **Depends:** T-3-3, T-3-4, CTI-3-4

**Files:**
- `apps/trust-dns-viz/` (new)
- `apps/trust-dns-viz/package.json` (Vite + D3.js)
- `apps/trust-dns-viz/src/main.ts`
- `apps/trust-dns-viz/src/graph.ts` (force-directed graph)
- `apps/trust-dns-viz/src/ws.ts` (WebSocket consumer)
- `crates/sbo3l-server/src/ws_events.rs` (new — SSE/WS endpoint emitting agent events)

**What:**
Force-directed graph (D3.js) showing 5+ agents discovering each other via ENS in real-time. Live updates as:
- New agents resolve each other → edge appears
- Cross-agent attestations sign → edge animates with signed badge
- SBO3L decisions made → node pulses green (allow) or red (deny)
- Audit checkpoints happen → node ring updates

Embedded at `app.sbo3l.dev/trust-dns`.

**Acceptance criteria:**
- [ ] Visualization renders 5 agents with edges
- [ ] WebSocket updates render < 1s latency
- [ ] 60fps with 100 agents stress test
- [ ] Mobile responsive
- [ ] Lighthouse perf > 90
- [ ] Visualization is the demo video centerpiece for ENS Most Creative

**QA Test Plan:**
- Heidi screencaps page at multiple states; Daniel reviews
- Lighthouse run: `npm run lighthouse` on production deploy
- 100-agent stress: `bash demo-scripts/agent-fleet-stress.sh` triggers 100 agent events; viz keeps 60fps

---

### [T-3-6] "Trust DNS" 1500-word essay

**Owner:** 📚 Frank | **Effort:** 4h | **Depends:** T-3-4

**Files:** `docs.sbo3l.dev/content/trust-dns.md` (or appropriate doc site path)

**What:**
1500-word essay: "Why ENS is the natural trust layer for autonomous agent economy". Analogy with TLS/X.509 + DNS for the web. Covers:
- Problem: agents need to discover + verify each other; existing solutions (centralized registries) have trust issues
- Why ENS: immutable, public, cryptographically verifiable, decentralized
- The `sbo3l:*` records as agent passport
- Cross-agent verification protocol (links to T-3-4 docs)
- Future: ENSIP-25 CCIP-Read, ERC-8004 integration

**Acceptance criteria:**
- [ ] 1400-1700 words
- [ ] Audience-stated at top: "AI agent platform engineers, ENS community"
- [ ] All claims have code/test references
- [ ] No jargon without first-use definition
- [ ] Published at `docs.sbo3l.dev/trust-dns`
- [ ] Linked from marketing site

**QA Test Plan:**
- Heidi reads, runs every code block as-shown
- Frank's editorial standards: voice, tone, no breathless claims

---

### [T-3-7] ENS Most Creative submission narrative + demo video

**Owner:** Daniel | **Effort:** 6h | **Depends:** T-3-1..T-3-6

**Files:** `submissions/ens-most-creative/` (new)

**What:**
Package ENS Most Creative submission:
- `submission.md` — narrative (1000 words: problem, solution, why creative, demo evidence, tech depth)
- `demo-video.mp4` (90s vignette focused on Most Creative angle — trust DNS)
- `demo-video.url` — YouTube URL for ETHGlobal form
- Live demo links: https://app.sbo3l.dev/trust-dns + https://docs.sbo3l.dev/trust-dns

**Acceptance criteria:**
- [ ] Submission text 800-1200 words
- [ ] Video 75-105s, audio clear, captions included
- [ ] Demo links live + working
- [ ] Submitted to ETHGlobal Most Creative track form

---

## Track 4 — ENS AI Agents

### [T-4-1] ENSIP-25 CCIP-Read for off-chain text records

**Owner:** ⛓️ Ivan | **Effort:** 8h | **Depends:** T-3-1

**Files:**
- `crates/sbo3l-identity/src/ccip_read.rs` (new)
- `apps/ccip-gateway/` (new — small HTTP gateway returning sbo3l:* records over CCIP-Read protocol)
- `tests/test_ccip_read.rs`
- `docs/ccip-read.md` (new)

**What:**
Implement ENSIP-25 (CCIP-Read) for off-chain sbo3l:* records. Agent has minimal on-chain footprint (just the resolver pointer); record set fetched via CCIP gateway. Reduces gas cost dramatically for high-frequency record updates (e.g. reputation updates per audit checkpoint).

**Acceptance criteria:**
- [ ] CCIP-Read gateway running at `ccip.sbo3l.dev` (or similar)
- [ ] Compatible with `ethers.js` + `viem` resolver flow (judges can resolve via standard tooling)
- [ ] Test: `viem.getEnsText({name: 'research-agent.sbo3lagent.eth', key: 'sbo3l:reputation'})` returns expected
- [ ] Doc explains the resolver setup

**QA Test Plan:**
```bash
cargo test --test test_ccip_read

# E2E via viem
node -e "
import { createPublicClient, http } from 'viem';
import { mainnet } from 'viem/chains';
const c = createPublicClient({ chain: mainnet, transport: http('https://ethereum-rpc.publicnode.com') });
const r = await c.getEnsText({ name: 'research-agent.sbo3lagent.eth', key: 'sbo3l:reputation' });
console.log(r);
"
# expect: numeric reputation
```

---

### [T-4-2] ERC-8004 Identity Registry integration

**Owner:** ⛓️ Ivan + 🛠️ Bob | **Effort:** 6h | **Depends:** T-3-3

**Files:**
- `crates/sbo3l-identity/src/erc8004.rs` (new)
- `crates/sbo3l-cli/src/agent.rs` (extend with `--erc8004-register` flag)
- `tests/test_erc8004.rs`
- `docs/erc8004-integration.md`

**What:**
Register Passport capsule URI in ERC-8004 Identity Registry as a service reference. Agent's on-chain identity now includes pointer to verifiable proof artifact.

**Acceptance criteria:**
- [ ] Agent registration includes ERC-8004 entry
- [ ] Capsule URI registered as service reference
- [ ] Cross-org auditor can pull capsule via ERC-8004 → fetch URI → verify
- [ ] Test on Sepolia first, mainnet after Daniel approves

---

### [T-4-3] Cross-agent reputation via ENS records

**Owner:** ⛓️ Ivan + 🦀 Alice | **Effort:** 8h | **Depends:** T-3-3

**Files:**
- `crates/sbo3l-policy/src/reputation.rs` (new — compute reputation from audit chain)
- `crates/sbo3l-identity/src/reputation_publisher.rs` (publish to ENS on checkpoint)
- `tests/test_reputation.rs`

**What:**
Each agent publishes `sbo3l:reputation` (0-100) computed from audit chain success rate (allowed / total decisions). Updated on each checkpoint creation. Other agents query reputation before delegating via cross-agent attestation.

**Acceptance criteria:**
- [ ] Reputation computed from audit chain
- [ ] Published to ENS on checkpoint
- [ ] Cross-agent attestation refuses if target reputation < threshold
- [ ] Reputation visible in `sbo3l passport resolve` output

---

## Track 5 — Uniswap Best API

### [T-5-1] Uniswap Trading API integration

**Owner:** 🛠️ Bob | **Effort:** 10h | **Depends:** F-5

**Files:**
- `crates/sbo3l-execution/src/uniswap_trading_api.rs` (new)
- `crates/sbo3l-execution/Cargo.toml` (add reqwest with rustls-tls)
- `crates/sbo3l-execution/examples/uniswap_trading_api_smoke.rs` (new)
- `tests/test_uniswap_trading_api.rs`
- `docs/cli/uniswap.md` (new)

**What:**
Integrate Uniswap Developer Platform Trading API (REST) for swap construction. Calls:
- `POST /quote` — get quote with route + gas
- `POST /swap` — get unsigned tx calldata for given quote
- (Phase 2 stops here; Phase 3 sends signed tx via Smart Wallet)

Capture full response (`requestId`, `quote`, `route`, `gas`, `expiry`, `slippage`) into Passport capsule's `executor_evidence`.

**Acceptance criteria:**
- [ ] `UniswapTradingApiExecutor::live_from_env()` ctor (env: `SBO3L_UNISWAP_TRADING_API_KEY`)
- [ ] Quote response captured with all fields
- [ ] Swap calldata captured
- [ ] Capsule `executor_evidence` includes Trading API response (truncated to schema-allowed fields)
- [ ] Backwards-compat: existing QuoterV2 path still works

---

### [T-5-2] Universal Router pattern

**Owner:** 🦀 Alice | **Effort:** 6h | **Depends:** T-5-1

**Files:**
- `crates/sbo3l-policy/src/universal_router.rs` (new)
- `tests/test_universal_router.rs`

**What:**
Multi-step trade construction (e.g. ETH → token-A → token-B) with policy guards at each step. Policy can require:
- Each intermediate token in allowlist
- Total slippage cap across all hops
- Total gas cap

**Acceptance criteria:**
- [ ] Multi-hop policy gate enforced
- [ ] Test: 3-hop trade allowed if all intermediates in allowlist; rejected otherwise

---

### [T-5-3] Smart Wallet integration

**Owner:** ⛓️ Ivan + 🛠️ Bob | **Effort:** 10h | **Depends:** T-5-1

**Files:**
- `crates/sbo3l-execution/src/smart_wallet.rs` (new)
- `tests/test_smart_wallet.rs`

**What:**
Agents act as Smart Account (ERC-4337) owners; SBO3L is the policy guard before signing tx. Smart Wallet UserOperation signed by SBO3L (via `Signer` trait) only after policy decision = allow.

**Acceptance criteria:**
- [ ] Smart Account deployment script (Sepolia)
- [ ] UserOp signed by SBO3L with attached capsule
- [ ] Bundler accepts UserOp
- [ ] On-chain execution success

---

### [T-5-4] MEV protection in policy

**Owner:** 🦀 Alice | **Effort:** 4h | **Depends:** none

**Files:**
- `crates/sbo3l-policy/src/mev.rs` (new)
- `crates/sbo3l-policy/src/lib.rs` (wire into Swap intent)
- `tests/test_mev_protection.rs`

**What:**
Policy-level MEV protection:
- `max_slippage_bps` enforced (default 50)
- `max_priority_fee_gwei` enforced (default 2)
- `quote_freshness_seconds` enforced (default 30)

**Acceptance criteria:**
- [ ] Quote older than freshness window → policy denies with `policy.quote_stale`
- [ ] Slippage > cap → `policy.slippage_too_high`
- [ ] Priority fee > cap → `policy.priority_fee_too_high`

---

### [T-5-5] Real Sepolia swap with tx hash captured

**Owner:** Daniel + 🛠️ Bob | **Effort:** 6h | **Depends:** T-5-1, T-5-3

**Files:**
- `demo-scripts/sponsors/uniswap-real-swap.sh` (new)
- `crates/sbo3l-execution/examples/uniswap_real_swap_smoke.rs`

**What:**
Real Sepolia swap via Trading API + Smart Wallet. Capture real tx hash into capsule.

**Acceptance criteria:**
- [ ] Real Sepolia swap executes (verifiable on Etherscan)
- [ ] `executor_evidence.tx_hash` populated
- [ ] Demo script reproduces end-to-end

**QA Test Plan:**
```bash
SBO3L_UNISWAP_TRADING_API_KEY=$(cat /tmp/uniswap-key) \
SBO3L_UNISWAP_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com \
SBO3L_UNISWAP_PRIVATE_KEY=$(cat /tmp/sepolia-key) \
bash demo-scripts/sponsors/uniswap-real-swap.sh

# Output should include real tx hash
TX_HASH=$(jq -r .execution.executor_evidence.tx_hash demo-scripts/artifacts/uniswap-real-swap-capsule.json)
echo "Verify on Sepolia: https://sepolia.etherscan.io/tx/$TX_HASH"
```

---

### [T-5-6] examples/uniswap-agent/

**Owner:** 📘 Carol + 🐍 Dave | **Effort:** 4h | **Depends:** T-5-5

**Files:**
- `examples/uniswap-agent/typescript/`
- `examples/uniswap-agent/python/`

**What:** 30-line TS + Py examples of agent doing a swap through SBO3L.

---

## Track 1 — Framework Integrations (six)

### [T-1-1] LangChain TypeScript plugin

**Owner:** 📘 Carol | **Effort:** 8h | **Depends:** F-9

**Files:**
- `integrations/langchain-typescript/`
- `package.json` — `@sbo3l/langchain`
- `src/SBO3LTool.ts`
- `examples/langchain-agent-with-sbo3l.ts`

**What:**
LangChain JS Tool that wraps SBO3L: agent's intent → tool call → SBO3L decide → return signed receipt or denial.

**Acceptance criteria:**
- [ ] `@sbo3l/langchain` published
- [ ] Example agent with `OpenAI` + LangChain + SBO3LTool runs end-to-end
- [ ] Tested against running daemon

---

### [T-1-2] LangChain Python plugin

**Owner:** 🐍 Dave | **Effort:** 8h | **Depends:** F-10

**Files:**
- `integrations/langchain-python/`
- `pyproject.toml` — `sbo3l-langchain`
- `sbo3l_langchain/tool.py`

**What:** Mirror of T-1-1 in Python.

---

### [T-1-3] CrewAI middleware

**Owner:** 🐍 Dave | **Effort:** 6h | **Depends:** F-10

**Files:**
- `integrations/crewai/`

**What:** CrewAI tool/middleware that gates Crew actions through SBO3L.

---

### [T-1-4] AutoGen adapter

**Owner:** 📘 Carol | **Effort:** 6h | **Depends:** F-9

**Files:**
- `integrations/autogen/`

**What:** Microsoft AutoGen adapter.

---

### [T-1-5] ElizaOS plugin

**Owner:** 📘 Carol | **Effort:** 8h | **Depends:** F-9

**Files:**
- `integrations/elizaos/`

**What:** ElizaOS plugin (matches Bleyle's competitor track-fit).

---

### [T-1-6] LlamaIndex integration

**Owner:** 🐍 Dave | **Effort:** 6h | **Depends:** F-10

**Files:**
- `integrations/llamaindex/`

**What:** LlamaIndex tool wrapper.

For T-1-1 through T-1-6: each follows same pattern (Tool/middleware wrapper → fetch SBO3L → return decision). Acceptance criteria: published package + working example agent + integration test against running daemon.

---

## Cross-Track Infrastructure 3 — sbo3l.dev surface

### [CTI-3-1] Buy `sbo3l.dev` domain

**Owner:** Daniel | **Effort:** 30 min | **Depends:** none

**Files:** none in repo

**What:** Daniel buys `sbo3l.dev` from Namecheap or Cloudflare Registrar (~$15/yr). Sets nameservers + DNSSEC.

**Acceptance criteria:**
- [ ] Domain owned, DNS configured
- [ ] Cloudflare or similar nameservers active
- [ ] SSL/TLS via Let's Encrypt configured

---

### [CTI-3-2] Marketing site at `sbo3l.dev`

**Owner:** 🎨 Eve | **Effort:** 12h | **Depends:** CTI-3-1

**Files:**
- `apps/marketing/` (Astro / Next.js)
- `apps/marketing/src/pages/index.astro`
- `apps/marketing/src/pages/features.astro`
- `apps/marketing/src/pages/pricing.astro`
- `apps/marketing/src/pages/proof.astro` (replaces github.io capsule download)

**What:**
Marketing site:
- Hero: tagline + 3-card layout + CTA "Start in 5 min"
- Features: architecture diagram + per-pillar feature cards
- Live evidence: ENS / Uniswap / KH evidence (from README)
- Pricing: free / team / enterprise (placeholder pricing)
- Proof: capsule download + verifier instructions

Deployed via Cloudflare Pages or Vercel.

**Acceptance criteria:**
- [ ] sbo3l.dev resolves to marketing site
- [ ] Lighthouse perf > 90
- [ ] WCAG AA compliant
- [ ] Mobile responsive
- [ ] No external CDN deps (privacy)

---

### [CTI-3-3] Documentation site at `docs.sbo3l.dev`

**Owner:** 📚 Frank | **Effort:** 16h | **Depends:** CTI-3-1

**Files:**
- `apps/docs/` (Astro Starlight)
- Migrated content from `docs/` + new tutorial pages

**What:**
Documentation site:
- 5-min tutorial (QUICKSTART.md content)
- API reference (auto-generated from OpenAPI)
- Conceptual guides (APRP, audit log, capsule, cross-agent)
- SDK references (TS + Python)
- CLI reference (per-command pages from `docs/cli/`)

**Acceptance criteria:**
- [ ] docs.sbo3l.dev resolves
- [ ] Search works (built-in Starlight search)
- [ ] Code blocks runnable as-shown
- [ ] Lighthouse perf > 90

---

### [CTI-3-4] Hosted preview at `app.sbo3l.dev`

**Owner:** 🎨 Eve + 🚢 Grace | **Effort:** 24h | **Depends:** CTI-3-1, F-1, F-7

**Files:**
- `apps/hosted-app/` (Next.js or SvelteKit)
- Backend: containerized SBO3L daemon + auth proxy + Postgres for users + sqlite mounted per-tenant
- Deploy: Fly.io / Railway / Render

**What:**
Free tier hosted SBO3L:
- Login (GitHub OAuth)
- Per-user SBO3L instance (SQLite mounted in container)
- Dashboard: recent decisions, audit log, capsule downloads
- Real-time agent feed (consumed by trust-dns viz)

**Acceptance criteria:**
- [ ] app.sbo3l.dev hosts free tier
- [ ] Login works
- [ ] Per-user state isolated
- [ ] Dashboard shows real-time agent activity
- [ ] Production runbook in `docs/ops/runbook.md`
- [ ] OpenTelemetry traces flowing
- [ ] Daily backup + restore-test

---

## Phase 2 done condition

All 26 tickets merged + Phase 2 exit gate green (see `08-exit-gates.md`). Submissions packaged for ENS Most Creative + Uniswap Best API. Floor: $500-1,500 secured. Best case: $3,750.

Move to Phase 3.
