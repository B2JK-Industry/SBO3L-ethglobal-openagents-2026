import Link from "next/link";
import { fetchFlags } from "./actions";
import { FlagsTable } from "./FlagsTable";

export const dynamic = "force-dynamic";

export default async function AdminFlagsPage() {
  const initial = await fetchFlags();

  return (
    <main>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: "1em" }}>
        <h1>Feature flags</h1>
        <Link href="/admin/users" style={{ fontSize: "0.85em" }}>
          ← Users
        </Link>
      </header>
      <p style={{ color: "var(--muted)", marginBottom: "1.5em", maxWidth: 760 }}>
        Hot-reloadable runtime flags backed by Dev 1's <code>feature_flags</code> module
        (<a href="https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/pull/213">#213</a>).
        Toggling a flag here is durable, takes effect on the next request the daemon
        processes, and writes a <code>flag.changed</code> audit event automatically. The
        table refreshes every 10 seconds so multi-admin changes propagate.
      </p>

      {!initial.ok && (
        <aside
          role="alert"
          style={{
            background: "var(--code-bg)",
            border: "1px solid var(--border)",
            borderLeft: "3px solid #ff6b6b",
            borderRadius: "var(--r-md)",
            padding: "1em 1.2em",
            color: "var(--muted)",
            marginBottom: "1.5em",
          }}
        >
          <strong style={{ color: "#ff6b6b" }}>Daemon unreachable.</strong>{" "}
          Failed to load flags from <code>{process.env.SBO3L_DAEMON_URL ?? "http://localhost:8080"}/v1/admin/flags</code>: {initial.error}.{" "}
          The page below shows an empty list; toggle attempts will surface the same error inline.
        </aside>
      )}

      <FlagsTable initial={initial.data ?? { flags: [], fetched_at: new Date().toISOString() }} />
    </main>
  );
}
