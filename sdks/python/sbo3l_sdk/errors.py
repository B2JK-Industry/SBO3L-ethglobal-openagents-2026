"""Error hierarchy for the Python SDK.

Mirrors the TypeScript SDK shape:
  - `SBO3LError` — daemon returned a non-2xx; carries the RFC 7807 problem.
  - `SBO3LTransportError` — network/transport failure (separate from server-
     side rejection).
  - `PassportVerificationError` — client-side structural verifier failure.
"""

from __future__ import annotations

from .types import ProblemDetail


class SBO3LError(Exception):
    """Raised when the SBO3L daemon returns a non-2xx response.

    Carries the RFC 7807 problem-detail body verbatim so callers can branch
    on `code` (e.g. `auth.required`, `policy.budget_exceeded`).
    """

    __slots__ = ("code", "problem", "status")

    status: int
    code: str
    problem: ProblemDetail

    def __init__(self, problem: ProblemDetail) -> None:
        super().__init__(f"{problem.code}: {problem.title} — {problem.detail}")
        self.status = problem.status
        self.code = problem.code
        self.problem = problem


class SBO3LTransportError(Exception):
    """Raised on network / transport failures (timeout, DNS, refused).

    Distinct from `SBO3LError` (which represents a server-side rejection).
    """

    __slots__ = ()


class PassportVerificationError(Exception):
    """Raised by `verify_or_raise` when a capsule fails one or more structural checks.

    Carries `codes` (tuple of failure-code strings) for programmatic handling.
    """

    __slots__ = ("codes",)

    codes: tuple[str, ...]

    def __init__(self, codes: tuple[str, ...], detail: str | None = None) -> None:
        codes_join = ", ".join(codes)
        msg = (
            f"passport verification failed: {detail} [{codes_join}]"
            if detail
            else f"passport verification failed: {codes_join}"
        )
        super().__init__(msg)
        self.codes = codes
