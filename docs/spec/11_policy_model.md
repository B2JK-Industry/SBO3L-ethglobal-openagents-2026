# K. Policy Model

## K.0 Filozofia

- **Human-readable** — policy je vlastnou dokumentáciou.
- **Strojovo evaluovateľná** — kompiluje sa do Rego (OPA) alebo CEL.
- **Verzovaná, podpísaná, immutable.**
- **Kompozičná** — možnosť `extends` z baseline policy (napr. `extends: policy://default-low-risk`).
- **Dvojvrstvová** — `allow` lists + explicit `deny` (deny vyhráva).
- **Risk-aware** — pre rôzne risk class rôzne pravidlá.

---

## K.1 Štruktúra YAML

```yaml
version: 42
extends: policy://default-low-risk     # voliteľne
agent_id: research-agent-01            # alebo agent_group: research-team

limits:
  max_per_payment_usd: 0.25
  max_daily_usd: 10
  max_weekly_usd: 50
  max_monthly_usd: 100
  max_per_provider_daily_usd:
    "api.example.com": 5
    "obolos.tech": 3
  max_per_token_daily_usd:
    USDC: 10
  max_per_task_usd: 1
  max_requests_per_minute: 30
  max_requests_per_hour: 600

allowed:
  protocols:
    - x402
    - direct_transfer
  chains:
    - base
    - polygon
  tokens:
    - USDC
    - USDT
  providers:
    - id: api.example.com
      cert_pin: "sha256/AAAA..."
      reputation_min: 50
    - id: obolos.tech
      cert_pin: "sha256/BBBB..."
  recipients:
    - chain: base
      address: "0xAbC...111"
      label: "trusted-inference-receiver"
  methods:
    - GET
    - POST
  contract_methods:                    # whitelist for direct_transfer
    USDC:
      - "transfer(address,uint256)"
      - "transferWithAuthorization(...)"
  intents:
    - purchase_api_call
    - purchase_dataset

approval_required:
  if_amount_over_usd: 1
  if_new_provider: true
  if_new_recipient: true
  if_new_token: true
  if_new_chain: true
  if_policy_change: true
  if_daily_soft_cap_exceeded:
    threshold_usd: 8                   # menej než hard cap (10)
  if_anomaly_score_over: 0.7
  approvers_needed: 1                  # M-of-N pre tento subset
  ttl_seconds: 300

deny:
  unknown_contract_calls: true
  raw_native_transfers: true           # native ETH/MATIC mimo allowlist
  unverified_x402_challenge: true
  blacklisted_recipients:
    - chain: base
      address: "0xDEAD...beef"
  blacklisted_providers:
    - "fishy-provider.example"
  destination_country_codes:           # GeoIP-based, optional
    - "KP"
    - "IR"
  during_emergency: true               # explicit fail-closed counterpart
  if_simulator_disagreement: true
  if_attestation_drift: true

risk_overrides:
  high_risk_calldata:
    if_calldata_size_over_bytes: 200
    treat_as: high_risk
  high_risk_actions:
    - require_human_approval: true
    - require_simulator_match_strict: true
    - require_attestation_max_age_seconds: 60

simulation:
  enabled: true
  required: true
  match_tolerance_percent: 1
  rpc_quorum_min: 2

signing:
  key_ref: agent-research-01-key
  attestation_required: true
  multisig_required: false

audit:
  full_payload_in_log: false           # iba hashe (default)
  on_chain_anchor: true
  anchor_chain: base
  anchor_period_hours: 24

emergency:
  per_agent_kill_switch: true
  freeze_on_anomaly_score_over: 0.95
```

---

## K.2 Evaluation order

Policy engine vyhodnocuje v presnom poradí (dvojvrstvový allow/deny model):

1. **`emergency.frozen` check** → `deny` ak `EmergencyState.frozen == true` (a politika `deny.during_emergency: true`).
2. **`deny` rules** → ak match, okamžite `deny`.
3. **`allowed.*` whitelist match** → ak nie je v allowliste, `deny` (s reason).
4. **`limits.*` numeric caps** → ak prekročené hard cap, `deny`.
5. **`approval_required.*` triggers** → ak match a request nie je už approved, `escalate`.
6. **`risk_overrides`** → môže prepísať na `escalate` alebo `deny`.
7. **`simulation.required`** → ak fail, `deny`.
8. **`signing.attestation_required`** → kontrola pred odovzdaním do signera.
9. Inak → `allow`.

