# H. Data Flow

## H.1 Hlavný payment flow (14 krokov)

Scenár: AI agent volá platené API. API odpovie HTTP 402 s x402 challenge. Agent potrebuje zaplatiť a opakovať request.

| # | Krok | Aktér | Vstup | Výstup | Trust zone |
|---|---|---|---|---|---|
| 1 | Volanie plateného API | Agent | request bez paymentu | HTTP 402 + x402 challenge body | 1 → internet |
| 2 | Parsovanie 402 challenge | Agent | x402 headers + body | parsed challenge object | 1 |
| 3 | Vytvorenie payment requestu | Agent → Vault | APRP JSON (viď G.1) | `payment_request_id`, status `received` | 1 → 2 |
| 4 | Schema + auth validácia | Gateway (Zone 2) | APRP JSON, agent mTLS | normalizovaný internal request | 2 |
| 5 | x402 challenge verifikácia | Vault x402 verifier | challenge + provider TLS info | `x402_verified=true/false`, expected payload | 2 → 3 |
| 6 | Policy evaluation | Policy engine (Zone 3) | request + agent profile + state | `decision: allow / deny / escalate` + reason | 3 |
| 7 | Budget check & reserve | Budget ledger | decision + amount | reservation alebo "over budget" | 3 |
| 8 | Transaction simulation | Simulator | tx template | `simulated_ok=true` + expected balance changes | 3 → RPC (read-only) |
| 9 | (Optional) Human approval | Approval gateway | escalation context | `approved` / `rejected` / `timeout` | 3 ↔ 5 |
| 10 | Decision signing | Decision signer (Zone 3) | normalized tx + decision metadata | signed decision token | 3 |
| 11 | Signing | HSM/TEE signer (Zone 4) | tx template + decision token + attestation | tx signature | 4 |
| 12 | Audit write | Audit writer (Zone 6) | event with hashes + signature | audit_event_id, hash chain extended | 4/3 → 6 |
| 13 | Payment delivery | Vault → x402 provider | HTTP request s `X-Payment` header (signed payload) | provider response (HTTP 200 + payment receipt) | vault → internet |
| 14 | Settlement zápis | Vault | provider receipt + on-chain confirmation (ak je) | budget commit alebo release | 3 → 6 |

Po kroku 14 vault vráti agentovi finálny status a (ak je) data response.

---

## H.2 ASCII diagram (high-level)

```
                            ┌──────────────────────────────────────────────┐
                            │          INTERNET / EXTERNAL APIs            │
                            └──────────────────┬───────────────────────────┘
                                               │ 1. HTTP 402 challenge
                                               ▼
┌────────────────────────────────────────────────────────────────────────┐
│  ZONE 1: UNTRUSTED — Agent runtime, LLM, tools                          │
│                                                                         │
│   ┌────────────┐    2. parse 402     ┌─────────────────┐                │
│   │   AGENT    ├────────────────────▶│  payment intent │                │
│   └────┬───────┘                     └────────┬────────┘                │
│        │                                      │ 3. POST /v1/payment-requests
└────────┼──────────────────────────────────────┼────────────────────────┘
         │                                      │ (Unix socket / vsock / mTLS)
         ▼                                      ▼
┌────────────────────────────────────────────────────────────────────────┐
│  ZONE 2: CONTROLLED — Gateway, validator                                │
│                                                                         │
│   ┌────────────────┐  4. schema      ┌─────────────────┐                │
│   │  Auth (mTLS)   ├────────────────▶│  Normalizer     │                │
│   └────────────────┘                 └────────┬────────┘                │
└──────────────────────────────────────────────┼─────────────────────────┘
                                               │ normalized request
                                               ▼
┌────────────────────────────────────────────────────────────────────────┐
│  ZONE 3: TRUSTED POLICY (TEE in V4)                                     │
│                                                                         │
│   ┌────────────┐ 5    ┌────────────┐ 6    ┌────────────┐ 7              │
│   │ x402 verify├─────▶│ Policy eval├─────▶│Budget check│                │
│   └────────────┘      └────────────┘      └─────┬──────┘                │
│                                                 │                       │
│   ┌────────────┐                                │                       │
│   │ Simulator  │◀───────────────────── 8 ───────┘                       │
│   └─────┬──────┘                                                        │
│         │                                                               │
│         │ 9 (if escalation needed)              ┌────────────────────┐  │
│         ├──────────────────────────────────────▶│ Approval gateway   │  │
│         │                                       └─────────┬──────────┘  │
│         │                                                 │             │
│         │ ◀────────── admin signed approval ──────────────┘             │
│         │                                                               │
│         ▼  10                                                           │
│   ┌─────────────────┐                                                   │
│   │ Decision signer │ → signed decision token                           │
│   └────────┬────────┘                                                   │
└────────────┼─────────────────────────────────────────────────┬──────────┘
             │                                                 │
             ▼ signed decision + tx template                   │
┌─────────────────────────────────────────────┐                │
│  ZONE 4: SIGNING (HSM / TEE-sealed)         │                │
│                                             │                │
│   ┌─────────────────────────┐               │                │
│   │ Verify decision token   │               │                │
│   │ Verify attestation      │               │                │
│   │ Sign with HSM key       │               │                │
│   └────────────┬────────────┘               │                │
└────────────────┼────────────────────────────┘                │
                 │ signature                                   │
                 │                                             │
                 ├─────────────────────────────────────────────┤
                 │                                             │
                 ▼  12. event                                  ▼  12. event
┌─────────────────────────────────────────────────────────────────────────┐
│  ZONE 6: AUDIT (append-only, hash-chained, signed)                      │
│                                                                         │
│   request_received → x402_verified → policy_decided → simulated         │
│   → (approved?) → decision_signed → tx_signed → tx_broadcast            │
│   → settled / failed                                                    │
└─────────────────────────────────────────────────────────────────────────┘
                                              │
                                              ▼
                       13. POST x402 endpoint with X-Payment header
                                              │
                                              ▼
                                    Provider returns 200 + receipt
                                              │
                                              ▼
                       14. settlement event → budget commit/release
                                              │
                                              ▼
                                Response back to agent (Zone 1)
```

