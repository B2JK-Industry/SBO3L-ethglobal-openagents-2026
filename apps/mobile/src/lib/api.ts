// Thin client over the SBO3L hosted-app + daemon endpoints. Mirrors
// the shape of apps/hosted-app/lib/sbo3l-client.ts so swapping the
// backend URL is a config-only change.
//
// Auth: Bearer token retrieved from expo-secure-store (set during
// the OAuth flow in src/lib/auth.ts). 401 responses bubble up so
// the UI can re-prompt sign-in.

import Constants from "expo-constants";
import * as SecureStore from "expo-secure-store";

const TOKEN_KEY = "sbo3l.session";

function baseUrl(): string {
  const explicit = process.env.EXPO_PUBLIC_API_BASE_URL;
  if (explicit) return explicit;
  return (Constants.expoConfig?.extra?.apiBaseUrl as string | undefined) ?? "https://sbo3l-app.vercel.app";
}

export async function setToken(token: string): Promise<void> {
  await SecureStore.setItemAsync(TOKEN_KEY, token, { keychainAccessible: SecureStore.AFTER_FIRST_UNLOCK });
}

export async function getToken(): Promise<string | null> {
  return SecureStore.getItemAsync(TOKEN_KEY);
}

export async function clearToken(): Promise<void> {
  await SecureStore.deleteItemAsync(TOKEN_KEY);
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const token = await getToken();
  const headers: Record<string, string> = { "content-type": "application/json", ...((init?.headers ?? {}) as Record<string, string>) };
  if (token) headers["authorization"] = `Bearer ${token}`;
  const res = await fetch(`${baseUrl()}${path}`, { ...init, headers });
  if (!res.ok) {
    throw new ApiError(res.status, await res.text().catch(() => res.statusText));
  }
  return (await res.json()) as T;
}

export class ApiError extends Error {
  constructor(public status: number, public body: string) {
    super(`SBO3L API error ${status}: ${body}`);
  }
}

export interface Tenant {
  slug: string;
  display_name: string;
  tier: "free" | "pro" | "enterprise";
}

export interface AuditEvent {
  event_id: string;
  ts_ms: number;
  kind: string;
  agent_id?: string;
  decision?: "allow" | "deny";
  deny_code?: string;
}

export interface PendingApproval {
  decision_id: string;
  agent_id: string;
  intent: string;
  amount_usd_cents?: number;
  expires_at: string;
}

export const api = {
  me: () => request<{ memberships: Array<{ tenant: Tenant; role: string }> }>("/api/me"),
  audit: (slug: string, limit = 50) => request<AuditEvent[]>(`/api/t/${slug}/audit?limit=${limit}`),
  approvals: (slug: string) => request<PendingApproval[]>(`/api/t/${slug}/approvals`),
  resolveApproval: (slug: string, id: string, decision: "allow" | "deny") =>
    request<{ ok: true }>(`/api/t/${slug}/approvals/${id}`, {
      method: "POST",
      body: JSON.stringify({ decision }),
    }),
  registerPushToken: (slug: string, token: string) =>
    request<{ ok: true }>(`/api/t/${slug}/push-tokens`, {
      method: "POST",
      body: JSON.stringify({ token }),
    }),
};
