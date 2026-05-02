"""Client-side structural verifier for SBO3L Passport capsules (v1 + v2).

Mirrors `sdks/typescript/src/passport.ts` and `crates/sbo3l-core/src/passport.rs`'s
structural rules.

"Structural" means: shape + invariants + cross-field equality. It does NOT
recompute Ed25519 signatures or re-derive `request_hash` / `policy_hash` —
use the Rust CLI `sbo3l-cli passport verify --strict` for cryptographic
checks.

What the verifier catches client-side:
  - schema id unknown (not v1 or v2)
  - cross-field mismatches: `request.request_hash` ↔ `decision.receipt.request_hash`,
    `policy.policy_hash` ↔ `decision.receipt.policy_hash`,
    `decision.result` ↔ `decision.receipt.decision`,
    `audit.audit_event_id` ↔ `decision.receipt.audit_event_id`
  - hex shape rules (lowercase 64-hex / 128-hex)
  - deny capsules with `execution.status != "not_called"` (truthfulness)
  - live mode without `live_evidence` (live-evidence invariant)
  - mock mode with `live_evidence` (mock-only invariant)
  - mock anchor `mock_anchor: false` (PSM-A4 truthfulness rule)
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import Any, Literal

from .errors import PassportVerificationError

_HEX64 = re.compile(r"^[0-9a-f]{64}$")
_HEX128 = re.compile(r"^[0-9a-f]{128}$")
_MOCK_ANCHOR_REF = re.compile(r"^local-mock-anchor-[0-9a-f]{16}$")


@dataclass(frozen=True, slots=True)
class VerifyCheck:
    """One structural check's outcome."""

    code: str
    description: str
    passed: bool
    detail: str | None = None


@dataclass(frozen=True, slots=True)
class VerifyResult:
    """Aggregated verification result. `ok` iff every check passed."""

    ok: bool
    checks: tuple[VerifyCheck, ...]
    failures: tuple[VerifyCheck, ...]
    schema_version: Literal[1, 2] | None

    def codes(self) -> tuple[str, ...]:
        """Failure codes only — convenience for callers logging or branching."""

        return tuple(c.code for c in self.failures)


@dataclass(slots=True)
class _Acc:
    """Internal accumulator for checks, kept mutable inside `verify`."""

    checks: list[VerifyCheck] = field(default_factory=list)


def verify(capsule: Any) -> VerifyResult:
    """Run all structural checks against `capsule`. Never raises on a check failure.

    Pass an unknown payload (e.g. `json.loads(capsule_bytes)`); the verifier
    handles non-object inputs gracefully.
    """

    acc = _Acc()

    schema_passed = isinstance(capsule, dict) and capsule.get("schema") in {
        "sbo3l.passport_capsule.v1",
        "sbo3l.passport_capsule.v2",
    }
    if not schema_passed:
        detail = (
            f"capsule is not an object: type={type(capsule).__name__}"
            if not isinstance(capsule, dict)
            else f"unknown schema id: {capsule.get('schema')!r}"
        )
        acc.checks.append(
            VerifyCheck(
                code="capsule.schema_unknown",
                description=(
                    "capsule.schema is `sbo3l.passport_capsule.v1` or `sbo3l.passport_capsule.v2`"
                ),
                passed=False,
                detail=detail,
            )
        )
        return _finalize(acc, None)

    acc.checks.append(
        VerifyCheck(
            code="capsule.schema_unknown",
            description=(
                "capsule.schema is `sbo3l.passport_capsule.v1` or `sbo3l.passport_capsule.v2`"
            ),
            passed=True,
        )
    )
    schema_version: Literal[1, 2] = 2 if capsule["schema"] == "sbo3l.passport_capsule.v2" else 1

    _run_common_checks(capsule, acc)
    if schema_version == 2:
        _run_v2_only_checks(capsule, acc)

    return _finalize(acc, schema_version)


