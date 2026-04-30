# Phase 3 — Top Product + Attack All Tracks (Days 61-100)

> Goal: Win 1st in every targetable track. 0G integration depth, multi-agent swarm, Gensyn AXL, golden vertical demo, master video, EIP draft. Exit: all 8 tracks submitted.

## Phase 3 ticket index

### Track 1 (Phase 3 closure) — KH Best Use 1st

| ID | Title | Owner | Effort | Depends |
|---|---|---|---|---|
| T-1-7 | KH Best Use submission narrative + 100+ live executions logged | Daniel + 📘 Carol/🐍 Dave | 8h | All Phase 2 T-1-* + KH live runs |
| T-1-8 | Public KH protocol PR (IP-1 spec adoption) | Daniel | 6h | T-1-7 |
| T-1-9 | 2+ enterprise pilot signal (companies attesting use) | Daniel | 20h outreach | none |

### Track 6 — 0G Track A (Framework/Tooling)

| ID | Title | Owner | Effort | Depends |
|---|---|---|---|---|
| T-6-1 | 0G Storage backend for Passport capsules | ⛓️ Ivan | 12h | F-6 |
| T-6-2 | 0G DA layer for audit log publishing | ⛓️ Ivan | 14h | F-6 |
| T-6-3 | 0G Compute Network verifier | ⛓️ Ivan | 16h | T-6-1 |
| T-6-4 | 5+ bug reports filed against 0G testnet | Daniel + ⛓️ Ivan | 4h | T-6-1..T-6-3 |
| T-6-5 | `docs/0g-integration.md` full architecture | 📚 Frank | 4h | T-6-1..T-6-4 |

### Track 7 — 0G Track B (Autonomous Agents)

| ID | Title | Owner | Effort | Depends |
|---|---|---|---|---|
| T-7-1 | 5-agent swarm (research/trading/coordinator/audit/execution) | 🦀 Alice + 🛠️ Bob | 24h | T-3-3, T-3-4, T-6-1, T-6-2 |
| T-7-2 | Cross-agent coordination via ENS + SBO3L + 0G + KH | 🦀 Alice | 16h | T-7-1 |
| T-7-3 | Swarm demo video + narrative | Daniel | 8h | T-7-2 |

### Track 8 — Gensyn AXL

| ID | Title | Owner | Effort | Depends |
|---|---|---|---|---|
| T-8-1 | Multi-node SBO3L network (3 nodes) | 🌐 Judy | 16h | F-1..F-6 |
| T-8-2 | AXL-based cross-node agent discovery | 🌐 Judy | 14h | T-8-1 |
| T-8-3 | Cross-node policy enforcement demo | 🌐 Judy | 12h | T-8-2 |
| T-8-4 | Federated audit chain proof | 🌐 Judy | 8h | T-8-3 |

### Cross-Track Infrastructure 4 — Demo + Submission + Launch

| ID | Title | Owner | Effort | Depends |
|---|---|---|---|---|
| CTI-4-1 | Golden vertical demo (`examples/golden-vertical/run.sh`) | 🛠️ Bob | 12h | All track tickets |
| CTI-4-2 | Master demo video (3:30) | Daniel | 24h | CTI-4-1 |
| CTI-4-3 | Public proof site v2 (replaces github.io) | 🎨 Eve | 16h | CTI-3-2 |
| CTI-4-4 | EIP draft: "ERC-XXXX SBO3L Passport Capsule" | Daniel + 📚 Frank | 12h | F-6 |
| CTI-4-5 | Submission packaging per track (8 forms) | Daniel | 16h | All track tickets |
| CTI-4-6 | Sponsor outreach blitz (Luca, Dhaiwat, notMartin, Vitalik) | Daniel | 12h | none (any time) |
| CTI-4-7 | OSS launch (HN + Reddit + Twitter) | Daniel + 📚 Frank | 6h | CTI-4-2 |

**Total Phase 3 effort:** ~280h. With 10 agents in parallel + 100h Daniel = ~40 days.

---

## Track 1 (Phase 3 closure)

### [T-1-7] KH Best Use submission narrative + 100+ live executions logged

**Owner:** Daniel + 📘 Carol/🐍 Dave | **Effort:** 8h | **Depends:** All T-1-* + ongoing KH live runs

**Files:**
- `submissions/keeperhub-best-use/submission.md`
- `demo-scripts/kh-fleet-execution-log.md` (record 100+ live runs)
- `demo-scripts/run-kh-fleet.sh` (script that triggers diverse KH live runs)

