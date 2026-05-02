# Security policy

> SBO3L is a cryptographically-verifiable trust layer for autonomous AI agents. Vulnerabilities in our protocol, capsule format, audit chain, or policy boundary directly affect end-user trust. We take security reports seriously and respond fast.

## Reporting a vulnerability

**Preferred:** open a [GitHub Security Advisory](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/security/advisories/new) (private, encrypted).

**Alternative:** email `security@sbo3l.dev` with subject prefix `[SECURITY]`. PGP key: see `docs/security/pgp-key.asc` (fingerprint pinned in this repo).

**Do NOT** open public GitHub issues, post in Discord/Telegram, or tweet about an unpatched vulnerability — coordinated disclosure protects users.

### What to include

| Field | Required | Notes |
|---|---|---|
| Affected component | ✅ | One of: `sbo3l-core`, `sbo3l-server`, `sbo3l-policy`, `sbo3l-identity`, `sbo3l-execution`, `@sbo3l/sdk`, `sbo3l-sdk` (Py), framework integration, hosted-app, marketing, ENS gateway |
| Affected version(s) | ✅ | E.g. `crates ≤ 1.2.0`, `@sbo3l/sdk ≤ 1.0.0`. Check `Cargo.lock` for transitive impact. |
| Reproduction steps | ✅ | Minimal reproducer, ideally a `cargo test` case or `curl` invocation. |
| Severity self-assessment | recommended | CVSS 3.1 vector; we'll re-score. |
| Impact statement | ✅ | What's the worst an attacker can do? Capsule forgery? Audit chain rewrite? Policy bypass? Credential exfil? |
| Suggested fix | optional | If you have one. |

### Response SLA

| Stage | Target |
|---|---|
| Acknowledgement | ≤ 24h |
| Triage + severity assignment | ≤ 72h |
| Fix shipped (Critical) | ≤ 7 days |
| Fix shipped (High) | ≤ 14 days |
| Fix shipped (Medium / Low) | ≤ 30 / 90 days |
| Public advisory | once fix is in latest release on all affected channels (crates.io, npm, PyPI) |

## Severity matrix

We use a hybrid CVSS + impact-class model. Impact class drives the bounty tier; CVSS calibrates within-class.

### Critical 🔴 — capsule trust break

Anything that breaks the falsifiability of a SBO3L capsule. Examples:

- Forged capsule that passes all 6 strict-verifier checks against the canonical mainnet `policy_hash`.
- Audit chain rewrite that preserves linkage byte-for-byte (linkage hash collision in `chain_hash_v2`).
- Policy bypass that produces an `APPROVED` decision while the request actually exceeds a hard cap (`per_tx`, `daily`, `monthly`, `per_provider`, MEV slippage, allowlist).
- Ed25519 / Ethereum signing key disclosure or recovery from public capsule + audit data.
- Replay attack against a finalized capsule that produces a second `APPROVED` decision for the same `nonce`.
- CCIP-Read gateway signature forgery (ENSIP-25 OffchainResolver verification bypass).

### High 🟠 — boundary erosion

Server-side issues that don't directly forge capsules but degrade the boundary. Examples:

- Authentication bypass on `/v1/*` endpoints (skipping the `Authorization: Bearer` gate).
- Idempotency state-machine race that allows two pipeline runs against the same `Idempotency-Key`.
- DB tampering (e.g. SQL injection) that lands a side-effect without an audit row.
- Sponsor-adapter MITM (KeeperHub / ENS / Uniswap) that swaps an upstream response without the audit row recording the divergence.
- Memory-safety issues in `sbo3l-core` (use-after-free, double-free, OOB write) reachable from network input.
- WASM verifier (`/proof` page) crash on adversarial capsule input that allows DoS of the marketing site.

### Medium 🟡 — supply chain / metadata

- Capsule metadata manipulation that doesn't change the verification verdict but mis-attributes (e.g. `agent_id` swap that survives ENS resolution).
- Dependency confusion in `@sbo3l/*` npm scope or `sbo3l-*` PyPI prefix (typosquatting, namespace squat).
- Crates.io / npm / PyPI publish workflow misuse that could land an unsigned package.
- GH Actions secret exfil from public PRs (e.g. `pull_request_target` misuse).
- Audit-DB DoS (storage exhaustion via crafted high-fanout APRPs).

### Low 🟢 — hardening

- Information disclosure that doesn't reveal secrets (e.g. internal error stack traces in HTTP responses).
- Weak defaults that don't violate the boundary but could in misuse.
- Missing security headers on Vercel previews.
- TLS configuration weaknesses on hosted services.

## Bounty program

