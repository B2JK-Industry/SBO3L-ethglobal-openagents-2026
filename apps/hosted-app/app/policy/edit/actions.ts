"use server";

import { auth } from "@/auth";
import { meetsRole } from "@/lib/roles";

export interface DryRunResult {
  ok: boolean;
  decision?: "allow" | "deny";
  deny_code?: string;
  policy_snapshot_hash?: string;
  error?: string;
}

export interface SaveResult {
  ok: boolean;
  policy_hash?: string;
  error?: string;
}

const DAEMON_URL = process.env.SBO3L_DAEMON_URL ?? "http://localhost:8080";

// Dry-run a candidate APRP envelope against the in-editor policy. The
// daemon's POST /v1/policy/check (added in #178) reads the request
// body's `policy_override` field as a candidate; existing fixed
// policy.json is unchanged. Returns the projected decision.
export async function dryRunPolicy(policyJson: string, sampleAprp: string): Promise<DryRunResult> {
  const session = await auth();
  if (!meetsRole(session?.user?.role, "operator")) {
    return { ok: false, error: "operator role required" };
  }

  let policy: unknown;
  let aprp: unknown;
  try {
    policy = JSON.parse(policyJson);
  } catch (e) {
    return { ok: false, error: `policy JSON parse: ${e instanceof Error ? e.message : "invalid"}` };
  }
  try {
    aprp = JSON.parse(sampleAprp);
  } catch (e) {
    return { ok: false, error: `APRP JSON parse: ${e instanceof Error ? e.message : "invalid"}` };
  }

  try {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), 4000);
    const res = await fetch(`${DAEMON_URL}/v1/policy/check`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ ...(aprp as Record<string, unknown>), _policy_override: policy }),
      signal: ctrl.signal,
      cache: "no-store",
    });
    clearTimeout(timer);
    if (!res.ok) {
      return { ok: false, error: `daemon ${res.status} ${res.statusText}` };
    }
    const body = (await res.json()) as { decision: "allow" | "deny"; deny_code?: string; policy_snapshot_hash: string };
    return {
      ok: true,
      decision: body.decision,
      deny_code: body.deny_code,
      policy_snapshot_hash: body.policy_snapshot_hash,
    };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : "fetch failed" };
  }
}

// Save the policy. Today this is a daemon round-trip to a
// (forthcoming) POST /v1/policy/upload endpoint that:
//   1. validates the body against sbo3l.policy.v1 schema
//   2. computes the policy_snapshot_hash
//   3. swaps the active policy atomically
//   4. appends a `policy.changed` audit event
// Until that endpoint ships, this returns ok:false with a clear
// pending-feature error so operators see the editor isn't a no-op.
export async function savePolicy(policyJson: string): Promise<SaveResult> {
  const session = await auth();
  if (!meetsRole(session?.user?.role, "admin")) {
    return { ok: false, error: "admin role required" };
  }

  let policy: unknown;
  try {
    policy = JSON.parse(policyJson);
  } catch (e) {
    return { ok: false, error: `policy JSON parse: ${e instanceof Error ? e.message : "invalid"}` };
  }

  try {
    const res = await fetch(`${DAEMON_URL}/v1/policy/upload`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(policy),
      cache: "no-store",
    });
    if (res.status === 404) {
      return {
        ok: false,
        error: "Daemon /v1/policy/upload endpoint not yet implemented; tracked as Dev 1 follow-up. Editor + dry-run path is live; save will activate once endpoint ships.",
      };
    }
    if (!res.ok) {
      return { ok: false, error: `daemon ${res.status} ${res.statusText}` };
    }
    const body = (await res.json()) as { policy_hash: string };
    return { ok: true, policy_hash: body.policy_hash };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : "fetch failed" };
  }
}
