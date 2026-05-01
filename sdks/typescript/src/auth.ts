/**
 * Auth helpers for SBO3L. The wire shape (bearer token + JWT with
 * `sub == agent_id` claim) is finalised in F-1; until F-1 lands, these
 * helpers only assemble the `Authorization` header — they don't sign or
 * verify JWTs (that's the daemon's job).
 *
 * The SDK never reads a private key. JWT signing is the caller's
 * responsibility (or, in the no-key boundary model, a downstream signing
 * service). This file purely constructs `Authorization` header strings.
 */

/**
 * Configuration for SBO3L request authentication. Either supply a static
 * bearer token or a JWT-supplier callback (the supplier is invoked per
 * request so callers can rotate / refresh).
 *
 * Bearer mode is service-to-service (matches `SBO3L_BEARER_TOKEN_HASH` env
 * var on the server). JWT mode is per-agent (claim `sub` = `agent_id`).
 */
export type AuthConfig =
  | { kind: "bearer"; token: string }
  | { kind: "jwt"; token: string }
  | { kind: "jwt-supplier"; supplier: () => string | Promise<string> }
  | { kind: "none" };

/**
 * Compute the `Authorization` header value for a given auth config.
 * Returns `undefined` when `kind === "none"` so callers can spread the
 * result into a header object without producing an empty `Authorization`.
 */
export async function authHeader(auth: AuthConfig): Promise<string | undefined> {
  switch (auth.kind) {
    case "none":
      return undefined;
    case "bearer":
      return `Bearer ${auth.token}`;
    case "jwt":
      return `Bearer ${auth.token}`;
    case "jwt-supplier": {
      const t = await auth.supplier();
      return `Bearer ${t}`;
    }
  }
}

/**
 * Decode a JWT *without verifying its signature*. Returns the parsed claim
 * set. The SDK does not verify JWT signatures client-side — the daemon does
 * that against `SBO3L_JWT_PUBKEY_HEX`. Use this helper only to inspect a JWT
 * before sending (e.g. to confirm `sub` matches your `agent_id`).
 *
 * Throws if the token isn't a well-formed three-segment JWT.
 */
export function decodeJwtClaims(jwt: string): Record<string, unknown> {
  const parts = jwt.split(".");
  if (parts.length !== 3) {
    throw new Error("invalid JWT: expected three dot-separated segments");
  }
  const payload = parts[1];
  if (typeof payload !== "string" || payload.length === 0) {
    throw new Error("invalid JWT: empty payload segment");
  }
  const json = base64UrlDecodeToString(payload);
  const claims = JSON.parse(json) as unknown;
  if (typeof claims !== "object" || claims === null) {
    throw new Error("invalid JWT: payload is not a JSON object");
  }
  return claims as Record<string, unknown>;
}

/**
 * Confirm a JWT's `sub` claim equals the expected `agent_id`. This mirrors
 * the F-1 daemon-side check; running it client-side surfaces a misconfigured
 * token before the round-trip.
 *
 * @throws Error if `sub` is missing or does not equal `expectedAgentId`.
 */
export function assertJwtSubMatches(jwt: string, expectedAgentId: string): void {
  const claims = decodeJwtClaims(jwt);
  const sub = claims["sub"];
  if (typeof sub !== "string") {
    throw new Error("invalid JWT: missing or non-string 'sub' claim");
  }
  if (sub !== expectedAgentId) {
    throw new Error(
      `JWT 'sub' claim '${sub}' does not match expected agent_id '${expectedAgentId}'`,
    );
  }
}

function base64UrlDecodeToString(input: string): string {
  // Pad and convert base64url → base64
  const padded = input + "=".repeat((4 - (input.length % 4)) % 4);
  const b64 = padded.replace(/-/g, "+").replace(/_/g, "/");
  // Buffer is available in Node >= 18; modern browsers expose `atob`.
  if (typeof Buffer !== "undefined") {
    return Buffer.from(b64, "base64").toString("utf-8");
  }
  // Browser fallback. atob returns a binary string; decode via TextDecoder.
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return new TextDecoder("utf-8").decode(bytes);
}
