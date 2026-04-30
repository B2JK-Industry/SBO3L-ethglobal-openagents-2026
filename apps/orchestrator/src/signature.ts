import { createHmac, timingSafeEqual } from "node:crypto";

/**
 * Verifies a Linear webhook signature against the raw request body.
 *
 * Linear computes `HMAC-SHA256(secret, rawBody)` and sends the hex digest in
 * the `linear-signature` header. We MUST verify on the raw body bytes — any
 * pre-parsed JSON re-stringification will mismatch.
 *
 * Returns false (not throws) on length / format mismatch so the caller can
 * respond with 401 + structured log entry rather than a stack trace.
 */
export function verifyLinearSignature(
  rawBody: string | Buffer,
  signatureHeader: string | undefined,
  secret: string,
): boolean {
  if (!signatureHeader || !secret) return false;

  const expected = createHmac("sha256", secret)
    .update(rawBody)
    .digest("hex");

  // Reject up-front if lengths differ — timingSafeEqual throws otherwise.
  if (expected.length !== signatureHeader.length) return false;

  try {
    return timingSafeEqual(
      Buffer.from(expected, "utf8"),
      Buffer.from(signatureHeader, "utf8"),
    );
  } catch {
    return false;
  }
}
