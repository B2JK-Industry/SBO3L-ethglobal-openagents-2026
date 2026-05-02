"use client";

import { useState } from "react";

interface Props {
  tenantSlug: string;
  targetTier: "pro" | "enterprise";
  isUpgrade: boolean;
  label: string;
}

export function UpgradeButton({ tenantSlug, targetTier, isUpgrade, label }: Props): JSX.Element {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onClick = async (): Promise<void> => {
    setBusy(true);
    setError(null);
    try {
      const res = await fetch("/api/billing/checkout", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ tenant_slug: tenantSlug, target_tier: targetTier }),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.detail ?? body.error ?? `HTTP ${res.status}`);
      }
      const { url } = (await res.json()) as { url: string };
      window.location.href = url;
    } catch (e) {
      setError((e as Error).message);
      setBusy(false);
    }
  };

  return (
    <>
      <button onClick={onClick} disabled={busy} className={isUpgrade ? "" : "ghost"} style={{ width: "100%", fontSize: "0.85em" }}>
        {busy ? "Redirecting…" : label}
      </button>
      {error && <p style={{ color: "#f87171", fontSize: "0.75em", marginTop: "0.4em" }}>{error}</p>}
    </>
  );
}
