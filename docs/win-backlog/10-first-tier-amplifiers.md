# First-Tier Amplifiers — "Only 1st" Mandate

> Daniel's mandate: **only 1st places**. This file lists the amplifier tickets that push every targeted track from "competitive submission" to "undeniable 1st place".
>
> These tickets are **additive** to Phase 2 + Phase 3 tickets; they are not replacements. Run them in parallel with Phase 2 + Phase 3 work.
>
> Owner: same agent personas as `03-agents.md` apply.

---

## Mandate

| Track | Original 1st prob | After amplifiers | Effort delta | Daniel time delta |
|---|---|---|---|---|
| KH Best Use | 65% | **88%** | +60h dev | +60h |
| KH Builder Feedback | 90% | **96%** | +7h | +5h |
| ENS Most Creative | 35% | **65%** | +60h dev + $300 art | +20h |
| ENS AI Agents | 30% | **80%** | +100h dev + $200 mainnet | +40h |
| Uniswap Best API | 55% | **82%** | +80h dev + $100 mainnet | +30h |

**Total amplifier effort: ~307h dev + $600 mainnet/art + ~155h Daniel.**

Combined with original Phase 2 + Phase 3 backlog: **~580h dev + ~250h Daniel**, fits in 100 days × 4 devs × 6 productive hours = 2400 dev-hours (24% utilization, comfortable buffer).

## Decision gates

### DG-1 — 0G testnet recovery decision (Day 21)

**Gate condition:** by end of Day 21, is 0G testnet operational enough to support 3-layer integration (Storage + DA + Compute)?

**Criteria for "operational":**
- Faucet returns testnet tokens within 5 minutes
- Storage SDK upload succeeds for ≥ 80% of attempts in stress test (50 uploads)
- DA layer accepts at least 100 events/hour
- Compute Network responds to job submissions within 60s

**If GREEN (≥ 3 of 4 criteria pass):** continue T-6-1, T-6-2, T-6-3, T-6-5 (0G Track A) + T-7-1, T-7-2, T-7-3 (0G Track B). Add 0G amplifiers (TBD on this date).

**If RED:** **DROP both 0G tracks**. Reallocate ~80h Phase 3 effort to KH-A2 (more live executions), ENS-AGENT-A1 (more mainnet agents), and UNI-A4 (more production volume). Update Linear board, post in #sbo3l-coordination.