---

## H.3 Sequence diagram (UML-style, textový)

```
Agent       Gateway     PolicyEng    Sim      Approval   Signer     HSM      Audit     Provider
  │            │             │         │          │         │         │         │          │
  │── POST ───▶│             │         │          │         │         │         │          │
  │            │── normalize▶│         │          │         │         │         │          │
  │            │             │── x402 verify ────────────────────────────────▶  │  (read)  │
  │            │             │── policy eval                                                │
  │            │             │── budget check                                               │
  │            │             │── simulate ──────▶│         │          │         │          │
  │            │             │◀── ok ────────────│         │          │         │          │
  │            │             │── (if escalate) ──────────▶│           │         │          │
  │            │             │                            │ wait...   │         │          │
  │            │             │◀── approved ───────────────│           │         │          │
  │            │             │── sign decision ─────────────────────▶│           │         │
  │            │             │                                       │── sign ─▶│          │
  │            │             │                                       │◀── sig ──│          │
  │            │             │── audit event ──────────────────────────────────▶│          │
  │            │             │── deliver payment ──────────────────────────────────────────▶│
  │            │             │◀── 200 OK + receipt ─────────────────────────────────────────│
  │            │             │── settle / commit ──────────────────────────────▶│          │
  │            │◀── result ──│         │          │         │         │         │          │
  │◀── 200 ────│             │         │          │         │         │         │          │
```

---

## H.4 Alternatívne flow scenáre

### Scenár B — Direct ERC-20 transfer (bez x402)
- Krok 5 (x402 verify) sa preskakuje.
- Krok 8 simulator overí, že `transfer(to, amount)` ide na známy recipient z policy allowlist.
- Krok 13 sa nahradí broadcastom on-chain transakcie + čakaním na confirmation.

### Scenár C — Stream payment (super-fluid štýl)
- Krok 11 podpíše stream creation (napr. Sablier/Superfluid).
- Krok 14 namiesto jednorazového commitu monitoruje stream usage a periodicky updatuje budget.

### Scenár D — Smart account session key issuance
- Vault podpíše *session key authorization* pre kontrakt.
- Agent dostane session key handle; vault aj naďalej brán session key v signing zone.
- Každé použitie session key prejde policy znova (key handle nie je voľný credential).

### Scenár E — Recovery flow
- Triggered manually multisig adminmi.
- Vault enter `recovery_mode`, žiadne payments.
- Recovery instructions: rotate compromised key (HSM keygen + on-chain key update na smart accountoch), update policy, znovu attest.

---

## H.5 Latency targets (SLO indikatívne)

| Krok | P50 | P99 |
|---|---|---|
| Schema + auth (4) | 5 ms | 20 ms |
| x402 verify (5) | 50 ms | 300 ms (incl. cert pin / DNS) |
| Policy eval (6) | 5 ms | 50 ms |
| Budget check (7) | 5 ms | 30 ms |
| Simulation (8) | 100 ms | 1000 ms |
| Sign (10–11) | 50 ms HSM, 200 ms TEE | 500 ms / 1500 ms |
| Audit write (12) | 5 ms | 30 ms |
| End-to-end (3 → 14, no human) | **~250 ms** | **~2 s** |
| End-to-end (s human approval) | minutes | TTL (5 min default) |

Cieľ: pri auto-approved x402 paymentoch byť pod 1 sekundu P50 — porovnateľné s bežným HTTP volaním.
