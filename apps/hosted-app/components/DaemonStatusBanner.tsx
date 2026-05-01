// Server component — renders a one-line banner when the daemon is
// unreachable, signalling that the page below shows mock fixtures.
// Async fetch happens at request time; no client JS.

import { isDaemonAlive } from "@/lib/sbo3l-client";

export async function DaemonStatusBanner(): Promise<JSX.Element | null> {
  const alive = await isDaemonAlive();
  if (alive) return null;
  return (
    <aside
      style={{
        background: "var(--code-bg)",
        border: "1px solid var(--border)",
        borderLeft: "3px solid #ff6b6b",
        borderRadius: "var(--r-md)",
        padding: "0.7em 1em",
        marginBottom: "1.5em",
        color: "var(--muted)",
        fontSize: "0.9em",
      }}
      role="status"
    >
      <strong style={{ color: "#ff6b6b" }}>Demo mode</strong> — SBO3L daemon at{" "}
      <code>{process.env.SBO3L_DAEMON_URL ?? "http://localhost:8080"}</code> unreachable. Page shows mock fixtures. Boot the daemon (
      <code>cargo run --release -p sbo3l-server</code>) or set <code>SBO3L_DAEMON_URL</code> to use real data.
    </aside>
  );
}
