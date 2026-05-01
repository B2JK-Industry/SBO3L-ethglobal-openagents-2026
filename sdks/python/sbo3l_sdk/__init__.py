"""sbo3l-sdk — official Python SDK for SBO3L.

SBO3L is the cryptographically verifiable trust layer for autonomous AI
agents. Every action passes through SBO3L's policy boundary first; output is
a self-contained Passport capsule anyone can verify offline.
"""

from __future__ import annotations

from ._version import __version__
from .auth import (
    AuthConfig,
    assert_jwt_sub_matches,
    auth_header,
    bearer,
    decode_jwt_claims,
    jwt,
    jwt_supplier,
    none,
)
from .client import SBO3LClient
from .errors import PassportVerificationError, SBO3LError, SBO3LTransportError
from .passport import VerifyCheck, VerifyResult, verify, verify_or_raise
from .sync import SBO3LClientSync
from .types import (
    AgentId,
    AprpDestination,
    AprpDestinationEoa,
    AprpDestinationErc20Transfer,
    AprpDestinationSmartAccount,
    AprpDestinationX402Endpoint,
    AprpExpectedResult,
    AprpIntent,
    AprpMoney,
    AprpPaymentProtocol,
    AprpRiskClass,
    CapsuleAgent,
    CapsuleAuditCheckpoint,
    CapsuleAuditSegment,
    CapsuleAuditV1,
    CapsuleAuditV2,
    CapsuleDecision,
    CapsuleExecution,
    CapsuleExecutionMode,
    CapsuleExecutionStatus,
    CapsuleExecutor,
    CapsuleLiveEvidence,
    CapsulePolicySnapshot,
    CapsulePolicyV1,
    CapsulePolicyV2,
    CapsuleRequest,
    CapsuleResolver,
    CapsuleVerification,
    Decision,
    DoctorStatus,
    PassportCapsule,
    PassportCapsuleV1,
    PassportCapsuleV2,
    PaymentRequest,
    PaymentRequestResponse,
    PaymentStatus,
    PolicyReceipt,
    ProblemDetail,
    ReceiptSignature,
    Ulid,
)

__all__ = [
    # version
    "__version__",
    # clients
    "SBO3LClient",
    "SBO3LClientSync",
    # errors
    "SBO3LError",
    "SBO3LTransportError",
    "PassportVerificationError",
    # passport
    "verify",
    "verify_or_raise",
    "VerifyResult",
    "VerifyCheck",
    # auth
    "AuthConfig",
    "auth_header",
    "bearer",
    "jwt",
    "jwt_supplier",
    "none",
    "decode_jwt_claims",
    "assert_jwt_sub_matches",
    # types — APRP
    "AgentId",
    "Ulid",
    "AprpIntent",
    "AprpRiskClass",
    "AprpPaymentProtocol",
    "AprpMoney",
    "AprpDestination",
    "AprpDestinationX402Endpoint",
    "AprpDestinationEoa",
    "AprpDestinationSmartAccount",
    "AprpDestinationErc20Transfer",
    "AprpExpectedResult",
    "PaymentRequest",
    # types — receipt
    "Decision",
    "ReceiptSignature",
    "PolicyReceipt",
    # types — server response
    "PaymentStatus",
    "PaymentRequestResponse",
    "ProblemDetail",
    # types — capsule
    "CapsuleResolver",
    "CapsuleAgent",
    "CapsuleRequest",
    "CapsulePolicyV1",
    "CapsulePolicyV2",
    "CapsulePolicySnapshot",
    "CapsuleDecision",
    "CapsuleExecutor",
    "CapsuleExecutionMode",
    "CapsuleExecutionStatus",
    "CapsuleLiveEvidence",
    "CapsuleExecution",
    "CapsuleAuditCheckpoint",
    "CapsuleAuditV1",
    "CapsuleAuditV2",
    "CapsuleAuditSegment",
    "DoctorStatus",
    "CapsuleVerification",
    "PassportCapsuleV1",
    "PassportCapsuleV2",
    "PassportCapsule",
]
