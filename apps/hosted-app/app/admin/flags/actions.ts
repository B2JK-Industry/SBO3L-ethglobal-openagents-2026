"use server";

import { auth } from "@/auth";
import { meetsRole } from "@/lib/roles";
import { listFlags, toggleFlag, type FeatureFlag, type FlagsResponse } from "@/lib/flags-client";

export interface ListFlagsResult {
  ok: boolean;
  data?: FlagsResponse;
  error?: string;
}

export interface ToggleFlagResult {
  ok: boolean;
  flag?: FeatureFlag;
  error?: string;
}

async function requireAdmin(): Promise<{ ok: true } | { ok: false; error: string }> {
  const session = await auth();
  if (!meetsRole(session?.user?.role, "admin")) {
    return { ok: false, error: "admin role required" };
  }
  return { ok: true };
}

export async function fetchFlags(): Promise<ListFlagsResult> {
  const gate = await requireAdmin();
  if (!gate.ok) return gate;
  try {
    const data = await listFlags();
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : "fetch failed" };
  }
}

// Toggle one flag by name. Daemon's /v1/admin/flags/<name> writes a
// flag.changed audit event server-side; we surface the new flag state
// so the client can do an optimistic-update-with-rollback pattern.
export async function setFlagEnabled(name: string, enabled: boolean): Promise<ToggleFlagResult> {
  const gate = await requireAdmin();
  if (!gate.ok) return gate;
  try {
    const flag = await toggleFlag(name, enabled);
    return { ok: true, flag };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : "toggle failed" };
  }
}
