import { auth } from "@/auth";
import { DaemonStatusBanner } from "@/components/DaemonStatusBanner";
import { listAudit, DaemonError } from "@/lib/sbo3l-client";
import { mockAgents } from "@/lib/mock-data";

interface AgentRow {
  agentId: string;
  ensName: string;
  pubkeyPrefix: string;
  lastSeen: string;
  decisionCount: number;
}

export default async function AgentsPage() {
  const session = await auth();
  let rows: AgentRow[];

  try {
    // Roll-up over /v1/audit: aggregate by agent_id, count decisions,
    // pick most-recent timestamp. Slice 3 swaps to /v1/agents once
    // Ivan's ENS-AGENT-A2 subname registry endpoint lands.
    const page = await listAudit({ limit: 200 });
    const byAgent = new Map<string, AgentRow>();
    for (const e of page.events) {
      const existing = byAgent.get(e.agent_id);
      if (existing) {
        existing.decisionCount += e.decision ? 1 : 0;
        if (e.ts_unix_ms > Date.parse(existing.lastSeen)) {
          existing.lastSeen = new Date(e.ts_unix_ms).toISOString();
        }
      } else {
        byAgent.set(e.agent_id, {
          agentId: e.agent_id,
          ensName: `${e.agent_id}.sbo3lagent.eth`,
          pubkeyPrefix: e.agent_pubkey.slice(0, 18),
          lastSeen: new Date(e.ts_unix_ms).toISOString(),
          decisionCount: e.decision ? 1 : 0,
        });
      }
    }
    rows = [...byAgent.values()];
  } catch (err) {
    if (!(err instanceof DaemonError)) throw err;
    rows = mockAgents.map((a) => ({
      agentId: a.agentId,
      ensName: a.ensName,
      pubkeyPrefix: a.pubkeyPrefix,
      lastSeen: a.createdAt,
      decisionCount: 0,
    }));
  }

  return (
    <main>
      <DaemonStatusBanner />
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1.5em" }}>
        <h1>Agents</h1>
        <button disabled title="ENS subname issuance via Durin lands once Ivan ships ENS-AGENT-A2">+ New agent</button>
      </header>
      <p style={{ color: "var(--muted)", marginBottom: "2em" }}>
        Hi @{session?.user?.githubLogin ?? session?.user?.name ?? "developer"}. Each agent gets an ENS subname under{" "}
        <code>sbo3lagent.eth</code> (issuance gated on Ivan's ENS path).
      </p>
      {rows.length === 0 && <p style={{ color: "var(--muted)" }}>No agents yet — make a /v1/payment-requests call to bootstrap one.</p>}
      {rows.length > 0 && (
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ borderBottom: "1px solid var(--border)", textAlign: "left", color: "var(--muted)" }}>
              <th style={{ padding: "0.5em 0.8em" }}>Agent ID</th>
              <th style={{ padding: "0.5em 0.8em" }}>ENS</th>
              <th style={{ padding: "0.5em 0.8em" }}>Pubkey</th>
              <th style={{ padding: "0.5em 0.8em" }}>Decisions</th>
              <th style={{ padding: "0.5em 0.8em" }}>Last seen</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((a) => (
              <tr key={a.agentId} style={{ borderBottom: "1px solid var(--border)" }}>
                <td style={{ padding: "0.6em 0.8em" }}><code>{a.agentId}</code></td>
                <td style={{ padding: "0.6em 0.8em", color: "var(--muted)" }}>{a.ensName}</td>
                <td style={{ padding: "0.6em 0.8em", color: "var(--muted)" }}><code>{a.pubkeyPrefix}</code></td>
                <td style={{ padding: "0.6em 0.8em" }}>{a.decisionCount}</td>
                <td style={{ padding: "0.6em 0.8em", color: "var(--muted)" }}>{new Date(a.lastSeen).toLocaleString()}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </main>
  );
}