**Princíp:** `deny` **vždy** vyhráva. `escalate` má nižšiu prioritu než `deny`.

---

## K.3 Kompilácia do Rego (príklad)

```rego
package mandate.policy

default decision := {"action": "deny", "reason": "default fail-closed"}

decision := {"action": "deny", "reason": "emergency frozen"} if {
    input.emergency_state.frozen
    data.policy.deny.during_emergency
}

decision := {"action": "deny", "reason": "blacklisted recipient"} if {
    some r in data.policy.deny.blacklisted_recipients
    input.request.destination.address == r.address
    input.request.chain == r.chain
}

decision := {"action": "deny", "reason": "unknown provider"} if {
    not provider_allowed
}

provider_allowed if {
    some p in data.policy.allowed.providers
    p.id == input.request.provider_url_host
}

decision := {"action": "deny", "reason": "exceeds per_payment hard cap"} if {
    input.request.amount_usd > data.policy.limits.max_per_payment_usd
}

decision := {"action": "escalate", "reason": "amount over approval threshold"} if {
    input.request.amount_usd > data.policy.approval_required.if_amount_over_usd
    not already_approved
}

decision := {"action": "allow"} if {
    not deny_match
    allow_match
    within_limits
    not escalation_needed
}
```

(Skrátené; production verzia má desiatky pravidiel, plne unit-tested.)

---

## K.4 Default policies (shipped baseline)

### `policy://default-deny-all` (highest safety)
- `allowed: {}` — nič nie je povolené.
- Užitočná ako baseline pre `extends`.

### `policy://default-low-risk`
- `max_per_payment_usd: 0.10`, `max_daily_usd: 1`.
- Iba x402 + USDC + Base.
- `if_new_provider: true`, `if_new_recipient: true`.
- Vhodné pre prvotný experiment.

### `policy://default-research`
- Vyššie limity (`max_daily_usd: 25`).
- Allowlist providerov rozšírený o curated research sources.
- `risk_overrides.high_risk_actions.require_human_approval: true`.

### `policy://default-trader` (advanced)
- Multisig pre treasury ops.
- `signing.multisig_required: true` pre nad threshold.
- `audit.on_chain_anchor: true` mandatorne.

---

## K.5 Policy lifecycle

1. **Draft** — admin pripraví YAML, `mandate policy lint` lokálne.
2. **Sign** — admin (alebo M-of-N) podpíše canonical hash.
3. **Submit** — `PATCH /v1/agents/{id}/policies` so podpisami.
4. **Validation** — vault overí podpisy + lint + (optional) dry-run nad poslednými 100 requestami.
5. **Activation** — atomický switch; staré requesty in-flight sa dohrávajú nad starou verziou.
6. **Deprecation** — predchádzajúca verzia označená `deprecated`, ostáva v DB pre forensics.
7. **Revocation** — môže byť explicitne `revoked` (napr. po incidente).

---

## K.6 Policy linter pravidlá

- Žiadne `cap_usd: 0` bez explicitného `hard_cap: true` (asi sa zabudlo).
- `allowed.providers` non-empty (alebo explicitné `extends`).
- `approval_required.ttl_seconds` v rozumnom rozsahu (60–3600).
- Unique recipient/provider entries.
- Cert pin je validný sha256 hex/base64.
- `risk_overrides` referencujú existujúce risk classes.
- Súčet `per_provider_daily` ≤ `max_daily_usd` (warning, nie error).

---

## K.7 Príklad escalation flow (textový)

```
PaymentRequest amount=$1.50 → policy threshold $1.00 → ESCALATE
   │
   ▼
ApprovalRequest created (TTL 5 min)
   │
   ▼ notification (web/push/CLI)
Admin pozre request, podpíše decision
   │
   ▼
ApprovalDecision (signature verified)
   │
   ▼
PaymentRequest re-evaluation → ALLOW
   │
   ▼
Decision signed → HSM signs → audit → settle
```

---

## K.8 Otvorené body (riešené v rámci `14_open_questions.md`)

- Final voľba DSL (Rego vs. CEL vs. vlastné).
- Spôsob distribúcie default policies (signed bundle?).
- Konkrétny anomaly score model.
- Multi-tenant podpora policy (rôzni admini pre rôznych agentov).
