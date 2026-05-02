# PCI-DSS scope assessment

> **Status:** 🟢 SBO3L is **out-of-scope by design** for PCI-DSS. Customers using SBO3L do **not** expand their PCI scope to include the SBO3L daemon, audit DB, or capsule storage, **provided** they follow the architectural separation in this document.

## TL;DR

> SBO3L processes **agent decisions about payment intents** — it does not process, store, or transmit **cardholder data** (CHD) or **sensitive authentication data** (SAD) as defined by PCI-DSS v4.0. The capsule schema is structurally incapable of containing a PAN (primary account number), CVV, or magnetic-stripe data.

## What PCI-DSS regulates

PCI-DSS scope is determined by where CHD/SAD lives and flows. CHD is:
- Primary Account Number (PAN)
- Cardholder name (when stored alongside PAN)
- Service code
- Expiration date

SAD is:
- Full magnetic-stripe data / equivalent on chip
- CAV2/CVC2/CVV2/CID
- PIN / PIN block

A system is **in scope** if it stores, processes, or transmits CHD/SAD, **or** is connected to a system that does without proper segmentation.

## SBO3L's data flow

SBO3L's payment-related data flow:

```
Agent ──APRP──> SBO3L daemon ──policy decision──> SBO3L daemon ──signed receipt──> Sponsor adapter ──> Payment processor / network
```

The **APRP** (Agent Payment Request Protocol) envelope SBO3L receives contains:
- `agent_id` — ENS name or pubkey hash
- `nonce` — ULID, application-defined
- `expiry` — RFC 3339 timestamp
- `intent.amount` — numeric (e.g. `100`) + `currency` (e.g. `"USDC"`)
- `intent.recipient` — string (e.g. an Ethereum address or KH workflow ID)
- `intent.metadata` — opaque object (customer-controlled)

**None of these fields are CHD or SAD.** The "amount + recipient" is an instruction; the actual payment instrument (a credit card PAN, an ACH account number, a stablecoin contract address) lives **downstream of SBO3L** in the payment processor that the sponsor adapter calls.

## What if a customer puts CHD in `intent.metadata`?

The metadata field is opaque to SBO3L — we don't validate it. A customer **could** technically stuff a PAN into `intent.metadata`, but doing so:
1. **Brings their SBO3L deployment into PCI scope.** (We document this.)
2. **Exposes the PAN in the audit log** in plaintext (capsule encodes the field bytes).
3. **Goes against documented best practice** ([`docs/compliance/pci-dss-scope.md#out-of-scope-architecture`](pci-dss-scope.md)).

We **strongly recommend** customers tokenize before putting any payment-instrument identifier into APRP — pass the tokenized identifier (e.g. a Stripe `pm_*` token) through `intent.metadata`, and let the payment processor map back to the PAN downstream. The token IS NOT CHD per PCI-DSS Tokenization v4.0.

## Out-of-scope architecture

> The "right way" to deploy SBO3L is to keep it **outside** your CHD environment.

```
                          ╔═══════════════ PCI scope (CHD env) ═══════════════╗
Agent                     ║                                                    ║
  │                       ║   Tokenization service                             ║
  │ APRP w/ pm_token       ║      │                                            ║
  ▼                       ║      ▼ PAN                                         ║
SBO3L daemon  ─────RPC──> ║   Payment processor (Stripe / Adyen / ACH switch) ║
  │                       ║      │                                            ║
  │ signed receipt        ║      ▼                                            ║
  ▼                       ║   Card network                                    ║
Audit DB (SQLite)         ╚════════════════════════════════════════════════════╝
                                  ▲
                                  │
                                  │ NO CHD in APRP, NO CHD in audit row
```

In this architecture:
- The **tokenization service + payment processor** are in PCI scope.
- The **agent + SBO3L daemon + audit DB** are out of scope.
- The boundary is the `pm_token` → PAN dereference, which happens entirely inside the PCI-scope.

## SAQ guidance

Customers that integrate SBO3L:
- **SAQ A** — if you outsource all CHD handling (e.g. Stripe Checkout, hosted payment page) and SBO3L only touches tokenized references, this is your level.
- **SAQ A-EP** — if you have a server-side payment integration but tokens are still SBO3L's only payment input.
- **SAQ D** — if you store CHD on your own systems. SBO3L doesn't help or hurt this — it's a question of your CHD storage.

In **none** of these does SBO3L itself enter PCI scope.

## Specific PCI-DSS v4.0 requirements

| Requirement | Applies to SBO3L? | Notes |
|---|---|---|
| Req 1 — Network security controls | _customer scope_ | Customer firewalls SBO3L behind a non-PCI segment |
| Req 2 — Apply secure configurations | _customer scope_ for SBO3L deploy; _SBO3L scope_ for SBO3L source | We ship secure defaults (F-1 default-deny auth) |
| Req 3 — Protect stored account data | **N/A** | SBO3L stores no account data |
| Req 4 — Protect cardholder data with strong cryptography | **N/A** | No CHD in transit through SBO3L |
| Req 5 — Anti-malware | _customer scope_ | Standard server hardening |
| Req 6 — Secure software development | ✅ **partial** | SBO3L uses memory-safe Rust; Ed25519 + secp256k1 for signing; signed releases via OIDC. SOC 2 CC8 covers this. |
| Req 7 — Restrict access | **N/A directly** | Bearer + JWT auth at SBO3L; customer applies need-to-know |
| Req 8 — Identify users + auth | ✅ | F-1 + JWT `sub` binding |
| Req 9 — Restrict physical access | _delegated to cloud DC_ | |
| Req 10 — Log + monitor | ✅ **exemplary** | The audit chain is structurally aligned with Req 10's tamper-evident logging |
| Req 11 — Test security regularly | 🟡 | Chaos suite; cargo-audit (planned); SOC 2 audit pipeline |
| Req 12 — Information security policy | 🟡 | `SECURITY.md` + `SECURITY_NOTES.md` + `docs/compliance/` |

## Stronger statement

> **An honest claim:** an SBO3L deployment that follows the out-of-scope architecture above produces an **audit log that is structurally stronger than what most PCI Req 10 implementations achieve** — specifically:
>
> - **Req 10.2 (record events):** SBO3L records every policy decision with a `chain_hash_v2` that linearly chains all events; deletion or backdating breaks subsequent verification.
> - **Req 10.5 (secure audit trails):** Ed25519 signature over canonical bytes makes the audit chain forge-resistant in a way that filesystem permissions alone cannot.
> - **Req 10.7 (retention):** application-layer retention is configurable; the chain itself is portable to any storage.
>
> Customers seeking PCI Req 10 enhancement (separately from SBO3L's primary trust boundary purpose) can use SBO3L as a logging substrate for **non-CHD events** (agent decisions, batch jobs, admin actions) and benefit from tamper-evidence without expanding PCI scope.

## Honest gaps

- 🟢 **None for the out-of-scope architecture.** SBO3L plus PCI is a no-op.
- 🟡 **In-scope architecture is undocumented and discouraged.** A customer that intentionally puts SBO3L in PCI scope (e.g. for centralized logging of CHD) would need a Qualified Security Assessor (QSA) to walk through Req 1–12 against the deployment. Not a use case we're targeting.

## See also

- [`README.md`](README.md) — top-level compliance posture.
- [`audit-log-as-evidence.md`](audit-log-as-evidence.md) — Req 10 walkthrough.
- [`shared-controls.md`](shared-controls.md) — controls reusable across SOC 2 / GDPR / HIPAA / PCI.
