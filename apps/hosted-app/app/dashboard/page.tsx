import Link from "next/link";
import { auth, signOut } from "@/auth";
import { DaemonStatusBanner } from "@/components/DaemonStatusBanner";
import { listAudit, eventsWebSocketUrl, DaemonError } from "@/lib/sbo3l-client";
import { mockAuditEvents, mockCapsules } from "@/lib/mock-data";
import { RecentDecisionsLive } from "./RecentDecisionsLive";

interface Stats {
  decisionsTotal: number;
  allowCount: number;
  denyCount: number;
  chainLength: number;
}

export default async function DashboardPage() {
  const session = await auth();
  const handle = session?.user?.githubLogin ?? session?.user?.name ?? "developer";

  let stats: Stats;
  try {
    const page = await listAudit({ limit: 200 });
    const decisions = page.events.filter((e) => e.event_type === "policy.decision");
    stats = {
      decisionsTotal: decisions.length,
      allowCount: decisions.filter((e) => e.decision === "allow").length,
      denyCount: decisions.filter((e) => e.decision === "deny").length,
      chainLength: page.chain_length,
    };
  } catch (err) {
    if (!(err instanceof DaemonError)) throw err;
    stats = {
      decisionsTotal: mockAuditEvents.filter((e) => e.eventType === "policy.decision").length,
      allowCount: mockAuditEvents.filter((e) => e.decision === "allow").length,
      denyCount: mockAuditEvents.filter((e) => e.decision === "deny").length,
      chainLength: mockAuditEvents.length,
    };
  }

  return (
    <main>
      <DaemonStatusBanner />
      <header style={{ display: "flex", justifyContent: "space-between", marginBottom: "2em" }}>
        <h1>Dashboard</h1>
        <form action={async () => { "use server"; await signOut({ redirectTo: "/" }); }}>
          <button type="submit" className="ghost">Sign out</button>
        </form>
      </header>
      <p style={{ color: "var(--muted)", marginBottom: "1.5em" }}>Hi, @{handle}.</p>

      <nav style={{ display: "flex", gap: "1em", marginBottom: "2em", flexWrap: "wrap" }}>
        <Link href="/agents">Agents</Link>
        <Link href="/audit">Audit log</Link>
        <Link href="/capsules">Capsule library</Link>
        <Link href="/trust-dns">Trust DNS</Link>
      </nav>

      <section style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: "1em" }}>
        <Card title="Decisions today" value={`${stats.decisionsTotal}`} hint={`allow ${stats.allowCount} · deny ${stats.denyCount}`} />
        <Card title="Audit chain" value={`${stats.chainLength}`} hint="length · strict verifiable" />
        <Card title="Capsules emitted" value={`${mockCapsules.length}`} hint="library at /capsules" />
      </section>

      <RecentDecisionsLive wsUrl={eventsWebSocketUrl()} />
    </main>
  );
}

function Card({ title, value, hint }: { title: string; value: string; hint: string }) {
  return (
    <article
      style={{
        border: "1px solid var(--border)",
        borderRadius: "var(--r-lg)",
        padding: "1.2em",
        background: "var(--code-bg)",
      }}
    >
      <h2 style={{ fontSize: "0.9em", color: "var(--muted)", fontWeight: 500 }}>{title}</h2>
      <p style={{ fontSize: "1.6em", fontWeight: 700, color: "var(--accent)", margin: "0.3em 0" }}>{value}</p>
      <p style={{ fontSize: "0.85em", color: "var(--muted)" }}>{hint}</p>
    </article>
  );
}
