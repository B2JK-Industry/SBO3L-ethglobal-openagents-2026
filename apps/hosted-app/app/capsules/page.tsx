import { mockCapsules } from "@/lib/mock-data";

export default function CapsulesPage() {
  const capsules = mockCapsules;

  return (
    <main>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1.5em" }}>
        <h1>Capsule library</h1>
        <a
          href="https://sbo3l-marketing.vercel.app/proof"
          style={{ color: "var(--accent)" }}
        >
          Verify in browser →
        </a>
      </header>
      <p style={{ color: "var(--muted)", marginBottom: "2em" }}>
        Self-contained Passport capsules. Each is offline-verifiable against the agent's published Ed25519 pubkey alone
        (see <a href="https://sbo3l-docs.vercel.app/concepts/capsule">/concepts/capsule</a>).
      </p>
      <ul style={{ listStyle: "none", padding: 0, display: "grid", gap: "0.8em" }}>
        {capsules.map((c) => (
          <li
            key={c.capsuleId}
            style={{
              display: "grid",
              gridTemplateColumns: "1fr auto auto auto",
              gap: "1em",
              alignItems: "center",
              padding: "1em",
              border: "1px solid var(--border)",
              borderRadius: "var(--r-md)",
              background: "var(--code-bg)",
            }}
          >
            <div>
              <code style={{ fontSize: "0.85em" }}>{c.capsuleId}</code>
              <div style={{ color: "var(--muted)", fontSize: "0.85em", marginTop: "0.3em" }}>
                <code>{c.agentId}</code> · {new Date(c.emittedAt).toLocaleString()}
              </div>
            </div>
            <span style={{ color: c.decision === "allow" ? "var(--accent)" : "#ff6b6b", fontWeight: 600 }}>
              {c.decision}
            </span>
            <span style={{ color: "var(--muted)", fontSize: "0.85em" }}>
              {(c.sizeBytes / 1024).toFixed(1)} KB
            </span>
            <button className="ghost" disabled title="Real download lands in CTI-3-4 main slice 2">
              Download
            </button>
          </li>
        ))}
      </ul>
      <p style={{ color: "var(--muted)", marginTop: "2em", fontSize: "0.9em" }}>
        Mock data. Real capsule library + download via daemon adapter lands in CTI-3-4 main slice 2.
      </p>
    </main>
  );
}