> **Initial bounty pool:** $10,000 USD, funded by Daniel B. (project lead). Future-state: matched by sponsor partners (KeeperHub, ENS, Uniswap) pending program ramp.

### Payouts (USD)

| Severity | Payout | Hall of Fame |
|---|---|---|
| Critical | $1,000 – $5,000 | ✅ |
| High | $250 – $1,000 | ✅ |
| Medium | $50 – $250 | ✅ |
| Low | swag / Hall of Fame | ✅ |

Payouts are determined by:
1. **Impact** (how many users / funds affected if exploited)
2. **Quality of report** (clear repro, suggested fix, proof of concept)
3. **Coordination** (private disclosure followed; no public exploit before patch)

Payment options: USD wire, USDC on Sepolia/mainnet (after mainnet deploy), or donation to a 501(c)(3) of your choice (we publish proof of donation).

### Eligibility

- ✅ Security researchers acting in good faith.
- ✅ First reporter of a unique vulnerability (duplicates handled per-good-faith — see "Duplicates" below).
- ❌ SBO3L team members and immediate family.
- ❌ Reports requiring physical access to a victim's machine, social engineering of SBO3L staff, or exploits against unsupported software (we'll list these in `docs/security/out-of-scope.md`).

### Duplicates

If two reporters submit the same vulnerability:
- The first valid report (by `triage acknowledgement` timestamp) gets the full bounty.
- A **second reporter who provides materially better repro / impact analysis** can receive up to 25% of the original payout.
- Both are credited in the Hall of Fame.

### Out of scope

| Excluded | Why |
|---|---|
| `SBO3L_ALLOW_UNAUTHENTICATED=1` mode | Documented `⚠ DEV ONLY ⚠` in code; banner printed at startup. |
| `local_mock()` sponsor adapters | Documented test fixtures, never used in production. |
| Vercel preview URLs (`*.vercel.app`) | Marketing/demo only; not the production trust boundary. |
| `*.eth` records on testnet (Sepolia) | Test fixtures, not user-facing. |
| 0-day in transitive dependencies | Report directly to the affected project; we'll issue a SBO3L advisory after the upstream fix lands. |
| Rate-limit complaints | Not a security issue. |

## Hall of Fame

> **Status: program launching 2026-05-02.** No reports yet. Be the first.

| Researcher | Severity | Component | Date | Advisory |
|---|---|---|---|---|
| _(reserved)_ | _(reserved)_ | _(reserved)_ | _(reserved)_ | _(reserved)_ |

Hall of Fame entries are added at the time the public advisory ships (post-fix), with the researcher's preferred handle and (optional) link to their site.

## Test environment for researchers

We provide a dedicated researcher environment:

- **Sepolia testnet daemon** — `https://researcher.sbo3l.dev` (rate-limited; reach out for an API token before scanning).
- **Test corpus** — `test-corpus/` in this repo; contains golden capsules, intentional-failure capsules, and the 5 chaos scenarios.
- **Local-dev Docker image** — `docker pull sbo3l/researcher-dev:1.2.0` spins up a self-contained daemon + ENS resolver + Sepolia mock for offline testing.

Please **do not pentest the production daemon** without prior coordination — DoS attacks against production users are out of scope and may end your participation.

## Cryptographic assertions

If you want to test our cryptographic assertions independently:

| Property | Falsifiable how |
|---|---|
| Capsule chain linkage | Run `sbo3l verify-audit --strict-hash --db <path>` against any DB; tampered byte must reject. |
| Mainnet `policy_hash` matches offline fixture | `sbo3l policy current --hash` ↔ ENS text record `policy_hash` on `sbo3lagent.eth`. |
| CCIP-Read gateway signature | Decode the gateway response; verify Ed25519 signature against the gateway pubkey baked into the OffchainResolver contract. |
| Audit chain Ed25519 over canonical bytes | Re-derive `payload_hash` (JCS-canonical SHA-256), verify `signature` against `chain_hash_v2`. |

## Disclosure policy

We follow **coordinated disclosure**: we work with the reporter privately until a fix is shipped on all affected channels, then publish a GitHub Security Advisory. Default coordination window is **90 days** from acknowledgement, extendable by mutual agreement if the fix is operationally complex.

If a vulnerability is being actively exploited in the wild at the time of report, we'll consider an immediate disclosure with the fix.

## See also

- [`SECURITY_NOTES.md`](SECURITY_NOTES.md) — internal-facing technical security notes (boundary, threats, deployment hardening).
- [`docs/security/`](docs/security/) — extended security documentation (PGP key, out-of-scope details, advisories archive once they ship).
- [`docs/compliance/`](docs/compliance/) — SOC 2 / GDPR / HIPAA / PCI-DSS posture (Phase 3.7).