def verify_or_raise(capsule: Any) -> dict[str, Any]:
    """Run `verify`; raise `PassportVerificationError` on any failure.

    Returns the (caller-supplied) capsule dict on success so this can be
    chained ergonomically.
    """

    r = verify(capsule)
    if not r.ok:
        detail = "; ".join(c.detail or c.description for c in r.failures)
        raise PassportVerificationError(r.codes(), detail)
    if not isinstance(capsule, dict):  # pragma: no cover (verify already covers)
        raise PassportVerificationError(("capsule.schema_unknown",), "non-dict input")
    return capsule


# ---------------------------------------------------------------------------


def _finalize(acc: _Acc, schema_version: Literal[1, 2] | None) -> VerifyResult:
    failures = tuple(c for c in acc.checks if not c.passed)
    return VerifyResult(
        ok=len(failures) == 0,
        checks=tuple(acc.checks),
        failures=failures,
        schema_version=schema_version,
    )


def _run_common_checks(c: dict[str, Any], acc: _Acc) -> None:
    request = c.get("request") or {}
    policy = c.get("policy") or {}
    decision = c.get("decision") or {}
    receipt = decision.get("receipt") or {}
    execution = c.get("execution") or {}
    audit = c.get("audit") or {}
    verification = c.get("verification") or {}

    request_hash = request.get("request_hash", "")
    policy_hash = policy.get("policy_hash", "")
    audit_event_id = audit.get("audit_event_id", "")
    receipt_signature = decision.get("receipt_signature", "")

    _shape(
        acc,
        "capsule.request_hash_shape",
        "request.request_hash is 64-char lowercase hex",
        bool(_HEX64.match(request_hash)),
        request_hash,
    )
    _shape(
        acc,
        "capsule.policy_hash_shape",
        "policy.policy_hash is 64-char lowercase hex",
        bool(_HEX64.match(policy_hash)),
        policy_hash,
    )

    _match(
        acc,
        "capsule.request_hash_match_receipt",
        "request.request_hash equals decision.receipt.request_hash",
        request_hash,
        receipt.get("request_hash", ""),
    )
    _match(
        acc,
        "capsule.policy_hash_match_receipt",
        "policy.policy_hash equals decision.receipt.policy_hash",
        policy_hash,
        receipt.get("policy_hash", ""),
    )
    _match(
        acc,
        "capsule.decision_match_receipt",
        "decision.result equals decision.receipt.decision",
        decision.get("result", ""),
        receipt.get("decision", ""),
    )
    _match(
        acc,
        "capsule.audit_event_id_match_receipt",
        "audit.audit_event_id equals decision.receipt.audit_event_id",
        audit_event_id,
        receipt.get("audit_event_id", ""),
    )

    _shape(
        acc,
        "capsule.audit_event_hash_shape",
        "audit.event_hash is 64-char lowercase hex",
        bool(_HEX64.match(audit.get("event_hash", ""))),
        audit.get("event_hash", ""),
    )
    _shape(
        acc,
        "capsule.audit_prev_event_hash_shape",
        "audit.prev_event_hash is 64-char lowercase hex",
        bool(_HEX64.match(audit.get("prev_event_hash", ""))),
        audit.get("prev_event_hash", ""),
    )
    _shape(
        acc,
        "capsule.receipt_signature_shape",
        "decision.receipt_signature is 128-char lowercase hex",
        bool(_HEX128.match(receipt_signature)),
        "shape mismatch",
    )

    if decision.get("result") == "deny":
        passed = execution.get("status") == "not_called"
        _push(
            acc,
            "capsule.deny_status_must_be_not_called",
            "deny capsules must have execution.status === 'not_called'",
            passed,
            None if passed else f"got status={execution.get('status')!r}",
        )

    mode = execution.get("mode")
    live_evidence = execution.get("live_evidence")
    if mode == "live":
        has_evidence = isinstance(live_evidence, dict) and any(
            isinstance(live_evidence.get(k), str) and len(live_evidence[k]) > 0
            for k in ("transport", "response_ref", "block_ref")
        )
        _push(
            acc,
            "capsule.live_mode_requires_evidence",
            "execution.mode == 'live' requires non-empty live_evidence",
            has_evidence,
            None if has_evidence else "live_evidence is null or has no populated key",
        )
    if mode == "mock" and live_evidence is not None:
        _push(
            acc,
            "capsule.mock_mode_rejects_live_evidence",
            "execution.mode == 'mock' must have live_evidence == null/absent",
            False,
            "live_evidence present in mock-mode capsule",
        )

    sponsor_payload_hash = execution.get("sponsor_payload_hash")
    if isinstance(sponsor_payload_hash, str):
        _shape(
            acc,
            "capsule.sponsor_payload_hash_shape",
            "execution.sponsor_payload_hash is 64-char lowercase hex",
            bool(_HEX64.match(sponsor_payload_hash)),
            sponsor_payload_hash,
        )

    cp = audit.get("checkpoint")
    if isinstance(cp, dict):
        _push(
            acc,
            "capsule.checkpoint_mock_anchor_required",
            "audit.checkpoint.mock_anchor must be true",
            cp.get("mock_anchor") is True,
            None if cp.get("mock_anchor") is True else f"mock_anchor={cp.get('mock_anchor')!r}",
        )
        _shape(
            acc,
            "capsule.checkpoint_mock_anchor_ref_shape",
            "audit.checkpoint.mock_anchor_ref matches local-mock-anchor-<16hex>",
            bool(_MOCK_ANCHOR_REF.match(cp.get("mock_anchor_ref", ""))),
            cp.get("mock_anchor_ref", ""),
        )
        _shape(
            acc,
            "capsule.checkpoint_chain_digest_shape",
            "audit.checkpoint.chain_digest is 64-char lowercase hex",
            bool(_HEX64.match(cp.get("chain_digest", ""))),
            cp.get("chain_digest", ""),
        )
        _shape(
            acc,
            "capsule.checkpoint_latest_event_hash_shape",
            "audit.checkpoint.latest_event_hash is 64-char lowercase hex",
            bool(_HEX64.match(cp.get("latest_event_hash", ""))),
            cp.get("latest_event_hash", ""),
        )

    live_claims = verification.get("live_claims") or []
    if live_evidence is None and isinstance(live_claims, list) and len(live_claims) > 0:
        _push(
            acc,
            "capsule.live_claims_without_evidence",
            ("verification.live_claims must be empty when execution.live_evidence is null"),
            False,
            f"live_claims={live_claims!r}",
        )


