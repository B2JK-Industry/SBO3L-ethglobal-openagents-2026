"""SBO3L wire types as Pydantic v2 strict models.

Mirrors the JSON Schemas in `schemas/`:
  - aprp_v1.json
  - policy_receipt_v1.json
  - sbo3l.passport_capsule.v1.json

Capsule v2 (F-6) is additive over v1 (`policy.policy_snapshot` and
`audit.audit_segment` optional fields). The v2 model is declared here as a
narrow extension that re-exports cleanly once the F-6 schema lands.

Source of truth: the JSON Schemas. If any field drifts, update the schema
first, then re-mirror here.

All models have `model_config = ConfigDict(extra="forbid", frozen=True)` to
enforce `additionalProperties: false` end-to-end and rule out post-construct
mutation of wire envelopes.
"""

from __future__ import annotations

from typing import Annotated, Any, Literal

from pydantic import BaseModel, ConfigDict, Field

# ---------------------------------------------------------------------------
# Newtype-style aliases (Annotated[str, Field(pattern=...)] for runtime check)
# ---------------------------------------------------------------------------

#: Stable agent slug. `^[a-z0-9][a-z0-9_-]{2,63}$`.
AgentId = Annotated[str, Field(pattern=r"^[a-z0-9][a-z0-9_-]{2,63}$")]