**What:**
Quantity + quality submission for KH Best Use 1st. Match Cerberus's 40+ runs by hitting 100+ across 3+ workflows. Six framework integrations (LangChain TS+Py, CrewAI, AutoGen, ElizaOS, LlamaIndex) all hit KH live. Each integration logs ≥ 15 executions to `kh-fleet-execution-log.md` with timestamp + workflow + executionId.

**Acceptance criteria:**
- [ ] 100+ KH live executions logged across 6 integrations
- [ ] 3+ KH workflows used
- [ ] Submission narrative covers framework integrations + execution depth
- [ ] Demo video vignette (60s) shows live KH execution

**QA Test Plan:**
```bash
wc -l demo-scripts/kh-fleet-execution-log.md  # >= 100 lines
grep -c "kh-" demo-scripts/kh-fleet-execution-log.md  # >= 100 executionIds
```

---

### [T-1-8] Public KH protocol PR

**Owner:** Daniel | **Effort:** 6h | **Depends:** T-1-7

**Files:** External (KeeperHub repo PR)

**What:**
Open public PR against KeeperHub's protocol/spec/sdk repo proposing IP-1 envelope adoption. PR includes spec text + reference implementation pointer to `crates/sbo3l-keeperhub-adapter/`. Tag @luca + KH maintainers. PR exists publicly even if not merged.

**Acceptance criteria:**
- [ ] PR opened against keeperhub-protocol repo (public URL captured)
- [ ] PR description references IP-1 spec
- [ ] Reference implementation: `crates/sbo3l-keeperhub-adapter/` linked
- [ ] @luca + maintainers tagged
- [ ] PR URL added to `submissions/keeperhub-best-use/submission.md` as "Mergeable contribution"

---

### [T-1-9] 2+ enterprise pilot signal

**Owner:** Daniel | **Effort:** 20h outreach (across Phase 3)

**Files:** `submissions/keeperhub-best-use/pilot-attestations.md`

**What:**
Get 2+ companies to publicly attest "we're using SBO3L+KH in production" (or "evaluating for production"). Could be small consultancy clients, indie hackers, friendly companies. Goal: real-customer signal beats hypothetical adoption.

