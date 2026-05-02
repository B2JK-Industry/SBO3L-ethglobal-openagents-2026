"use client";

import { useState } from "react";
import { runKmsHealthCheck, type KmsStatus, type KmsStatusResult } from "./actions";

interface Props {
  initial: KmsStatusResult;
}

const BACKEND_LABEL: Record<KmsStatus["backend"], string> = {
  "in-memory": "In-memory (dev only)",
  "file":      "File-backed",
  "kms-aws":   "AWS KMS",
  "kms-gcp":   "Google Cloud KMS",
  "vault":     "HashiCorp Vault",
  "unknown":   "Unknown",
};

const BACKEND_HINT: Record<KmsStatus["backend"], string> = {
  "in-memory": "Loses signing key on daemon restart. Acceptable for local dev only.",
  "file":      "Persists across restarts. Acceptable for local production where the file lives on encrypted storage.",
  "kms-aws":   "Production-recommended. Private key never leaves AWS KMS.",
  "kms-gcp":   "Production-recommended. Private key never leaves GCP KMS.",
  "vault":     "Production-recommended. Private key never leaves Vault.",
  "unknown":   "Daemon reported a backend kind this UI doesn't recognise yet.",
};

export function KmsStatusCard({ initial }: Props): JSX.Element {
  const [data, setData] = useState<KmsStatusResult>(initial);
  const [busy, setBusy] = useState(false);

  const onHealthCheck = async (): Promise<void> => {
    setBusy(true);
    const next = await runKmsHealthCheck();
    setData(next);
    setBusy(false);
  };

  if (!data.ok && data.pending) {
    return (
      <aside
        role="status"
        style={{
          background: "var(--code-bg)",
          border: "1px solid var(--border)",
          borderLeft: "3px solid var(--accent)",
          borderRadius: "var(--r-md)",
          padding: "1.2em 1.4em",
          color: "var(--muted)",
        }}
      >
        <strong style={{ color: "var(--accent)" }}>Endpoint pending.</strong>{" "}
        <span style={{ display: "block", marginTop: "0.4em" }}>{data.error}</span>
        <span style={{ display: "block", marginTop: "0.6em", fontSize: "0.85em" }}>
          When Dev 1 ships <code>/v1/admin/kms/status</code> + <code>/v1/admin/kms/health</code>, this page will activate automatically — no UI changes needed.
        </span>
      </aside>
    );
  }

  if (!data.ok) {
    return (
      <aside
        role="alert"
        style={{
          background: "var(--code-bg)",
          border: "1px solid var(--border)",
          borderLeft: "3px solid #ff6b6b",
          borderRadius: "var(--r-md)",
          padding: "1.2em 1.4em",
          color: "var(--muted)",
        }}
      >
        <strong style={{ color: "#ff6b6b" }}>Daemon unreachable.</strong>
        <span style={{ display: "block", marginTop: "0.4em" }}>{data.error}</span>
      </aside>
    );
  }

  const s = data.status!;

  return (
    <div style={{ display: "grid", gap: "1em" }}>
      <article style={{ padding: "1.4em", border: "1px solid var(--border)", borderRadius: "var(--r-lg)", background: "var(--code-bg)" }}>
        <header style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: "0.8em" }}>
          <h2 style={{ fontSize: "1.1em" }}>Backend</h2>
          <span style={{ color: s.configured ? "var(--accent)" : "#ff6b6b", fontFamily: "var(--font-mono)", fontSize: "0.85em" }}>
            ● {s.configured ? "configured" : "not configured"}
          </span>
        </header>
        <p style={{ fontSize: "1.1em", color: "var(--fg)", marginBottom: "0.4em" }}>
          {BACKEND_LABEL[s.backend]}
        </p>
        <p style={{ color: "var(--muted)", fontSize: "0.9em", marginBottom: "0.8em" }}>{BACKEND_HINT[s.backend]}</p>

        <dl style={{ display: "grid", gridTemplateColumns: "10em 1fr", gap: "0.4em 1em", margin: 0, fontSize: "0.9em" }}>
          {s.pubkey_b58 && <><dt style={{ color: "var(--muted)" }}>Public key</dt><dd><code>{s.pubkey_b58.slice(0, 24)}…</code></dd></>}
          {s.key_id && <><dt style={{ color: "var(--muted)" }}>Key ID</dt><dd><code>{s.key_id}</code></dd></>}
          {s.region && <><dt style={{ color: "var(--muted)" }}>Region</dt><dd><code>{s.region}</code></dd></>}
          <dt style={{ color: "var(--muted)" }}>Last signed</dt>
          <dd>{s.last_signed_at ? new Date(s.last_signed_at).toLocaleString() : <em style={{ color: "var(--muted)" }}>never</em>}</dd>
          {s.last_signed_by_request_hash && (
            <>
              <dt style={{ color: "var(--muted)" }}>Last request_hash</dt>
              <dd><code>{s.last_signed_by_request_hash.slice(0, 18)}…</code></dd>
            </>
          )}
        </dl>
      </article>

      <article style={{ padding: "1.4em", border: "1px solid var(--border)", borderRadius: "var(--r-lg)", background: "var(--code-bg)" }}>
        <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.8em" }}>
          <h2 style={{ fontSize: "1.1em" }}>Health check</h2>
          <button onClick={onHealthCheck} disabled={busy} className="ghost">
            {busy ? "Checking…" : "Run check"}
          </button>
        </header>
        {!s.health && <p style={{ color: "var(--muted)" }}>Click "Run check" to issue a no-op sign request and confirm the signer is reachable.</p>}
        {s.health?.ok === true && (
          <p style={{ color: "var(--accent)" }}>
            ✓ healthy{s.health.rtt_ms !== undefined && ` · ${s.health.rtt_ms} ms RTT`}
            {s.health.checked_at && <span style={{ color: "var(--muted)", marginLeft: "0.6em" }}>at {new Date(s.health.checked_at).toLocaleTimeString()}</span>}
          </p>
        )}
        {s.health?.ok === false && (
          <p style={{ color: "#ff6b6b" }}>
            ✗ unhealthy{s.health.detail && ` — ${s.health.detail}`}
          </p>
        )}
      </article>
    </div>
  );
}
