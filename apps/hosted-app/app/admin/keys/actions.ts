"use server";

import { auth } from "@/auth";
import { meetsRole } from "@/lib/roles";

const DAEMON_URL = process.env.SBO3L_DAEMON_URL ?? "http://localhost:8080";
const FETCH_TIMEOUT_MS = 4000;

export interface KmsStatus {
  backend: "in-memory" | "file" | "kms-aws" | "kms-gcp" | "vault" | "unknown";
  configured: boolean;
  pubkey_b58?: string | null;
  key_id?: string | null;
  last_signed_at?: string | null;
  last_signed_by_request_hash?: string | null;
  region?: string | null;
  health?: {
    ok: boolean;
    rtt_ms?: number;
    checked_at?: string;
    detail?: string;
  };
}

export interface KmsStatusResult {
  ok: boolean;
  status?: KmsStatus;
  pending?: boolean;   // daemon endpoint not yet implemented
  error?: string;
}

async function requireAdmin(): Promise<{ ok: true } | { ok: false; error: string }> {
  const session = await auth();
  if (!meetsRole(session?.user?.role, "admin")) {
    return { ok: false, error: "admin role required" };
  }
  return { ok: true };
}

export async function fetchKmsStatus(): Promise<KmsStatusResult> {
  const gate = await requireAdmin();
  if (!gate.ok) return { ok: false, error: gate.error };

  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
  try {
    const res = await fetch(`${DAEMON_URL}/v1/admin/kms/status`, {
      signal: ctrl.signal,
      cache: "no-store",
    });
    if (res.status === 404) {
      return {
        ok: false,
        pending: true,
        error: "Daemon /v1/admin/kms/status endpoint not yet implemented (tracked as Dev 1 follow-up to #213). UI will activate automatically once the endpoint ships.",
      };
    }
    if (!res.ok) {
      return { ok: false, error: `daemon ${res.status} ${res.statusText}` };
    }
    const status = (await res.json()) as KmsStatus;
    return { ok: true, status };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : "fetch failed" };
  } finally {
    clearTimeout(timer);
  }
}

// "Health check" sends a no-op sign request to confirm the configured
// signer is reachable + producing valid signatures. Daemon route TBD;
// when it lands, it should return { ok, rtt_ms, detail }.
export async function runKmsHealthCheck(): Promise<KmsStatusResult> {
  const gate = await requireAdmin();
  if (!gate.ok) return { ok: false, error: gate.error };

  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS * 2);
  try {
    const res = await fetch(`${DAEMON_URL}/v1/admin/kms/health`, {
      method: "POST",
      signal: ctrl.signal,
      cache: "no-store",
    });
    if (res.status === 404) {
      return {
        ok: false,
        pending: true,
        error: "Daemon /v1/admin/kms/health endpoint not yet implemented.",
      };
    }
    if (!res.ok) {
      return { ok: false, error: `daemon ${res.status} ${res.statusText}` };
    }
    const status = (await res.json()) as KmsStatus;
    return { ok: true, status };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : "fetch failed" };
  } finally {
    clearTimeout(timer);
  }
}
