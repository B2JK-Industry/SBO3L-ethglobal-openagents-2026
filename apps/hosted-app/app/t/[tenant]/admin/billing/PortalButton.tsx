"use client";

import { useState } from "react";

interface Props {
  tenantSlug: string;
  label: string;
}

// Codex review fix (PR #360): the previous billing page replaced the
// broken downgrade button with static "Downgrade via Customer Portal"
// text — admins had no actionable path. This button POSTs to
// /api/billing/portal and redirects to the Stripe-hosted Customer
// Portal where the admin can change plan, update payment method,
// or cancel. The portal handles tier transitions on the existing
// subscription correctly (vs Checkout which would create a duplicate).

export function PortalButton({ tenantSlug, label }: Props): JSX.Element {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onClick = async (): Promise<void> => {
    setBusy(true);
    setError(null);
    try {
      const res = await fetch("/api/billing/portal", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ tenant_slug: tenantSlug }),
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
      <button onClick={onClick} disabled={busy} className="ghost" style={{ width: "100%", fontSize: "0.85em" }}>
        {busy ? "Redirecting…" : label}
      </button>
      {error && <p style={{ color: "#f87171", fontSize: "0.75em", marginTop: "0.4em" }}>{error}</p>}
    </>
  );
}
