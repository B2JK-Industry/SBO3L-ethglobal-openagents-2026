import { auth, signOut } from "@/auth";

export default async function DashboardPage() {
  const session = await auth();
  // middleware.ts guarantees session exists for /dashboard/*; the
  // optional-chain below is for type-narrowing only.
  const handle = session?.user?.githubLogin ?? session?.user?.name ?? "developer";

  return (
    <main>
      <header style={{ display: "flex", justifyContent: "space-between", marginBottom: "2em" }}>
        <h1>Dashboard</h1>
        <form
          action={async () => {
            "use server";
            await signOut({ redirectTo: "/" });
          }}
        >
          <button type="submit" className="ghost">Sign out</button>
        </form>
      </header>
      <p style={{ color: "var(--muted)", marginBottom: "2em" }}>Hi, @{handle}.</p>

      <section style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: "1em" }}>
        <Card title="Decisions today" value="—" hint="awaits daemon link" />
        <Card title="Audit chain" value="—" hint="length + integrity" />
        <Card title="Quota used" value="0%" hint="of 1k/day free tier" />
      </section>

      <p style={{ color: "var(--muted)", marginTop: "2em", fontSize: "0.9em" }}>
        Live SSE feed + recent-decisions table + capsule downloads land in CTI-3-4 main.
      </p>
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