**Owner of decision:** Daniel + Ivan (after Ivan's Day 14-21 testnet probe report).

---

### DG-2 — Gensyn AXL sponsor clarity decision (Day 14)

**Gate condition:** by end of Day 14, do we have direct conversation with Gensyn team about what they value in AXL submissions?

**Criteria for "clarity":**
- Direct DM/call with Gensyn team member (Daniel personal outreach)
- Written guidance on what 1st-place AXL submission looks like
- Confirmation that "SBO3L uses AXL as transport for multi-node policy enforcement" qualifies as a 1st-tier project

**If GREEN (all 3 criteria):** continue T-8-1 through T-8-4 with sponsor-aligned scope. Add Gensyn-specific amplifiers based on guidance.

**If RED:** **DROP Gensyn AXL track**. Reallocate ~50h Phase 3 effort to ENS-MC amplifiers (more polish on Most Creative narrative).

**Owner of decision:** Daniel.

---

## Amplifier tickets

### KH Best Use — push 65% → 88%

#### [KH-A1] Co-author public KH protocol upgrade with Luca's team

**Owner:** Daniel + 🛠️ Bob (Dev1 in 4+1) | **Effort:** 16h Daniel + 8h dev | **Depends:** T-1-7, T-1-8 merged

**Files:**
- External: KeeperHub blog repo PR (joint post)
- External: KeeperHub livestream coordination
- `submissions/keeperhub-best-use/co-marketing.md` (record of joint deliverables)

**What:**
Daniel coordinates with Luca (KH team lead) for a joint public deliverable:
- Joint blog post on KeeperHub blog: "How SBO3L's IP-1 envelope makes agent execution provable"
- Joint livestream/AMA showing IP-1 envelope flowing through KH workflow in real-time
- Co-presented to KH community

**Acceptance criteria:**
- [ ] Joint blog post published on KeeperHub blog (or co-cross-posted)
- [ ] Livestream/AMA recording posted to YouTube + linked from KH community
- [ ] At least 2 KH team members publicly endorse via Twitter/X or LinkedIn
- [ ] Recording referenced in KH Best Use submission

**QA Test Plan:**
- [ ] Heidi verifies blog post URL is on KH-owned domain
- [ ] Heidi verifies livestream is publicly accessible
- [ ] Heidi captures Twitter/X endorsement screenshots into `submissions/keeperhub-best-use/co-marketing.md`

**[D] Daniel review:** Schedule joint deliverables with Luca, ensure messaging aligned.

---

#### [KH-A2] 200+ live KH executions + monitoring dashboard

**Owner:** 📘 Carol (Dev2 in 4+1) + ongoing | **Effort:** 12h dev + ongoing operational | **Depends:** T-1-1..T-1-6 merged (frameworks); F-9, F-10 (SDKs)

**Files:**
- `apps/hosted-app/src/pages/kh-fleet.tsx` (new dashboard page)
- `crates/sbo3l-server/src/metrics.rs` (metrics endpoint emitting kh execution counts)
- `demo-scripts/run-kh-fleet.sh` (script that triggers diverse KH live runs across 6 frameworks, callable from cron)

**What:**
- 200+ KH live executions across 3+ workflows over 100 days
- Public dashboard at `app.sbo3l.dev/kh-fleet` showing live counter (auto-refreshing) of total executions, executions per workflow, executions per framework
- Embed counter widget in submission page

**Acceptance criteria:**
- [ ] `app.sbo3l.dev/kh-fleet` shows real-time counter of executions
- [ ] By Day 100: counter reads ≥ 200
- [ ] Counter is signed (not just text — capsule URI per execution clickable for verify)
- [ ] Cron-driven background runs ensure executions accumulate even without manual trigger
- [ ] Submission page links to counter as live evidence

**QA Test Plan:**
```bash
# Daily check
curl -s https://app.sbo3l.dev/kh-fleet | grep -oE 'data-execution-count="[0-9]+"' | grep -oE '[0-9]+'
# Day 100 expected: >= 200

# Verify capsule per executionId is verifiable
EXEC_ID=$(curl -s https://app.sbo3l.dev/api/kh-fleet/recent | jq -r '.[0].execution_ref')
curl -s https://app.sbo3l.dev/api/kh-fleet/capsule?executionId=$EXEC_ID > /tmp/cap.json
cargo run -p sbo3l-cli -- passport verify --strict --path /tmp/cap.json
```

---

#### [KH-A3] 5+ enterprise/indie pilot attestations

**Owner:** Daniel | **Effort:** 30h outreach across 100 days

**Files:** `submissions/keeperhub-best-use/pilot-attestations.md`

**What:**
Get 5+ companies (real domains, not founder accounts) to publicly attest "we use SBO3L+KH". Attestations can be:
- Public LinkedIn post mentioning SBO3L+KH integration
- README badge on their public repo: "Powered by SBO3L"
- Twitter/X post or thread
- Written quote on their company blog

Targets:
- Daniel's existing professional network (consultancy clients)
- Indie hackers in agent space (offer to set them up free)
- Small teams using KeeperHub already (offer SBO3L integration help)

**Acceptance criteria:**
- [ ] 5+ public attestations from 5+ different organizations
- [ ] Each attestation links live + accessible
- [ ] Each attester's domain confirmed real (whois check, public business)
- [ ] Attestations linked from submission narrative

---

#### [KH-A4] Conference talk co-presented with Luca

**Owner:** Daniel + 🎨 Eve (Dev3 in 4+1) | **Effort:** 8h Daniel + 4h dev | **Depends:** none (start outreach Day 5)

**Files:**
- Conference submission package (varies by conference)
- `submissions/keeperhub-best-use/conference-deck.pdf`

**What:**
Submit talk proposal to one of: DevConnect, EthDenver, ETHGlobal Cannes, Token2049, EthCC. Co-presented with Luca or KH team rep. Topic: "Provable agent execution: how SBO3L + KeeperHub make autonomous payments auditable."

**Acceptance criteria:**
- [ ] Talk submitted to ≥ 1 major conference
- [ ] Talk accepted (or wait-list status visible)
- [ ] Co-speaker confirmed (Luca or KH rep)
- [ ] Slide deck drafted in `submissions/keeperhub-best-use/conference-deck.pdf`

---

#### [KH-A5] KH protocol PR MERGED upstream

**Owner:** Daniel | **Effort:** 12h relationship work

**Files:** External (KeeperHub repo)

**What:**
PR opened in T-1-8 must reach **MERGED** status (not just opened). Daniel works with KH maintainers to address review feedback, iterate, get the IP-1 spec accepted.

**Acceptance criteria:**
- [ ] PR merged into KH protocol repo
- [ ] Merge commit hash captured in submission
- [ ] Public commit author = Daniel + co-author (KH maintainer)

**[D] Daniel review:** This is relationship-driven; iteration may take 2-4 weeks. Start early.

---

### KH Builder Feedback — push 90% → 96%

#### [KH-BF-A1] 10+ issues with reproductions + suggested fixes + reference PRs

**Owner:** Daniel | **Effort:** 4h | **Depends:** T-2-1 (5 issues already filed)

**Files:** `FEEDBACK.md`, external KH issues

**What:**
Extend T-2-1's 5 issues to 10+. Each new issue includes:
- Concrete reproduction (literal commands)
- Expected vs actual
- Suggested fix in 1-2 sentences
- Reference PR draft (even unmerged) showing the suggested fix

**Acceptance criteria:**
- [ ] 10+ public issues filed against KeeperHub repos
- [ ] 5+ have reference PR drafts attached
- [ ] All linked from FEEDBACK.md "Issues filed" section

---

#### [KH-BF-A2] Public blog: "Building on KeeperHub — 10 things I'd improve"

**Owner:** Daniel | **Effort:** 3h

**Files:**
- `apps/marketing/src/blog/2026-05-XX-kh-feedback.md` (or Mirror.xyz / Medium / Hashnode)

**What:**
1500-2000 word public blog post listing 10 specific KH improvements with reasoning. Tagged with respect — this is "love letter from a deep adopter", not complaint piece.

Publish to: own blog (sbo3l.dev/blog) + cross-post to Mirror.xyz + Hashnode.

**Acceptance criteria:**
- [ ] Post published on sbo3l.dev + ≥ 1 third-party platform
- [ ] 1500-2000 words
- [ ] Each of 10 improvements has reproduction + suggested fix
- [ ] Submitted as Builder Feedback bounty narrative reference

---

### ENS Most Creative — push 35% → 65%

> **Cap acknowledgment:** ENS Most Creative is judge-subjective. Even with full amplifier execution, ceiling is ~65-70%. Pantheon mythology / Bottled genie / Crocheth balaclava narrative-only entries can still beat us if a narrative-creative judge is on panel. We accept this variance and ship anyway.

#### [ENS-MC-A1] Original "Trust DNS" visual zine / motion comic

**Owner:** Daniel (artist coordination) + 🎨 Eve (Dev3) | **Effort:** $200-500 art + 10h dev

**Files:**
- `apps/marketing/public/trust-dns-zine/` (artist deliverables)
- `apps/marketing/src/pages/trust-dns-story.astro` (interactive page)

**What:**
Hire artist (Daniel's network or Twitter) to produce:
- 8-12 page visual zine telling the "Trust DNS" story (agents discovering each other via ENS)
- Animated transitions / motion comic frames for marketing site
- Custom illustrations for trust-dns viz nodes

Deliverables in artist's natural style; SBO3L provides the narrative + technical accuracy review.

**Acceptance criteria:**
- [ ] Artist contracted + deliverables received within 14 days
- [ ] Zine published at `sbo3l.dev/trust-dns-story` as interactive Astro page
- [ ] Lighthouse perf > 90 (despite richer media)
- [ ] Zine downloadable as PDF
- [ ] Demo video opening sequence uses zine motion frames

**[D] Daniel review:** Coordinate with artist; review for technical accuracy + brand fit.

---

#### [ENS-MC-A2] "Trust DNS Manifesto" — 5000-word RFC-style piece

**Owner:** 📚 Frank (Dev3 in 4+1) | **Effort:** 16h | **Depends:** T-3-4 (cross-agent verification protocol shipped)

**Files:**
- `apps/marketing/src/blog/trust-dns-manifesto.md`
- Mirror.xyz / Paragraph.xyz cross-post

**What:**
Long-form essay positioning ENS-as-trust-DNS-for-agents as a foundational primitive. RFC-style structure:
- Abstract
- Problem statement (agents need cryptographic identity)
- Existing approaches (centralized registries, JWT-based, on-chain)
- Why ENS is the right answer
- The `sbo3l:*` namespace specification
- Cross-agent verification protocol
- Future work (CCIP-Read, ERC-8004 integration)
- References (cite RFCs, EIPs, papers)

**Acceptance criteria:**
- [ ] 4500-5500 words
- [ ] RFC-style structure with normative language ("MUST", "SHOULD", "MAY")
- [ ] Published on sbo3l.dev/blog + Mirror.xyz
- [ ] Cited references include EIP-137, ENSIP-25, ERC-8004
- [ ] Cross-posted to ENS DAO discussion forum

---

#### [ENS-MC-A3] Live agent fleet mints NFT per decision

**Owner:** ⛓️ Ivan (Dev4 in 4+1) | **Effort:** 24h | **Depends:** T-3-3 (agent fleet on Sepolia, then promote to mainnet via ENS-AGENT-A1)

**Files:**
- `crates/sbo3l-execution/src/nft_attestation.rs` (new)
- `apps/trust-dns-viz/src/nft-feed.ts` (NFT mint feed in viz)
- Mainnet ENS attestation registry (deploy or use existing EAS)

**What:**
Each agent decision (allow/deny) mints an attestation NFT on mainnet ENS attestation registry (or EAS). Trust-DNS viz shows live NFT mints as edges form.

Tangible artistic deliverable: mint feed at `app.sbo3l.dev/trust-dns/nft-feed` shows continuously growing collection.

**Acceptance criteria:**
- [ ] NFT mint per allow/deny decision (mainnet)
- [ ] Mint feed live + visible in viz
- [ ] By Day 100: ≥ 100 minted attestation NFTs
- [ ] OpenSea / similar marketplace shows the collection
- [ ] Gas cost per mint < $0.10 (use cheap chain or batch)

---

#### [ENS-MC-A4] ENSIP submission for `sbo3l:` namespace

**Owner:** Daniel + 📚 Frank | **Effort:** 12h | **Depends:** T-3-4 + ENS-MC-A2

**Files:**
- `ENSIP-DRAFT.md` (submitted to ensdomains/docs)

**What:**
Formal ENSIP submission proposing standardization of `sbo3l:` namespace for agent trust records. Specifies:
- `sbo3l:agent_id` semantics
- `sbo3l:policy_hash` semantics
- `sbo3l:audit_root` semantics
- `sbo3l:proof_uri` semantics
- `sbo3l:capability` semantics
- `sbo3l:reputation` semantics

**Acceptance criteria:**
- [ ] ENSIP draft PR opened in ensdomains/docs
- [ ] Tagged for ENS DAO review
- [ ] Public discussion thread on ENS forum referencing the proposal

---

### ENS AI Agents — push 30% → 80%

#### [ENS-AGENT-A1] Mainnet `sbo3l.eth` apex + 50+ production subname agents

**Owner:** Daniel (wallet) + ⛓️ Ivan | **Effort:** 40h + ~$200 mainnet | **Depends:** T-3-1 (Durin issuance flow)

**Files:**
- `scripts/register-mainnet-fleet.sh`
- `demo-fixtures/mainnet-agent-fleet.json`

**What:**
- Daniel registers `sbo3l.eth` apex on mainnet (~$30-200 depending on rarity)
- Issue 50 named agent subnames via Durin: `agent-001.sbo3l.eth` through `agent-050.sbo3l.eth`
- Each gets full sbo3l:* record set
- 10 named "specialist" agents: `research.sbo3l.eth`, `trading.sbo3l.eth`, ..., (10 categories)

**Acceptance criteria:**
- [ ] `sbo3l.eth` owned on mainnet
- [ ] 50 generic + 10 specialist subnames registered
- [ ] All 60 agents resolve via `LiveEnsResolver` with full record set
- [ ] Total mainnet cost ≤ $200 (using cheap mainnet windows + batched txs)
- [ ] Fleet visible at `app.sbo3l.dev/agents` (mainnet, not Sepolia)

---

#### [ENS-AGENT-A2] ERC-8004 mainnet registration (50+ agents)

**Owner:** ⛓️ Ivan | **Effort:** 16h | **Depends:** ENS-AGENT-A1

**Files:**
- `crates/sbo3l-identity/src/erc8004.rs` (extend for mainnet)

**What:**
Register all 60 agents in ERC-8004 Identity Registry on mainnet. Each entry references their Passport capsule URI.

**Acceptance criteria:**
- [ ] 60 mainnet ERC-8004 Identity Registry entries
- [ ] Each entry has capsule URI as service reference
- [ ] Cross-org auditor can pull capsule via ERC-8004 → URI → verify

---

#### [ENS-AGENT-A3] Production CCIP-Read gateway at ccip.sbo3l.dev

**Owner:** ⛓️ Ivan + 🚢 Grace (Dev4 in 4+1) | **Effort:** 24h | **Depends:** T-4-1 (CCIP-Read impl), CTI-3-4 (hosted infra)

**Files:**
- `apps/ccip-gateway/`
- DNS / TLS for `ccip.sbo3l.dev`

**What:**
Deploy CCIP-Read gateway at `ccip.sbo3l.dev` serving sbo3l:* records for the 60-agent fleet. Production-grade: rate limits, monitoring, fallback to on-chain if gateway down.

**Acceptance criteria:**
- [ ] Gateway live at ccip.sbo3l.dev
- [ ] Resolves records via `viem.getEnsText` for 60 agents
- [ ] Uptime > 99.5% over Day 50-100
- [ ] OpenTelemetry traces flowing
- [ ] Public status page

---

#### [ENS-AGENT-A4] Academic whitepaper "Agent Trust DNS via ENS"

**Owner:** 📚 Frank + Daniel | **Effort:** 24h | **Depends:** ENS-MC-A2 (manifesto provides much of the structure)

**Files:**
- `papers/agent-trust-dns.tex` (LaTeX)
- arXiv submission

**What:**
Academic-shaped whitepaper. 8-12 pages. Submit to arXiv (cs.CR or cs.DC). Optionally submit to IEEE/ACM workshop on AI security.

**Acceptance criteria:**
- [ ] arXiv submission live with DOI
- [ ] Co-authored by Daniel + at least 1 academic collaborator (find via Twitter/email outreach)
- [ ] Cites RFC + EIP + prior art
- [ ] Linked from sbo3l.dev/research

---

### Uniswap Best API — push 55% → 82%

#### [UNI-A1] Real **mainnet** swap broadcast

**Owner:** Daniel + 🛠️ Bob (Dev1) | **Effort:** 16h dev + Daniel wallet + ~$100 gas | **Depends:** T-5-5 (Sepolia swap working)

**Files:**
- `crates/sbo3l-execution/examples/uniswap_mainnet_swap_smoke.rs`
- `demo-scripts/sponsors/uniswap-mainnet-swap.sh`

**What:**
Real mainnet swap. Limited to small safe amount (e.g. 0.01 ETH → USDC). Real tx hash captured into capsule. Verifiable on Etherscan mainnet.

**Acceptance criteria:**
- [ ] Real mainnet swap executed
- [ ] tx hash on Etherscan mainnet
- [ ] Capsule embeds mainnet tx hash + chain_id=1
- [ ] Demo video shows live mainnet swap

**[D] Daniel review:** Daniel reviews swap parameters; confirms small safe amount; signs tx.

---

#### [UNI-A2] Uniswap Foundation grant or formal collaboration

**Owner:** Daniel | **Effort:** 8h outreach

**Files:** `submissions/uniswap-best-api/uniswap-collaboration.md`

**What:**
Submit grant application to Uniswap Foundation OR establish formal collaboration with Uniswap team (notMartin or wider). Goal: public endorsement / collaboration signal.

**Acceptance criteria:**
- [ ] Grant application submitted (publicly trackable) OR
- [ ] Formal collaboration documented (joint project page, Discord pin, etc.)
- [ ] Linked from submission

---

#### [UNI-A3] MEV protection benchmarks

**Owner:** 🦀 Alice (Dev1) + 🎨 Eve (Dev3) | **Effort:** 16h | **Depends:** T-5-4 (MEV protection in policy)

**Files:**
- `apps/marketing/src/uniswap/mev-benchmarks.md`
- `crates/sbo3l-policy/benches/mev_benchmarks.rs`

**What:**
Run SBO3L's MEV-protected swap policy against known MEV bot patterns (frontrun, sandwich, backrun). Publish quantitative results: "SBO3L denied N% of bot-shaped swaps that would have lost user $M."

**Acceptance criteria:**
- [ ] Benchmarks run against ≥ 3 known MEV bot patterns
- [ ] Quantitative results published at `sbo3l.dev/uniswap/mev-benchmarks`
- [ ] Benchmark reproducer in `crates/sbo3l-policy/benches/`

---

#### [UNI-A4] Production Uniswap volume from SBO3L-mediated swaps

**Owner:** Daniel | **Effort:** 30h outreach

**Files:** `submissions/uniswap-best-api/production-volume.md`

**What:**
Onboard 5+ test users (real users, real money) using SBO3L+Uniswap for actual swaps. Track volume on-chain. Display at `app.sbo3l.dev/uniswap/volume`.

**Acceptance criteria:**
- [ ] 5+ real users (different addresses, public attestations)
- [ ] Cumulative volume ≥ $1,000 USD-equivalent over Day 50-100
- [ ] On-chain volume verifiable via Etherscan + Dune dashboard

---

#### [UNI-A5] Uniswap Universal Router PR or example contribution

**Owner:** 🛠️ Bob | **Effort:** 8h | **Depends:** T-5-2 (Universal Router pattern)

**Files:** External Uniswap repo PR

**What:**
Open public PR against Uniswap's `universal-router` or `examples` repo contributing SBO3L policy-guarded swap pattern as a reference example.

**Acceptance criteria:**
- [ ] PR opened with reference implementation
- [ ] Tagged for review by Uniswap maintainers
- [ ] PR URL captured in submission

---

## First-Tier Exit Gate (combined with Phase 3 exit gate)

In addition to Phase 3 exit gate criteria (`08-exit-gates.md`), first-tier mode also requires:

### KH Best Use first-tier validation

```bash
# KH-A1: Co-marketing
[ -f submissions/keeperhub-best-use/co-marketing.md ]
grep -c "https://keeperhub" submissions/keeperhub-best-use/co-marketing.md  # >= 2

# KH-A2: Live counter >= 200
COUNT=$(curl -s https://app.sbo3l.dev/kh-fleet | grep -oE 'data-execution-count="[0-9]+"' | grep -oE '[0-9]+')
[ $COUNT -ge 200 ] || { echo FAIL; exit 1; }

# KH-A3: Pilot attestations >= 5
[ $(grep -c "^- " submissions/keeperhub-best-use/pilot-attestations.md) -ge 5 ]

# KH-A4: Conference deck exists
[ -f submissions/keeperhub-best-use/conference-deck.pdf ]

# KH-A5: KH PR merged
gh pr view <kh-pr-url> --json state | jq -r .state  # "MERGED"
```

### KH Builder Feedback first-tier validation

```bash
# KH-BF-A1: 10+ issues
grep -c "github.com/keeperhub" FEEDBACK.md  # >= 10

# KH-BF-A2: Public blog
curl -sf https://sbo3l.dev/blog/2026-05-XX-kh-feedback | grep -q "Building on KeeperHub"
```

### ENS Most Creative first-tier validation

```bash
# ENS-MC-A1: Zine published
curl -sf https://sbo3l.dev/trust-dns-story | grep -q "trust-dns-story"

# ENS-MC-A2: Manifesto published
curl -sf https://sbo3l.dev/blog/trust-dns-manifesto | grep -q "Trust DNS Manifesto"

# ENS-MC-A3: NFT mints
COUNT=$(curl -s https://app.sbo3l.dev/trust-dns/nft-feed | grep -oE 'data-mint-count="[0-9]+"' | grep -oE '[0-9]+')
[ $COUNT -ge 100 ]

# ENS-MC-A4: ENSIP submitted
gh pr view <ensip-pr-url> --json state | jq -r .state  # "OPEN" or "MERGED"
```

### ENS AI Agents first-tier validation

```bash
# ENS-AGENT-A1: 60 mainnet agents
SBO3L_ENS_RPC_URL=https://ethereum-rpc.publicnode.com \
  cargo run -p sbo3l-cli -- passport resolve agent-001.sbo3l.eth | grep -q "policy hash:"

# Confirm 60 total
COUNT=$(jq '.agents | length' demo-fixtures/mainnet-agent-fleet.json)
[ $COUNT -ge 60 ]

# ENS-AGENT-A2: ERC-8004 entries on mainnet
# (script that queries Identity Registry contract for each agent)

# ENS-AGENT-A3: CCIP gateway live
curl -sf https://ccip.sbo3l.dev/health | grep -q "ok"

# ENS-AGENT-A4: arXiv paper
curl -sf https://arxiv.org/abs/<paper-id> | grep -q "Agent Trust DNS"
```

### Uniswap first-tier validation

```bash
# UNI-A1: Mainnet swap
TX=$(jq -r .execution.executor_evidence.tx_hash demo-scripts/artifacts/uniswap-mainnet-capsule.json)
curl -sf "https://etherscan.io/tx/$TX" | grep -q "Transaction Details"

# UNI-A2: Foundation collaboration
[ -f submissions/uniswap-best-api/uniswap-collaboration.md ]

# UNI-A3: Benchmarks published
curl -sf https://sbo3l.dev/uniswap/mev-benchmarks | grep -q "MEV"

# UNI-A4: Production volume
VOL=$(curl -s https://app.sbo3l.dev/uniswap/volume | grep -oE 'data-total-usd="[0-9.]+"' | grep -oE '[0-9.]+')
[ $(echo "$VOL >= 1000" | bc) -eq 1 ]

# UNI-A5: Universal Router PR
gh pr view <uniswap-pr-url> --json state | jq -r .state  # "OPEN" or "MERGED"
```

---

## Budget summary

| Item | Cost |
|---|---|
| `sbo3l.eth` mainnet apex | ~$30-200 |
| 60 subname mainnet registrations (batched) | ~$50 |
| ERC-8004 mainnet registrations | ~$50 |
| Uniswap mainnet swap (small safe amount + gas) | ~$50-100 |
| ENS NFT attestation gas (100 mints batched) | ~$30 |
| Artist commission (Trust DNS zine) | $200-500 |
| **Total mainnet + art** | **$410-930** |

Plus operational:
- Domain `sbo3l.dev` (CTI-3-1) | ~$15/yr
- Hosted infra (CTI-3-4) | ~$50/month × 4 = $200
- KMS test keys (F-5) | minimal AWS/GCP free tier

**Total budget: ~$650-1,200 across 100 days.** Acceptable for a 1st-tier attack.

---

## When to skip an amplifier

If a track's amplifier is blocked by external factor (sponsor unresponsive, mainnet congestion, gas spike), do NOT force-ship a half-baked version. Better to:
1. Ship 4/5 amplifiers cleanly than 5/5 with 1 broken
2. Update probability assessment honestly
3. Reallocate freed effort to other tracks

The "only 1sts" mandate doesn't mean "ship every amplifier no matter what" — it means "design every track for 1st place; ship the amplifiers that actually amplify".

---

## Non-amplifier-ish things Daniel must do during 100 days

- Sponsor outreach: 60-100h personal DMs, calls, demos
- Wallet ops: ~20h signing mainnet txs at decision moments
- Customer pilot conversations: ~30h
- Conference talk prep + delivery: ~20h
- Blog writing (KH-BF-A2, ENS-MC manifesto): ~10h
- PR reviews: ~30h (every PR before merge)

**Daniel total: ~170-220h** across 100 days = ~17-22h/week. Plus the ~30h/week dev oversight = ~50h/week peak. Burnout is real — schedule weekends off strictly.
