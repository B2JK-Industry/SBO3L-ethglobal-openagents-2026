import { DaemonStatusBanner } from "@/components/DaemonStatusBanner";
import { listAudit, runPassport, DaemonError, type PassportCapsule } from "@/lib/sbo3l-client";
import { mockCapsules } from "@/lib/mock-data";

interface CapsuleRow {
  capsuleId: string;
  agentId: string;
  decision: "allow" | "deny";
  emittedAt: string;
  sizeBytes: number;
  jsonHref?: string;
}

function buildJsonHref(capsule: PassportCapsule): string {
  // Inline data: URL — small enough for individual capsules (~10-15 KB).
  // Slice 3 swaps to a server-action route that streams from the daemon
  // for capsules > 50 KB.
  const json = JSON.stringify(capsule, null, 2);
  return `data:application/json;charset=utf-8,${encodeURIComponent(json)}`;
}

export default async function CapsulesPage() {
  let rows: CapsuleRow[];

  try {
    const page = await listAudit({ limit: 50 });
    const decisionEvents = page.events.filter((e) => e.event_type === "policy.decision");
    const capsules = await Promise.all(
      decisionEvents.slice(0, 20).map(async (e) => {
        try {
          const c = await runPassport(e.request_hash);
          return { event: e, capsule: c };
        } catch {
          return { event: e, capsule: null };
        }
      }),
    );
    rows = capsules
      .filter((x): x is { event: typeof x.event; capsule: PassportCapsule } => x.capsule !== null)
      .map(({ event, capsule }) => ({
        capsuleId: capsule.capsule_id,
        agentId: event.agent_id,
        decision: capsule.policy_receipt.decision,
        emittedAt: capsule.emitted_at,
        sizeBytes: capsule.size_bytes,
        jsonHref: buildJsonHref(capsule),
      }));
  } catch (err) {
    if (!(err instanceof DaemonError)) throw err;
    rows = mockCapsules.map((c) => ({
      capsuleId: c.capsuleId,
      agentId: c.agentId,
      decision: c.decision,
      emittedAt: c.emittedAt,
      sizeBytes: c.sizeBytes,
    }));
  }

  return (
    <main>
      <DaemonStatusBanner />
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1.5em" }}>
        <h1>Capsule library</h1>
        <a href="https://sbo3l-marketing.vercel.app/proof" style={{ color: "var(--accent)" }}>
          Verify in browser →
        </a>
      </header>
      <p style={{ color: "var(--muted)", marginBottom: "2em" }}>
        Self-contained Passport capsules. Each is offline-verifiable against the agent's published Ed25519 pubkey alone (see{" "}
        <a href="https://sbo3l-docs.vercel.app/concepts/capsule">/concepts/capsule</a>).
      </p>
      {rows.length === 0 && <p style={{ color: "var(--muted)" }}>No capsules yet.</p>}
      <ul style={{ listStyle: "none", padding: 0, display: "grid", gap: "0.8em" }}>
        {rows.map((c) => (
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
            <span style={{ color: c.decision === "allow" ? "var(--accent)" : "#ff6b6b", fontWeight: 600 }}>{c.decision}</span>
            <span style={{ color: "var(--muted)", fontSize: "0.85em" }}>{(c.sizeBytes / 1024).toFixed(1)} KB</span>
            {c.jsonHref ? (
              <a href={c.jsonHref} download={`${c.capsuleId}.json`} className="btn ghost" style={{ padding: "0.4em 0.9em" }}>
                Download
              </a>
            ) : (
              <button className="ghost" disabled title="Run the daemon to enable downloads">
                Download
              </button>
            )}
          </li>
        ))}
      </ul>
    </main>
  );
}
