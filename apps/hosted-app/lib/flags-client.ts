// Fetch wrapper around the daemon's /v1/admin/flags surface.
//
// Endpoints (Dev 1's #213):
//   GET  /v1/admin/flags             — list all flags
//   POST /v1/admin/flags/<name>      — { enabled: boolean } toggle
//   Audit chain records `flag.changed` server-side automatically.

const DAEMON_URL = process.env.SBO3L_DAEMON_URL ?? "http://localhost:8080";
const FETCH_TIMEOUT_MS = 4000;

export interface FeatureFlag {
  name: string;
  enabled: boolean;
  description: string;
  default_value: boolean;
  last_changed_at: string | null;
  last_changed_by: string | null;
  category?: string;
}

export interface FlagsResponse {
  flags: FeatureFlag[];
  fetched_at: string;
}

export interface ToggleResult {
  ok: boolean;
  flag?: FeatureFlag;
  error?: string;
}

async function fetchDaemon<T>(path: string, init?: RequestInit, authToken?: string): Promise<T> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
  try {
    const res = await fetch(`${DAEMON_URL}${path}`, {
      ...init,
      signal: ctrl.signal,
      headers: {
        "Content-Type": "application/json",
        ...(authToken ? { Authorization: `Bearer ${authToken}` } : {}),
        ...(init?.headers ?? {}),
      },
      cache: "no-store",
    });
    if (!res.ok) {
      const body = await res.text().catch(() => "");
      throw new Error(`${res.status} ${res.statusText}${body ? ` — ${body.slice(0, 200)}` : ""}`);
    }
    return (await res.json()) as T;
  } finally {
    clearTimeout(timer);
  }
}

export async function listFlags(authToken?: string): Promise<FlagsResponse> {
  return fetchDaemon<FlagsResponse>("/v1/admin/flags", { method: "GET" }, authToken);
}

export async function toggleFlag(name: string, enabled: boolean, authToken?: string): Promise<FeatureFlag> {
  return fetchDaemon<FeatureFlag>(
    `/v1/admin/flags/${encodeURIComponent(name)}`,
    { method: "POST", body: JSON.stringify({ enabled }) },
    authToken,
  );
}
