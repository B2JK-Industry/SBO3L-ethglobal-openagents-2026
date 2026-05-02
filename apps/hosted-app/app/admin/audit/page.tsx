import Link from "next/link";
import { eventsWebSocketUrl } from "@/lib/sbo3l-client";
import { AuditTimeline } from "./AuditTimeline";
import { DecisionChart } from "./DecisionChart";

export const dynamic = "force-dynamic";

export default function AdminAuditPage() {
  const wsUrl = eventsWebSocketUrl();

  return (
    <main>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: "1em" }}>
        <h1>Audit timeline</h1>
        <nav style={{ fontSize: "0.85em", display: "flex", gap: "1em" }}>
          <Link href="/admin/users">Users</Link>
          <Link href="/admin/flags">Flags</Link>
          <Link href="/admin/keys">Keys</Link>
        </nav>
      </header>
      <p style={{ color: "var(--muted)", marginBottom: "1.5em", maxWidth: 760 }}>
        Live tail of the daemon's <code>/v1/admin/events</code> WebSocket bus.
        Each <code>kind: "decision"</code> frame carries the agent's
        decision, deny code (if any), severity, audit-event hash, and
        chain seq; <code>kind: "operational"</code> covers signer rotations
        + flag changes. Filter inline; export the buffered window as
        JSONL or CSV for post-hoc analysis. Buffered window holds the
        last 500 events; older events scroll off (full chain remains in
        the daemon's audit DB).
      </p>

      <DecisionChart wsUrl={wsUrl} />
      <AuditTimeline wsUrl={wsUrl} />

      <aside style={{ marginTop: "2em", padding: "1em 1.2em", background: "var(--code-bg)", border: "1px solid var(--border)", borderLeft: "3px solid var(--accent)", borderRadius: "var(--r-md)", color: "var(--muted)", fontSize: "0.9em" }}>
        <strong style={{ color: "var(--fg)" }}>Tenant scope:</strong> per-tenant
        filtering happens at the daemon (Grace's slice). Until that ships,
        admins see every tenant's events; non-admins are blocked by the role
        gate on this route. The agent-id filter above is client-side and
        applies to the buffered window only.
      </aside>
    </main>
  );
}
