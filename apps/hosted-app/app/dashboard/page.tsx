import Link from "next/link";
import { auth, signOut } from "@/auth";
import { mockAuditEvents, mockCapsules } from "@/lib/mock-data";
import { RecentDecisionsLive } from "./RecentDecisionsLive";

export default async function DashboardPage() {
  const session = await auth();
  const handle = session?.user?.githubLogin ?? session?.user?.name ?? "developer";

  const decisionsTotal = mockAuditEvents.filter((e) => e.eventType === "policy.decision").length;
  const allowCount = mockAuditEvents.filter((e) => e.decision === "allow").length;
  const denyCount = mockAuditEvents.filter((e) => e.decision === "deny").length;

  return (
    <main>
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
        <a href="https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/trust-dns-story" target="_blank" rel="noreferrer">Trust DNS</a>
      </nav>

      <section style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: "1em" }}>
        <Card title="Decisions today" value={`${decisionsTotal}`} hint={`allow ${allowCount} · deny ${denyCount}`} />
        <Card title="Audit chain" value={`${mockAuditEvents.length}`} hint="length · ✓ verified" />
        <Card title="Capsules emitted" value={`${mockCapsules.length}`} hint="downloadable + offline-verifiable" />
      </section>

      <RecentDecisionsLive />
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
