# Two-Developer Execution Plan

**Datum:** 2026-04-26  
**Ucel:** Ak projekt dostanu dvaja developeri, tento dokument hovori presne co maju robit, v akom poradi, ktore subory vlastnia a kedy je praca hotova.  
**Primary hackathon target:** ETHGlobal Open Agents.  
**Secondary package:** ETHPrague Agent Venture / Network Economy.

Before coding, both developers must read `30_ethglobal_submission_compliance.md`. The hackathon implementation must happen in a fresh public repo with frequent commits. This planning repo can be copied only as clearly labelled planning/spec material.

---

## 1. Product target for this build

Build target is **SBO3L** end to end. Public brand is **SBO3L**; implementation namespace is `mandate` for daemon, CLI, crates, paths and schema IDs.

Build target is:

> **SBO3L** - spending mandates for autonomous agents; a local policy, budget, receipt and audit layer that decides whether an AI agent may execute an onchain/payment action.

Submission package must prove:

1. A real agent can ask for a payment/action.
2. SBO3L can allow a legitimate action.
3. SBO3L can deny a malicious prompt-injection action.
4. A signed policy receipt proves why.
5. ENS can identify the agent and publish policy/audit metadata.
6. At least one sponsor-native adapter runs: KeeperHub first, Uniswap second.

---

## 2. Developer ownership

### Developer A - Core Vault

Owns:

- `/crates/sbo3l-core/`
- `/crates/sbo3l-server/`
- `/crates/sbo3l-cli/`
- `/schemas/`
- `/test-corpus/`
- `/migrations/`
- `docs/api/openapi.json`

Responsibilities:

1. Rust workspace bootstrap.
2. APRP/payment intent parsing and strict validation.
3. Policy engine and budget checks.
4. Decision token and policy receipt format.
5. Audit hash chain.
6. REST API and CLI commands needed by demo.
7. Contract tests against `test-corpus`.

Developer A must not implement sponsor adapters except shared interfaces.

### Developer B - Agent + Sponsor Demo

Owns:

- `/demo-agents/research-agent/`
- `/demo-scripts/`
- `/crates/sbo3l-mcp/`
- `/crates/sbo3l-execution/`
- `/crates/sbo3l-identity/`
- `/crates/sbo3l-receipts/` if split from core later
- `/web-ui/` or static trust badge if implemented

Responsibilities:

1. Real research-agent harness.
2. Prompt-injection and legitimate scenarios.
3. ENS identity proof.
4. KeeperHub guarded execution adapter.
5. Uniswap guarded swap adapter if time permits.
6. Trust badge / demo dashboard.
7. Final recorded demo scripts.

Developer B must not alter core wire formats without updating Developer A and `/schemas`.

---

## 3. Build order

### Milestone 0 - Repo ready

Owner: Developer A  
Support: Developer B only for demo folders

Done when:

- `cargo build` works.
- JSON schemas validate.
- OpenAPI parses.
- `mandate --help` works.
- `demo-scripts/run-single.sh D-P0-01` exists, even if minimal.

### Milestone 1 - Core payment request path

Owner: Developer A

Done when:

- `sbo3l aprp validate test-corpus/aprp/golden_001_minimal.json` passes.
- `sbo3l aprp validate test-corpus/aprp/adversarial_unknown_field.json` fails with `schema.unknown_field`.
- `POST /v1/payment-requests` accepts valid APRP.
- Invalid or unknown agent is rejected.
- Decision token exists but can be dev-signed.

### Milestone 2 - Policy, budget, receipt, audit

Owner: Developer A

Done when:

- `test-corpus/policy/reference_low_risk.json` loads.
- Legit x402 fixture is `auto_approved`.
- Prompt-injection fixture is `rejected` with `policy.deny_recipient_not_allowlisted`.
- Signed policy receipt is returned for allow and deny.
- Audit chain verifier detects tamper.

### Milestone 3 - Real agent harness

Owner: Developer B  
Blocked by: Milestone 1 API

Done when:

- `demo-agents/research-agent/run --scenario legit-x402` calls the real vault API.
- `demo-agents/research-agent/run --scenario prompt-injection` calls the real vault API.
- Both scenarios are deterministic without external LLM credentials.
- Output includes request hash, decision, deny code or tx/mock proof, policy hash and audit event id.

### Milestone 4 - Open Agents sponsor adapters

Owner: Developer B  
Support: Developer A for receipt/API changes

Priority:

1. ENS identity proof.
2. KeeperHub guarded execution.
3. Uniswap guarded swap.
4. Gensyn AXL buyer/seller payment only if time permits.
5. 0G storage/framework proof only if time permits.

Done when:

- `demo-scripts/sponsors/ens-agent-identity.sh` resolves or mocks configured ENS records and verifies active policy hash.
- `demo-scripts/sponsors/keeperhub-guarded-execution.sh` shows denied actions never reach execution and approved actions are routed.
- `demo-scripts/sponsors/uniswap-guarded-swap.sh` shows quote/risk checks, if included.

### Milestone 5 - Final demo package

Owner: Developer B  
Support: Developer A for bug fixes

Done when:

- One command runs the full demo:

```bash
bash demo-scripts/run-openagents-final.sh
```

- It shows:
  - agent identity,
  - legitimate action approved,
  - KeeperHub/Uniswap action routed after approval,
  - malicious action denied,
  - policy receipt,
  - audit verification,
  - ENS trust metadata.

---

## 4. Open Agents required scope

This is mandatory for the Open Agents submission:

| Feature | Owner | Required? |
|---|---|---:|
| APRP/payment intent schema | A | yes |
| Policy engine | A | yes |
| Budget checks | A | yes |
| Policy receipts | A | yes |
| Audit chain | A | yes |
| Real agent harness | B | yes |
| ENS agent identity proof | B | yes |
| KeeperHub guarded execution | B | yes |
| Uniswap guarded swap | B | should |
| Gensyn AXL buyer/seller | B | optional |
| 0G storage/plugin proof | B | optional |
| Full HSM/TPM | A | no |
| Full TEE | A | no |
| Full marketplace | B | no |
| ZK proofs | A/B | no |

---

## 5. Daily handoff contract

Every day both developers write:

```text
Changed:
- files/modules

Contracts changed:
- schemas/openapi/error codes, or none

Demo gates:
- passed / failed

Blockers:
- exact blocker

Next:
- next 1-3 tasks
```

No silent contract changes. If `/schemas`, `17_interface_contracts.md`, `docs/api/openapi.json` or error codes change, both developers must rebase their work mentally and rerun contract tests.

---

## 6. Stop doing list

Do not build before Open Agents demo is stable:

- full marketplace,
- full TDX/SEV-SNP,
- full HSM integration,
- mobile app,
- hosted SaaS,
- ZK proof,
- multi-chain support beyond what the sponsor adapter needs,
- complex UI.

Use CLI/static dashboard first. Polish only after the core demo runs.

---

## 7. Success definition

Two developers are done when a judge can see this in under three minutes:

1. "This agent has an ENS identity and policy."
2. "It asks to do a legitimate onchain/payment action."
3. "SBO3L allows it and emits a receipt."
4. "The action routes to a sponsor-native integration."
5. "Prompt injection tries to spend incorrectly."
6. "SBO3L denies it before execution."
7. "Audit verifies the whole thing."

If that works, the project is contest-ready.