def _run_v2_only_checks(c: dict[str, Any], acc: _Acc) -> None:
    policy = c.get("policy") or {}
    audit = c.get("audit") or {}

    snap = policy.get("policy_snapshot")
    if snap is not None:
        ok = isinstance(snap, dict)
        _push(
            acc,
            "capsule.policy_snapshot_shape",
            "policy.policy_snapshot, when present, must be a JSON object",
            ok,
            None if ok else f"type={type(snap).__name__}",
        )

    seg = audit.get("audit_segment")
    if seg is not None:
        ok = isinstance(seg, dict)
        _push(
            acc,
            "capsule.audit_segment_shape",
            "audit.audit_segment, when present, must be a JSON object",
            ok,
            None if ok else f"type={type(seg).__name__}",
        )


def _push(acc: _Acc, code: str, description: str, passed: bool, detail: str | None) -> None:
    acc.checks.append(VerifyCheck(code=code, description=description, passed=passed, detail=detail))


def _shape(acc: _Acc, code: str, description: str, passed: bool, offending: str) -> None:
    _push(acc, code, description, passed, None if passed else offending)


def _match(acc: _Acc, code: str, description: str, left: str, right: str) -> None:
    passed = left == right
    _push(acc, code, description, passed, None if passed else f"{left} != {right}")
