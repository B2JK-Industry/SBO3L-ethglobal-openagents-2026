# Bug bounty platform integration plan (R14 P4)

> **Status:** plan + reachout templates ready. **Account creation is Daniel-side** — both HackerOne and Immunefi require email-verified individual / org accounts that can't be provisioned by an automated agent.

## Why two platforms

SBO3L has two distinct vulnerability classes:

| Class | Primary platform | Why |
|---|---|---|
| Web2 / general security | **HackerOne** | Largest researcher pool; OSS tier waives platform fees |
| Web3 / cryptographic / DeFi | **Immunefi** | Specialist pool; standard for crypto projects; 6–7-figure bounties |

Listing on both maximizes reach without conflict (researchers self-select by class).

## HackerOne — OSS tier

### Eligibility

HackerOne offers free hosting for open-source projects under the **HackerOne Open Source program**. SBO3L qualifies:
- ✅ Apache-2.0 / MIT dual-licensed source
- ✅ Public GitHub repo
- ✅ Active maintenance (cascade visible)
- ✅ Existing `SECURITY.md` with disclosure policy

### Setup steps (Daniel-side)

1. Create individual account at https://hackerone.com/users/sign_up (Daniel's email).
2. Apply for OSS program at https://hackerone.com/opensource (cite repo + Apache-2.0 license).
3. After approval (~3-5 business days), set policy fields:
   - **Scope:** `*.sbo3l.dev` + `crates.io/crates/sbo3l-*` + `pypi.org/project/sbo3l-*` + `npmjs.com/package/@sbo3l/*` + `github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026`
   - **Out-of-scope:** copy from `docs/security/out-of-scope.md`
   - **Severity tiers:** mirror `SECURITY.md` Critical / High / Medium / Low
   - **Payouts:** $1K–5K Critical / $250–1K High / $50–250 Medium / swag Low (matches `SECURITY.md`)
4. Wire the HackerOne URL into `SECURITY.md` (this PR pre-populates with `https://hackerone.com/sbo3l` placeholder; replace once approved).

### Researcher reach: ~250K active

## Immunefi — crypto bug bounty

### Eligibility

