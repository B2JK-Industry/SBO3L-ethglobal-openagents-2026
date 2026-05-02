// Daemon URL helpers. Server-only constants for SSR routes; the WS
// helper returns the public ws://… URL so the browser can connect
// directly to /v1/events (no Next.js proxy in between).
//
// The daemon is configured via SBO3L_DAEMON_URL (production) or
// SBO3L_PUBLIC_DAEMON_URL (separate hostname for the WS bus when
// the HTTP API is behind a reverse proxy).

export const DAEMON_URL = process.env.SBO3L_DAEMON_URL ?? "http://localhost:8080";

/**
 * Convert the configured daemon URL to its WebSocket counterpart.
 * Browser code calls this server-side at SSR time so the resolved
 * URL bakes into the page payload.
 */
export function eventsWebSocketUrl(): string {
  const explicit = process.env.SBO3L_PUBLIC_DAEMON_URL ?? DAEMON_URL;
  if (explicit.startsWith("https://")) return `wss://${explicit.slice("https://".length)}/v1/admin/events`;
  if (explicit.startsWith("http://"))  return `ws://${explicit.slice("http://".length)}/v1/admin/events`;
  // Already ws:// or wss:// — pass through but ensure the path.
  if (explicit.endsWith("/v1/admin/events")) return explicit;
  return `${explicit.replace(/\/$/, "")}/v1/admin/events`;
}
