import type { ProblemDetail } from "./types.js";

/**
 * Thrown when the SBO3L daemon returns a non-2xx HTTP response. Carries the
 * RFC 7807 problem-detail body verbatim so callers can branch on `.code`.
 */
export class SBO3LError extends Error {
  override readonly name = "SBO3LError";

  /** HTTP status code returned by the daemon. */
  readonly status: number;

  /** Domain code, e.g. `auth.required`, `policy.budget_exceeded`. */
  readonly code: string;

  /** Full RFC 7807 problem-detail body. */
  readonly problem: ProblemDetail;

  constructor(problem: ProblemDetail) {
    super(`${problem.code}: ${problem.title} — ${problem.detail}`);
    this.status = problem.status;
    this.code = problem.code;
    this.problem = problem;
  }
}

/**
 * Thrown when network or transport errors prevent reaching the daemon.
 * Distinct from `SBO3LError` (which represents a server-side rejection).
 */
export class SBO3LTransportError extends Error {
  override readonly name = "SBO3LTransportError";

  override readonly cause?: unknown;

  constructor(message: string, cause?: unknown) {
    super(message);
    this.cause = cause;
  }
}

/**
 * Thrown by the client-side passport verifier when a capsule fails one or
 * more structural checks. Carries the list of failure codes.
 */
export class PassportVerificationError extends Error {
  override readonly name = "PassportVerificationError";

  /** Domain codes for failed checks, e.g. `["capsule.schema_unknown"]`. */
  readonly codes: readonly string[];

  constructor(codes: readonly string[], detail?: string) {
    super(
      detail
        ? `passport verification failed: ${detail} [${codes.join(", ")}]`
        : `passport verification failed: ${codes.join(", ")}`,
    );
    this.codes = codes;
  }
}

/**
 * Type guard: `e` is a `ProblemDetail` (RFC 7807-shaped object).
 */
export function isProblemDetail(e: unknown): e is ProblemDetail {
  if (typeof e !== "object" || e === null) return false;
  const o = e as Record<string, unknown>;
  return (
    typeof o["type"] === "string" &&
    typeof o["title"] === "string" &&
    typeof o["status"] === "number" &&
    typeof o["detail"] === "string" &&
    typeof o["code"] === "string"
  );
}