Immunefi requires:
- ✅ Smart contracts deployed (we have `AnchorRegistry.sol`, `OffchainResolver.sol`, `SBO3LReputationBond.sol`, `SBO3LSubnameAuction.sol`, etc.)
- ✅ Bounty pool funded ($10K initial pool meets Immunefi's minimum)
- ✅ Public source + audit history (cargo-fuzz + proptest + chaos suite serve as informal pre-audit)
- ✅ Onchain TVL or roadmap showing future TVL

### Setup steps (Daniel-side)

1. Email `bounties@immunefi.com` with reachout template (below).
2. Onboarding call — Immunefi reviews scope + payout tiers.
3. Sign listing agreement (Immunefi takes ~10% on payouts as fee; OSS-friendly terms exist for hackathon-tier projects).
4. Set scope:
   - **In-scope contracts:** AnchorRegistry, OffchainResolver, SBO3LReputationBond, SBO3LSubnameAuction, ReputationRegistry
   - **In-scope off-chain:** capsule schema integrity, ENS resolver path, CCIP-Read gateway signing
   - **Out-of-scope:** Vercel previews, Sepolia testnet
5. Wire the Immunefi URL into `SECURITY.md` (placeholder `https://immunefi.com/bounty/sbo3l` until live).

### Researcher reach: ~30K crypto-specialist

## Reachout templates

### Template A — HackerOne OSS application

```
Subject: SBO3L — OSS bug bounty program application

Hello HackerOne team,

I'm applying for the Open Source program with SBO3L
(https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026).

Project summary: SBO3L is a cryptographically-verifiable trust layer
for autonomous AI agents — every agent action produces a tamper-evident
signed audit row before any side effect. License: Apache-2.0 (with MIT
fallback for some crates).

We have an existing SECURITY.md with a 4-tier severity matrix
(Critical/High/Medium/Low), a $10K initial bounty pool, and a 90-day
coordinated disclosure window. We'd like to use HackerOne as the
primary submission channel.

Repo + license: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026
SECURITY.md: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/SECURITY.md
Maintainer: Daniel B. (babjak_daniel@hotmail.com — primary contact)

Please let me know what additional info you need to process the
application.

Thanks,
Daniel
```

### Template B — Immunefi reachout

```
Subject: SBO3L — bug bounty listing inquiry

Hello Immunefi team,

I'm reaching out about listing SBO3L on Immunefi
(https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026).

Project summary: SBO3L is a cryptographically-verifiable trust layer
for autonomous AI agents. We have several Solidity contracts deployed
on Sepolia testnet (AnchorRegistry, OffchainResolver, ReputationBond,
SubnameAuction, ReputationRegistry) and a capsule format anchored via
ENS text records on mainnet (sbo3lagent.eth, 5 records on chain).

Initial bounty pool: $10K USD funded by the project lead. We're at
hackathon-completion stage (ETHGlobal Open Agents 2026); production
mainnet deployment + TVL is on the post-hackathon roadmap.

Existing security infrastructure:
  - SECURITY.md with 4-tier severity matrix
  - 5 cargo-fuzz harnesses (parsers + verifiers)
  - 4 proptest invariants (APRP / hash / audit chain)
  - 5/5 chaos-engineering scenarios passing
  - SOC 2 / GDPR / HIPAA / PCI-DSS posture documented at docs/compliance/
  - cargo-mutants weekly mutation testing

Please let me know what information you need to scope the listing.
We're interested in OSS-friendly terms for the hackathon-tier launch
and would scale up the pool as mainnet TVL grows.

Thanks,
Daniel
babjak_daniel@hotmail.com
```

## SECURITY.md update plan

Once accounts are live, this PR's follow-up updates `SECURITY.md` to:

```diff
 ## Reporting a vulnerability

-**Preferred:** open a [GitHub Security Advisory](https://github.com/...).
+**Preferred:**
+- Web2 / general: https://hackerone.com/sbo3l (HackerOne OSS program)
+- Web3 / crypto: https://immunefi.com/bounty/sbo3l (Immunefi)
+- Direct: GitHub Security Advisory (encrypted)

-**Alternative:** email `security@sbo3l.dev`...
+**Email:** `security@sbo3l.dev` for anything that doesn't fit the above.
```

## OSS-Fuzz parallel track

In addition to HackerOne + Immunefi, we have OSS-Fuzz scaffolding ready ([`fuzz/oss-fuzz/`](../../fuzz/oss-fuzz/)). OSS-Fuzz finds bugs **without** a researcher in the loop — Google's compute runs our fuzz harnesses 24/7. Submit by PR to `google/oss-fuzz` adding a `projects/sbo3l/` directory with:
- `build.sh` (already at `fuzz/oss-fuzz/build.sh`)
- `Dockerfile` (already at `fuzz/oss-fuzz/Dockerfile`)
- `project.yaml` (already at `fuzz/oss-fuzz/project.yaml`)

OSS-Fuzz approvals usually take 2-3 weeks. Findings flow into the Hall of Fame in `SECURITY.md` like any other report.

## Hall of Fame integration

The Hall of Fame in `SECURITY.md` ships empty today. Once researchers start reporting:

1. Reporter chooses display preference (handle / real name / anonymous).
2. Heidi (or maintainer) appends row to the table at `SECURITY.md:Hall of Fame` AT THE TIME the public advisory ships (post-fix).
3. Cross-link the GitHub Security Advisory + commit hash that fixed it.

## See also

- [`SECURITY.md`](../../SECURITY.md) — top-level security policy.
- [`docs/security/out-of-scope.md`](out-of-scope.md) — what's not eligible.
- [`fuzz/oss-fuzz/`](../../fuzz/oss-fuzz/) — Google OSS-Fuzz scaffolding.