#: ULID. `^[0-7][0-9A-HJKMNP-TV-Z]{25}$`.
Ulid = Annotated[str, Field(pattern=r"^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]

#: Address. `^0x[a-fA-F0-9]{40}$`.
Address = Annotated[str, Field(pattern=r"^0x[a-fA-F0-9]{40}$")]

#: Lower-case hex SHA-256.
Hash256 = Annotated[str, Field(pattern=r"^[a-f0-9]{64}$")]

#: Lower-case hex Ed25519 signature.
SignatureHex = Annotated[str, Field(pattern=r"^[a-f0-9]{128}$")]


# ---------------------------------------------------------------------------
# Strict-frozen base
# ---------------------------------------------------------------------------


class _Strict(BaseModel):
    """Common config: deny unknown fields, freeze instances, populate by alias."""

    model_config = ConfigDict(
        extra="forbid",
        frozen=True,
        populate_by_name=True,
        # str_strip_whitespace=False — wire envelopes may carry significant
        # whitespace; never silently mutate.
    )


# ---------------------------------------------------------------------------
# APRP v1
# ---------------------------------------------------------------------------

AprpIntent = Literal[
    "purchase_api_call",
    "purchase_dataset",
    "pay_compute_job",
    "pay_agent_service",
    "tip",
]
AprpRiskClass = Literal["low", "medium", "high", "critical"]
AprpPaymentProtocol = Literal["x402", "l402", "erc20_transfer", "smart_account_session"]


class AprpMoney(_Strict):
    value: Annotated[str, Field(pattern=r"^(0|[1-9][0-9]*)(\.[0-9]{1,18})?$")]
    currency: Literal["USD"]


class AprpDestinationX402Endpoint(_Strict):
    type: Literal["x402_endpoint"]
    url: Annotated[str, Field(max_length=2048)]
    method: Literal["GET", "POST", "PUT", "PATCH", "DELETE"]
    expected_recipient: Address | None = None


class AprpDestinationEoa(_Strict):
    type: Literal["eoa"]
    address: Address


class AprpDestinationSmartAccount(_Strict):
    type: Literal["smart_account"]
    address: Address


class AprpDestinationErc20Transfer(_Strict):
    type: Literal["erc20_transfer"]
    token_address: Address
    recipient: Address


AprpDestination = Annotated[
    (
        AprpDestinationX402Endpoint
        | AprpDestinationEoa
        | AprpDestinationSmartAccount
        | AprpDestinationErc20Transfer
    ),
    Field(discriminator="type"),
]


class AprpExpectedResult(_Strict):
    kind: Literal["json", "file", "receipt", "none"]
    sha256: Hash256 | None = None
    content_type: Annotated[str, Field(max_length=128)] | None = None


class PaymentRequest(_Strict):
    """Full APRP v1 payload — mirrors `schemas/aprp_v1.json`."""

    agent_id: AgentId
    task_id: Annotated[str, Field(pattern=r"^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$")]
    intent: AprpIntent
    amount: AprpMoney
    token: Annotated[str, Field(pattern=r"^[A-Z0-9]{2,16}$")]
    destination: AprpDestination
    payment_protocol: AprpPaymentProtocol
    chain: Annotated[str, Field(pattern=r"^[a-z0-9][a-z0-9_-]{1,31}$")]
    provider_url: Annotated[str, Field(max_length=2048)]
    x402_payload: dict[str, Any] | None = None
    expiry: str  # RFC 3339; runtime parse is caller's choice
    nonce: Ulid
    expected_result: AprpExpectedResult | None = None
    risk_class: AprpRiskClass


# ---------------------------------------------------------------------------
# Policy Receipt v1
# ---------------------------------------------------------------------------

Decision = Literal["allow", "deny", "requires_human"]


class ReceiptSignature(_Strict):
    algorithm: Literal["ed25519"]
    key_id: Annotated[str, Field(min_length=3, max_length=128)]
    signature_hex: SignatureHex


class PolicyReceipt(_Strict):
    receipt_type: Literal["sbo3l.policy_receipt.v1"]
    version: Literal[1]
    agent_id: AgentId
    decision: Decision
    deny_code: Annotated[str, Field(pattern=r"^[a-z]+\.[a-z0-9_]+$")] | None = None
    request_hash: Hash256
    policy_hash: Hash256
    policy_version: Annotated[int, Field(ge=1)] | None = None
    audit_event_id: Annotated[str, Field(pattern=r"^evt-[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    execution_ref: Annotated[str, Field(max_length=256)] | None = None
    issued_at: str
    expires_at: str | None = None
    signature: ReceiptSignature


# ---------------------------------------------------------------------------
# Server response shape (matches PaymentRequestResponse in sbo3l-server)
# ---------------------------------------------------------------------------

PaymentStatus = Literal["auto_approved", "rejected", "requires_human"]


class PaymentRequestResponse(_Strict):
    status: PaymentStatus
    decision: Decision
    deny_code: str | None = None
    matched_rule_id: str | None = None
    request_hash: Hash256
    policy_hash: Hash256
    audit_event_id: str
    receipt: PolicyReceipt


# ---------------------------------------------------------------------------
# RFC 7807 problem detail
# ---------------------------------------------------------------------------


class ProblemDetail(_Strict):
    type: str
    title: str
    status: int
    detail: str
    code: str


# ---------------------------------------------------------------------------
# Passport Capsule v1
# ---------------------------------------------------------------------------

CapsuleResolver = Literal["offline-fixture", "live-ens"]


class CapsuleAgent(_Strict):
    agent_id: AgentId
    ens_name: str | None = None
    resolver: CapsuleResolver
    records: dict[str, str] | None = None


class CapsuleRequest(_Strict):
    aprp: dict[str, Any]
    request_hash: Hash256
    idempotency_key: str | None = None
    nonce: str | None = None


class CapsulePolicyV1(_Strict):
    policy_hash: Hash256
    policy_version: Annotated[int, Field(ge=1)]
    activated_at: str | None = None
    source: Annotated[str, Field(min_length=1)]


class CapsuleDecision(_Strict):
    result: Literal["allow", "deny"]
    matched_rule: str | None = None
    deny_code: str | None = None
    receipt: PolicyReceipt
    receipt_signature: SignatureHex


CapsuleExecutor = Literal["keeperhub", "uniswap", "none"]
CapsuleExecutionMode = Literal["mock", "live"]
CapsuleExecutionStatus = Literal["submitted", "succeeded", "denied", "not_called"]


class CapsuleLiveEvidence(_Strict):
    transport: Annotated[str, Field(min_length=1)] | None = None
    response_ref: Annotated[str, Field(min_length=1)] | None = None
    block_ref: Annotated[str, Field(min_length=1)] | None = None


class CapsuleExecution(_Strict):
    executor: CapsuleExecutor
    mode: CapsuleExecutionMode
    execution_ref: str | None = None
    status: CapsuleExecutionStatus
    sponsor_payload_hash: Hash256 | None = None
    live_evidence: CapsuleLiveEvidence | None = None
    #: Sponsor-specific evidence (e.g. Uniswap quote shape, KH IP-1 envelope).
    #: Mode-agnostic; allowed in both mock and live modes (P6.1 schema bump).
    executor_evidence: dict[str, Any] | None = None


class CapsuleAuditCheckpoint(_Strict):
    schema_id: Literal["sbo3l.audit_checkpoint.v1"] = Field(alias="schema")
    sequence: Annotated[int, Field(ge=1)]
    latest_event_id: str | None = None
    latest_event_hash: Hash256
    chain_digest: Hash256
    mock_anchor: Literal[True]
    mock_anchor_ref: Annotated[str, Field(pattern=r"^local-mock-anchor-[0-9a-f]{16}$")]
    created_at: str | None = None


class CapsuleAuditV1(_Strict):
    audit_event_id: Annotated[str, Field(min_length=1)]
    prev_event_hash: Hash256
    event_hash: Hash256
    bundle_ref: str | None = None
    checkpoint: CapsuleAuditCheckpoint | None = None


DoctorStatus = Literal["ok", "warn", "skip", "fail", "not_run"]


class CapsuleVerification(_Strict):
    doctor_status: DoctorStatus
    offline_verifiable: bool
    live_claims: list[str]


class PassportCapsuleV1(_Strict):
    schema_id: Literal["sbo3l.passport_capsule.v1"] = Field(alias="schema")
    generated_at: str
    agent: CapsuleAgent
    request: CapsuleRequest
    policy: CapsulePolicyV1
    decision: CapsuleDecision
    execution: CapsuleExecution
    audit: CapsuleAuditV1
    verification: CapsuleVerification


# ---------------------------------------------------------------------------
# Passport Capsule v2 (additive over v1; finalised once F-6 schema lands)
# ---------------------------------------------------------------------------

#: Optional `policy_snapshot` body. The full canonical policy JSON, so that
#: `--strict` re-derives `policy_hash` without `--policy <path>`.
#:
#: The exact shape will be stamped from `schemas/policy_v1.json` when F-6
#: lands. Until then, `dict[str, Any]` keeps the SDK forward-compatible with
#: any v2 capsule the daemon emits.
CapsulePolicySnapshot = dict[str, Any]


class CapsulePolicyV2(CapsulePolicyV1):
    """v2 policy block: v1 plus optional `policy_snapshot`."""

    policy_snapshot: CapsulePolicySnapshot | None = None


#: Optional `audit_segment` body. Bundle-shaped segment of the audit chain
#: so `--strict` walks the chain without `--audit-bundle <path>`. Exact shape
#: stamped once F-6 lands.
CapsuleAuditSegment = dict[str, Any]


class CapsuleAuditV2(CapsuleAuditV1):
    """v2 audit block: v1 plus optional `audit_segment`."""

    audit_segment: CapsuleAuditSegment | None = None


class PassportCapsuleV2(_Strict):
    schema_id: Literal["sbo3l.passport_capsule.v2"] = Field(alias="schema")
    generated_at: str
    agent: CapsuleAgent
    request: CapsuleRequest
    policy: CapsulePolicyV2
    decision: CapsuleDecision
    execution: CapsuleExecution
    audit: CapsuleAuditV2
    verification: CapsuleVerification


PassportCapsule = PassportCapsuleV1 | PassportCapsuleV2
