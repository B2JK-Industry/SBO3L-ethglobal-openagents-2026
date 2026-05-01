"use client";

import { useCallback, useState } from "react";
import { PolicyEditor } from "@/components/PolicyEditor";
import { STARTER_POLICY } from "@/lib/policy-schema";
import { dryRunPolicy, savePolicy, type DryRunResult, type SaveResult } from "./actions";

const SAMPLE_APRP = `{
  "agent_id": "research-agent-01",
  "intent": "swap",
  "amount": "0.05",
  "asset": "ETH",
  "chain": "sepolia",
  "expiry": "2026-12-31T23:59:59Z",
  "risk_class": "low",
  "nonce": "01HZRGABCDEFGHJKMNPQRSTV"
}
`;

export default function PolicyEditPage(): JSX.Element {
  const [policy, setPolicy] = useState(STARTER_POLICY);
  const [aprp, setAprp] = useState(SAMPLE_APRP);
  const [valid, setValid] = useState(true);
  const [dryRun, setDryRun] = useState<DryRunResult | null>(null);
  const [save, setSave] = useState<SaveResult | null>(null);
  const [busy, setBusy] = useState<"dry-run" | "save" | null>(null);

  const onEdit = useCallback((value: string, isValid: boolean) => {
    setPolicy(value);
    setValid(isValid);
    setDryRun(null);
  }, []);

  const onDryRun = async (): Promise<void> => {
    setBusy("dry-run");
    setDryRun(await dryRunPolicy(policy, aprp));
    setBusy(null);
  };

  const onSave = async (): Promise<void> => {
    setBusy("save");
    setSave(await savePolicy(policy));
    setBusy(null);
  };

  return (
    <main>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1em" }}>
        <h1>Edit policy</h1>
        <span style={{ color: valid ? "var(--accent)" : "#ff6b6b", fontFamily: "var(--font-mono)", fontSize: "0.85em" }}>
          {valid ? "● schema valid" : "● schema invalid"}
        </span>
      </header>
      <p style={{ color: "var(--muted)", marginBottom: "1.5em", maxWidth: 760 }}>
        Edit the active <code>sbo3l.policy.v1</code> file. Monaco autocompletes against the schema; inline diagnostics flag unknown fields. Use <strong>Test against APRP</strong> to dry-run any envelope through your in-progress policy without committing — calls <code>POST /v1/policy/check</code> with <code>_policy_override</code>.
      </p>

      <PolicyEditor initial={STARTER_POLICY} onChange={onEdit} />

      <section style={{ marginTop: "1.5em", display: "grid", gridTemplateColumns: "1fr 1fr", gap: "1.2em" }}>
        <div>
          <label style={{ fontSize: "0.85em", color: "var(--muted)" }}>Sample APRP envelope</label>
          <textarea
            value={aprp}
            onChange={(ev) => setAprp(ev.target.value)}
            rows={10}
            style={{ width: "100%", marginTop: "0.4em", background: "var(--code-bg)", color: "var(--fg)", border: "1px solid var(--border)", borderRadius: "var(--r-md)", padding: "0.8em", fontFamily: "var(--font-mono)", fontSize: "12px" }}
          />
        </div>
        <div>
          <label style={{ fontSize: "0.85em", color: "var(--muted)" }}>Dry-run result</label>
          <div style={{ marginTop: "0.4em", padding: "1em", background: "var(--code-bg)", border: "1px solid var(--border)", borderRadius: "var(--r-md)", minHeight: "100px", fontSize: "0.92em" }}>
            {!dryRun && <span style={{ color: "var(--muted)" }}>Click "Test against APRP" to project a decision.</span>}
            {dryRun && !dryRun.ok && <span style={{ color: "#ff6b6b" }}>Error: {dryRun.error}</span>}
            {dryRun?.ok && (
              <div style={{ display: "grid", gap: "0.4em" }}>
                <div>Decision: <strong style={{ color: dryRun.decision === "allow" ? "var(--accent)" : "#ff6b6b" }}>{dryRun.decision}</strong></div>
                {dryRun.deny_code && <div>Deny code: <code>{dryRun.deny_code}</code></div>}
                <div style={{ color: "var(--muted)", fontSize: "0.85em" }}>policy_snapshot_hash: <code>{dryRun.policy_snapshot_hash?.slice(0, 18)}…</code></div>
              </div>
            )}
          </div>
        </div>
      </section>

      <section style={{ marginTop: "1.5em", display: "flex", gap: "0.8em", alignItems: "center", flexWrap: "wrap" }}>
        <button onClick={onDryRun} disabled={!valid || busy !== null}>
          {busy === "dry-run" ? "Testing…" : "Test against APRP"}
        </button>
        <button onClick={onSave} disabled={!valid || busy !== null} className="ghost">
          {busy === "save" ? "Saving…" : "Save policy"}
        </button>
        {save && !save.ok && <span style={{ color: "#ff6b6b", fontSize: "0.9em" }}>{save.error}</span>}
        {save?.ok && <span style={{ color: "var(--accent)", fontSize: "0.9em" }}>Saved · policy_hash {save.policy_hash?.slice(0, 18)}…</span>}
      </section>
    </main>
  );
}