**Acceptance criteria:**
- [ ] 2+ public attestations (LinkedIn post, blog post, Twitter, or GitHub README mention)
- [ ] Attestations linked from submission narrative
- [ ] Attestations from real domains (not founder's own accounts)

---

## Track 6 — 0G Track A

### [T-6-1] 0G Storage backend for Passport capsules

**Owner:** ⛓️ Ivan | **Effort:** 12h | **Depends:** F-6

**Files:**
- `crates/sbo3l-storage/src/zerog_backend.rs` (new — storage trait impl)
- `crates/sbo3l-cli/src/passport.rs` (extend `passport run` with `--storage 0g` flag)
- `tests/test_0g_storage.rs`
- `docs/0g-storage.md`

**What:**
Push every emitted capsule to 0G Storage Network, return `rootHash`, embed in capsule's `passport_uri` field (replacing static github.io URL).

**Acceptance criteria:**
- [ ] `passport run --storage 0g` uploads capsule, gets rootHash
- [ ] `passport_uri` updated with `0g://<rootHash>` URI
- [ ] Capsule retrievable by rootHash via 0G Storage SDK
- [ ] Bug reports filed if 0G testnet broken (counts toward T-6-4)

---

### [T-6-2] 0G DA layer for audit log publishing

**Owner:** ⛓️ Ivan | **Effort:** 14h | **Depends:** F-6

**Files:**
- `crates/sbo3l-storage/src/zerog_da.rs` (new)
- `tests/test_0g_da.rs`

**What:**
Audit chain segments published to 0G DA layer for tamper-evidence beyond local SQLite. Every checkpoint creation also publishes to 0G DA.

**Acceptance criteria:**
- [ ] Checkpoint includes 0G DA reference
- [ ] Cross-org auditor can verify chain segment via 0G DA + local capsule

---

### [T-6-3] 0G Compute Network verifier

**Owner:** ⛓️ Ivan | **Effort:** 16h | **Depends:** T-6-1

**Files:**
- `apps/0g-verifier/` (new — runs on 0G Compute)
- `crates/sbo3l-cli/src/passport.rs` (extend with `--verify-on 0g-compute` flag)

**What:**
`passport verify` runs on 0G Compute Network, returns attested verdict. Novel use of 0G's compute layer.

**Acceptance criteria:**
- [ ] Verifier deployed to 0G Compute
- [ ] CLI can request verification via 0G Compute
- [ ] Returned verdict signed/attested by 0G compute node

---

### [T-6-4] 5+ bug reports filed against 0G testnet

**Owner:** Daniel + ⛓️ Ivan | **Effort:** 4h | **Depends:** T-6-1..T-6-3

**Files:** External 0G repo issues; URLs collected in `submissions/0g-track-a/bug-reports.md`

**What:**
Memory says 0G testnet is broken (faucet down, Storage SDK timeouts, KV nodes flaky). File 5+ specific actionable issues with reproductions. Counts toward Builder Feedback narrative AND demonstrates depth.

**Acceptance criteria:**
- [ ] 5+ issues filed against 0G repos
- [ ] Each has reproduction + expected/actual
- [ ] URLs collected

---

### [T-6-5] `docs/0g-integration.md` full architecture

**Owner:** 📚 Frank | **Effort:** 4h | **Depends:** T-6-1..T-6-4

**What:** Architecture doc explaining 3-layer 0G use (Storage + DA + Compute).

---

## Track 7 — 0G Track B (Autonomous Agents)

### [T-7-1] 5-agent swarm

**Owner:** 🦀 Alice + 🛠️ Bob | **Effort:** 24h | **Depends:** T-3-3, T-3-4, T-6-1, T-6-2

**Files:**
- `apps/swarm/` (new)
- `apps/swarm/research-agent/`
- `apps/swarm/trading-agent/`
- `apps/swarm/coordinator-agent/`
- `apps/swarm/audit-agent/`
- `apps/swarm/execution-agent/`

**What:**
5 specialized agents, each running as separate process:
- **research-agent** — fetches market data, builds APRP intent
- **trading-agent** — evaluates intent, queries Uniswap, decides
- **coordinator-agent** — reviews high-value trades, signs cross-agent attestation
- **audit-agent** — monitors audit chain, flags anomalies
- **execution-agent** — final action via KH or Uniswap

Each agent has ENS subname + sbo3l:* records (from T-3-3). Each gates actions through SBO3L. Memory persisted on 0G Storage. Audit on 0G DA.

**Acceptance criteria:**
- [ ] 5 agents running concurrently
- [ ] Coordination flow: research → trading → coordinator → execution → audit
- [ ] Each step gates through SBO3L (full audit chain)
- [ ] Memory + audit on 0G

---

### [T-7-2] Cross-agent coordination

**Owner:** 🦀 Alice | **Effort:** 16h | **Depends:** T-7-1

**What:** Wires the swarm's coordination protocol end-to-end. Demo: research-agent finds opportunity → coordinator approves → execution fires → audit captures.

---

### [T-7-3] Swarm demo video + narrative

**Owner:** Daniel | **Effort:** 8h | **Depends:** T-7-2

**Files:** `submissions/0g-track-b/demo-video.mp4`, `submissions/0g-track-b/submission.md`

**What:** 90s vignette + narrative for 0G Track B submission.

---

## Track 8 — Gensyn AXL

### [T-8-1] Multi-node SBO3L network (3 nodes)

**Owner:** 🌐 Judy | **Effort:** 16h | **Depends:** F-1..F-6

**Files:**
- `apps/multi-node/` (new — orchestration scripts)
- `docker-compose.multi-node.yml`

**What:** 3 SBO3L daemons running on different ports, each with own SQLite, own agent population.

**Acceptance criteria:**
- [ ] `docker compose -f docker-compose.multi-node.yml up` brings 3 nodes online
- [ ] Each node independent SQLite + signer
- [ ] Each node addressable

---

### [T-8-2] AXL-based cross-node agent discovery

**Owner:** 🌐 Judy | **Effort:** 14h | **Depends:** T-8-1

**Files:**
- `crates/sbo3l-axl/` (new crate — Gensyn AXL client)
- Integration with existing identity layer

**What:** Agent on node A queries node B's agent registry via AXL. Cross-node discovery without centralized registry.

---

### [T-8-3] Cross-node policy enforcement demo

**Owner:** 🌐 Judy | **Effort:** 12h | **Depends:** T-8-2

**What:** Agent A's action requires approval from coordinator on node B. Demoes federated policy enforcement.

---

### [T-8-4] Federated audit chain proof

**Owner:** 🌐 Judy | **Effort:** 8h | **Depends:** T-8-3

**What:** Each node has local chain; AXL provides cross-node anchor proof. Cross-org auditor can verify chain segment from any node.

---

## Cross-Track Infrastructure 4 — Demo + Submission

### [CTI-4-1] Golden vertical demo

**Owner:** 🛠️ Bob | **Effort:** 12h | **Depends:** All track tickets

**Files:**
- `examples/golden-vertical/run.sh` (new)
- `examples/golden-vertical/expected-transcript.txt`
- `examples/golden-vertical/README.md`

**What:**
Single bash script that walks the entire stack:
1. ENS resolve `research-agent.sbo3l.eth` → 7 sbo3l:* records
2. Build APRP intent
3. POST to SBO3L → policy decide → signed receipt
4. Cross-agent attestation: research-agent → trading-agent
5. Trading-agent submits swap APRP via SDK
6. SBO3L decide → KH live workflow execution (real executionId)
7. Uniswap real Sepolia swap, tx hash captured
8. Audit checkpoint pushed to 0G DA
9. Capsule v2 emitted (self-contained)
10. Capsule uploaded to 0G Storage, rootHash logged
11. `passport verify --strict` runs WITHOUT aux inputs
12. Capsule cross-verified via 0G Compute Network
13. Final transcript: "All 8 sponsors integrated, all checks green."

**Acceptance criteria:**
- [ ] One command, ~3-5 min runtime
- [ ] Output transcript matches `expected-transcript.txt` (deterministic except hashes/IDs/timestamps)
- [ ] Every sponsor's surface visible in output
- [ ] If ANY step fails, exits non-zero

**QA Test Plan:**
```bash
bash examples/golden-vertical/run.sh > /tmp/gv.txt
diff <(grep -E "^(STEP|✅)" /tmp/gv.txt) <(grep -E "^(STEP|✅)" examples/golden-vertical/expected-transcript.txt)
```

---

### [CTI-4-2] Master demo video (3:30)

**Owner:** Daniel | **Effort:** 24h (record + edit + reshoots)

**What:**
Master video (3:30):
- 0:00-0:15: Tagline + problem statement
- 0:15-1:00: Architecture diagram + boundary explanation
- 1:00-2:30: Golden vertical demo screencast
- 2:30-3:00: Per-sponsor highlights (5s each)
- 3:00-3:30: Closing + CTA

Plus 8 per-track vignettes (30s each).

**Acceptance criteria:**
- [ ] Master video 3:30 ± 0:10
- [ ] All 8 vignettes 30s ± 5s
- [ ] Audio clear, no clipping
- [ ] Captions/subtitles
- [ ] Uploaded to YouTube + IPFS pinned

---

### [CTI-4-3] Public proof site v2

**Owner:** 🎨 Eve | **Effort:** 16h | **Depends:** CTI-3-2

**What:** Replace github.io static site with `sbo3l.dev/proof`. Same artifacts (trust-badge + capsule + verifier docs) but on production domain. Old github.io URL 301 redirects (where possible).

---

### [CTI-4-4] EIP draft: "ERC-XXXX SBO3L Passport Capsule"

**Owner:** Daniel + 📚 Frank | **Effort:** 12h | **Depends:** F-6 (capsule v2 stable)

**Files:**
- `EIP-DRAFT.md` (new) — submitted to ethereum/EIPs repo
- `docs/eip-rationale.md`

**What:**
Draft EIP for Passport capsule schema as a standard for "Agent Action Verification". Specifies:
- Wire format (capsule v2)
- Verifier algorithm
- ENS records integration (`sbo3l:proof_uri`)
- ERC-8004 service reference compatibility

**Acceptance criteria:**
- [ ] EIP draft submitted to ethereum/EIPs repo
- [ ] Draft passes EIP-1 author guidelines
- [ ] Tagged for community review

---

### [CTI-4-5] Submission packaging per track (8 forms)

**Owner:** Daniel | **Effort:** 16h | **Depends:** all track work

**Files:** `submissions/<track>/submission.md` × 8

**What:** Fill 8 ETHGlobal track submission forms. Each:
- Project description
- Demo video URL
- Live demo links
- GitHub repo
- Sponsor-specific narrative

---

### [CTI-4-6] Sponsor outreach blitz

**Owner:** Daniel | **Effort:** 12h | **Depends:** none

**What:** Personal DMs to Luca (KH), Dhaiwat + Simon (ENS), notMartin (Uniswap), 0G team (Vitalik et al.). Pre-judging window: "here's what we shipped".

---

### [CTI-4-7] OSS launch

**Owner:** Daniel + 📚 Frank | **Effort:** 6h | **Depends:** CTI-4-2

**What:** HN + Reddit + Twitter coordinated launch. Post 24-48h before submission deadline (build judging signal).

---

## Phase 3 done condition

All 22 tickets merged + Phase 3 exit gate green (see `08-exit-gates.md`). All 8 tracks submitted with full deliverables. **Going for 1st in every track.**

Submission deadline reached. Pencils down. Wait for results.
