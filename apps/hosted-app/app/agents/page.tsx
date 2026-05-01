import { auth } from "@/auth";
import { mockAgents } from "@/lib/mock-data";

export default async function AgentsPage() {
  const session = await auth();
  const agents = mockAgents; // CTI-3-4 main slice 2: scope to session.user

  return (
    <main>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1.5em" }}>
        <h1>Agents</h1>
        <button disabled title="ENS subname issuance via Durin lands once Ivan ships ENS-AGENT-A2">+ New agent</button>
      </header>
      <p style={{ color: "var(--muted)", marginBottom: "2em" }}>
        Hi @{session?.user?.githubLogin ?? session?.user?.name ?? "developer"}. Each agent gets an ENS subname under{" "}
        <code>sbo3lagent.eth</code> (issuance gated on Ivan's ENS path).
      </p>
      <table style={{ width: "100%", borderCollapse: "collapse" }}>
        <thead>
          <tr style={{ borderBottom: "1px solid var(--border)", textAlign: "left", color: "var(--muted)" }}>
            <th style={{ padding: "0.5em 0.8em" }}>Agent ID</th>
            <th style={{ padding: "0.5em 0.8em" }}>ENS</th>
            <th style={{ padding: "0.5em 0.8em" }}>Pubkey</th>
            <th style={{ padding: "0.5em 0.8em" }}>Created</th>
          </tr>
        </thead>
        <tbody>
          {agents.map((a) => (
            <tr key={a.agentId} style={{ borderBottom: "1px solid var(--border)" }}>
              <td style={{ padding: "0.6em 0.8em" }}><code>{a.agentId}</code></td>
              <td style={{ padding: "0.6em 0.8em", color: "var(--muted)" }}>{a.ensName}</td>
              <td style={{ padding: "0.6em 0.8em", color: "var(--muted)" }}><code>{a.pubkeyPrefix}</code></td>
              <td style={{ padding: "0.6em 0.8em", color: "var(--muted)" }}>{new Date(a.createdAt).toLocaleString()}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p style={{ color: "var(--muted)", marginTop: "2em", fontSize: "0.9em" }}>
        Mock data. Real per-tenant agent list (Grace's daemon-side path layout) lands in CTI-3-4 main slice 2.
      </p>
    </main>
  );
}
