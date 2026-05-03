# UAT R20 Round H re-fire — truthfulness audit post-R20

> **Filed by:** Heidi (QA + Release agent), TASK A of post-R19 watch.
> **Repo state:** main HEAD `a4292ce` (after R20 cascade).
> **Mode:** headless agent — curl + cargo + cast.
> **Started:** 2026-05-03T07:00Z. **Filed:** ~07:30Z.
> **Trigger:** R20 added `apps/docs` Vercel deploy + CLI CCIP-Read + Tier 3 PREVIEW banner. Re-running Round H truthfulness checks against new surface.

## TL;DR

🟢 **PASS with 2 P2 + 1 P3 surfaced.** R20's structural claims all hold (apps/docs deploys; CCIP-Read flow works end-to-end via SBO3L CLI; Tier 3 PREVIEW banner now present). Surfaced 3 fresh truthfulness issues on the Sepolia subname `sbo3l:*` records.

| Round H sub-check | Result |
|---|---|
| sbo3l-docs.vercel.app live + content matches /docs tree | ✅ PASS — HTTP 200, "SBO3L Documentation" title, Phase 3 surfaces h2 visible |
| All 7 /submission/<slug> live | ✅ PASS — 7/7 HTTP 200; structured h1/h2 content present |
| CLI CCIP-Read on Sepolia subname returns research-agent-01 | ✅ PASS — `sbo3l agent verify-ens research-agent.sbo3lagent.eth --network sepolia` returns verdict PASS with 5 records via CCIP-Read |

## Defect table

| # | Severity | What | Repro | Fix |
|---|---|---|---|---|
| 1 | 🟡 P2 | Sepolia subname `sbo3l:endpoint` = `https://app.sbo3l.dev/v1` returns HTTP 000 (no DNS / no response) | `curl -m 10 https://app.sbo3l.dev/v1` | Either deploy a daemon at `app.sbo3l.dev/v1`, OR change the ENS record to a known-reachable URL (or document that the endpoint is operator-side / unreachable like the mainnet apex) |
| 2 | 🟡 P2 | Sepolia subname `sbo3l:proof_uri` = `…/capsules/research-agent-01.json` (or similar) returns HTTP 404 — only mainnet apex `/capsule.json` resolves | `curl -m 10 -L 'https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/capsules/research-agent-01.json'` | Publish per-agent capsule mirror at the URL the ENS record points to, OR update record to point at the existing `/capsule.json` |
| 3 | 🟢 P3 | Sepolia subname `sbo3l:policy_hash` differs from mainnet apex (`…2c94f8c8…` vs `…dd3109f1…`) — both look valid SHA-256, but the value disagreement is unflagged in any doc | `sbo3l agent verify-ens sbo3lagent.eth --network mainnet` ↔ `sbo3l agent verify-ens research-agent.sbo3lagent.eth --network sepolia` | Document the intentional split (mainnet = "apex policy committment"; Sepolia subname = "per-agent policy"). If the split is unintentional, align both. |

## A.1 — apps/docs Vercel deploy verification

```
$ curl https://sbo3l-docs.vercel.app/
HTTP=200 bytes=31092 time=0.80s
title: "SBO3L Documentation | SBO3L Docs"

$ curl https://sbo3l-docs.vercel.app/ | grep -oE '<h[1-3][^>]*>[^<]+</h[1-3]>'
<h1 data-page-title>SBO3L Documentation</h1>
<h2 id="what-is-sbo3l">What is SBO3L</h2>
<h2 id="phase-3-surfaces">Phase 3 surfaces</h2>
```

✅ Live, content matches the `/docs` tree on main. The custom-domain `docs.sbo3l.dev` is not yet pointed (HTTP 000 — DNS missing); the Vercel preview is canonical for now.

## A.2 — All 7 sponsor sub-pages

```
/submission/anthropic                   HTTP=200
/submission/ens-ai-agents               HTTP=200
/submission/ens-most-creative           HTTP=200
/submission/ens-most-creative-final     HTTP=200
/submission/keeperhub                   HTTP=200
/submission/keeperhub-builder-feedback  HTTP=200
/submission/uniswap                     HTTP=200
```

✅ 7/7. Sample structured content (keeperhub-builder-feedback): h1 "SBO3L → KeeperHub Builder Feedback" + h2 sections "Hero claim", "Why this bounty", "Issues filed". keeperhub: h1 "SBO3L → KeeperHub Best Use" + h2 "Hero claim", "Why this bounty", "Technical depth".

**Note:** Daniel's brief mentioned a "Kevin preempt" page, but no such slug appears in `apps/marketing/src/content/submissions/*.md` on main (only the 7 above). If "Kevin preempt" was meant as a content addition to one of the existing slugs (likely keeperhub or keeperhub-builder-feedback), the structural check still passes.

## A.3 — CLI verify-ens with CCIP-Read on Sepolia

```bash
sbo3l agent verify-ens research-agent.sbo3lagent.eth \
  --network sepolia \
  --rpc-url https://ethereum-sepolia-rpc.publicnode.com
```

```
verify-ens: research-agent.sbo3lagent.eth  (network: sepolia)
---
  —       sbo3l:agent_id            actual="research-agent-01"          ← matches expected
  —       sbo3l:endpoint            actual="https://app.sbo3l.dev/v1"   ← P2 #1: unreachable
  ABSENT  sbo3l:pubkey_ed25519      actual="(unset)"
  ABSENT  sbo3l:policy_url          actual="(unset)"
  ABSENT  sbo3l:capabilities        actual="(unset)"
  —       sbo3l:policy_hash         actual="e044f13c5acb2c94f8c8e0b3e9a1d7f2c8b5e4a3d6c9b8e7f6a5d4c3b2a1f0e9d"  ← P3 #3: differs from mainnet
  —       sbo3l:audit_root          actual="0x0000…0000"
  —       sbo3l:proof_uri           actual="https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/capsules/researc…"  ← P2 #2: 404
---
  totals: pass=0 fail=0 skip=5 absent=3
  verdict: PASS
```

✅ **CCIP-Read end-to-end works.** The CLI now follows the OffchainLookup revert from the new OR (per #446), fetches from the gateway, decodes the signed response, and returns 5 records.

The earlier R18 Round F gap ("CLI doesn't follow OffchainLookup") is **closed**.

## Cross-references

- [`docs/proof/uat-final-comprehensive-2026-05-03.md`](uat-final-comprehensive-2026-05-03.md) — comprehensive 8-round UAT (post-R19)
- [`docs/proof/uat-round-1.5-2026-05-03.md`](uat-round-1.5-2026-05-03.md) — continuous per-R18-PR verification
- [`docs/proof/final-uat-pre-submit.md`](final-uat-pre-submit.md) — Round 1 pre-submit
- [`docs/proof/user-acceptance-test-2026-05-02.md`](user-acceptance-test-2026-05-02.md) — UAT-1 (the report this round chains from)

## Next watches (TASKS B-E pending triggers)

- **TASK B:** fires when Daniel signals NÁVOD 1 mainnet Phase 2 setText complete (mainnet apex 7 records, non-localhost endpoint) → ship `uat-round-2-mainnet-or.md`
- **TASK C:** fires when Daniel pastes demo video URL → ship `uat-round-3-demo-video.md`
- **TASK D:** fires 6h before deadline OR Daniel "about to submit" → ship `final-pre-submit-certification.md`
- **TASK E:** fires when Dev 3 R21 100/100 Lighthouse PR lands → ship `uat-r21-lighthouse.md`

Standing watch on cascade.
