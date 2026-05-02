"use client";

import useSWR from "swr";
import { useState } from "react";
import type { FeatureFlag, FlagsResponse } from "@/lib/flags-client";
import { fetchFlags, setFlagEnabled } from "./actions";

interface Props {
  initial: FlagsResponse;
}

const REFRESH_MS = 10_000;

// Live admin table with optimistic toggle + rollback on error. SWR
// drives the 10-second background refresh so flag changes from
// other admins surface without a manual reload.
export function FlagsTable({ initial }: Props): JSX.Element {
  const { data, error, isValidating, mutate } = useSWR<FlagsResponse>(
    "admin-flags",
    async () => {
      const res = await fetchFlags();
      if (!res.ok) throw new Error(res.error ?? "fetch failed");
      return res.data!;
    },
    {
      fallbackData: initial,
      refreshInterval: REFRESH_MS,
      revalidateOnFocus: true,
    },
  );

  const flags: FeatureFlag[] = data?.flags ?? [];
  const grouped = groupByCategory(flags);
  const [busy, setBusy] = useState<string | null>(null);
  const [rowError, setRowError] = useState<{ name: string; message: string } | null>(null);

  const onToggle = async (flag: FeatureFlag, next: boolean): Promise<void> => {
    setBusy(flag.name);
    setRowError(null);

    // Optimistic update — replace the flag locally before the server
    // round-trip completes; SWR re-fetches on success or error.
    const prevFlags = data?.flags ?? [];
    const optimistic: FlagsResponse = {
      flags: prevFlags.map((f) => (f.name === flag.name ? { ...f, enabled: next } : f)),
      fetched_at: data?.fetched_at ?? new Date().toISOString(),
    };
    await mutate(optimistic, { revalidate: false });

    const res = await setFlagEnabled(flag.name, next);
    if (!res.ok) {
      // Rollback to the prior state and surface the error inline.
      await mutate({ flags: prevFlags, fetched_at: data?.fetched_at ?? new Date().toISOString() }, { revalidate: false });
      setRowError({ name: flag.name, message: res.error ?? "toggle failed" });
    } else {
      // Trigger a background revalidate to pick up server-canonical
      // last_changed_at and last_changed_by fields.
      await mutate();
    }
    setBusy(null);
  };

  return (
    <div>
      <div style={{ display: "flex", justifyContent: "flex-end", marginBottom: "0.6em", color: "var(--muted)", fontSize: "0.85em" }}>
        {error && <span style={{ color: "#ff6b6b" }}>● error: {(error as Error).message}</span>}
        {!error && isValidating && <span>● refreshing…</span>}
        {!error && !isValidating && data && (
          <span>● live · last fetch {new Date(data.fetched_at).toLocaleTimeString()}</span>
        )}
      </div>

      {Object.entries(grouped).map(([category, group]) => (
        <section key={category} style={{ marginBottom: "1.5em" }}>
          {category !== "_default" && (
            <h2 style={{ fontSize: "0.85em", color: "var(--muted)", textTransform: "uppercase", letterSpacing: "0.08em", margin: "1em 0 0.5em" }}>
              {category}
            </h2>
          )}
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "0.92em" }}>
            <thead>
              <tr style={{ borderBottom: "1px solid var(--border)", textAlign: "left", color: "var(--muted)" }}>
                <th style={{ padding: "0.55em 0.7em" }}>Flag</th>
                <th style={{ padding: "0.55em 0.7em" }}>Description</th>
                <th style={{ padding: "0.55em 0.7em" }}>Last changed</th>
                <th style={{ padding: "0.55em 0.7em", textAlign: "right" }}>State</th>
              </tr>
            </thead>
            <tbody>
              {group.map((f) => (
                <tr key={f.name} style={{ borderBottom: "1px solid var(--border)" }}>
                  <td style={{ padding: "0.6em 0.7em", verticalAlign: "top" }}>
                    <code>{f.name}</code>
                    {f.default_value !== f.enabled && (
                      <span style={{ marginLeft: "0.5em", color: "var(--muted)", fontSize: "0.78em" }}>
                        (default {f.default_value ? "on" : "off"})
                      </span>
                    )}
                  </td>
                  <td style={{ padding: "0.6em 0.7em", color: "var(--muted)", verticalAlign: "top" }}>{f.description}</td>
                  <td style={{ padding: "0.6em 0.7em", color: "var(--muted)", verticalAlign: "top", fontFamily: "var(--font-mono)", fontSize: "0.82em" }}>
                    {f.last_changed_at ? (
                      <>
                        {new Date(f.last_changed_at).toLocaleString()}
                        {f.last_changed_by && <div>by {f.last_changed_by}</div>}
                      </>
                    ) : (
                      <em>never</em>
                    )}
                  </td>
                  <td style={{ padding: "0.6em 0.7em", textAlign: "right", verticalAlign: "top" }}>
                    <Toggle
                      checked={f.enabled}
                      busy={busy === f.name}
                      onChange={(next) => onToggle(f, next)}
                      label={`Toggle ${f.name}`}
                    />
                    {rowError?.name === f.name && (
                      <div role="alert" style={{ color: "#ff6b6b", fontSize: "0.78em", marginTop: "0.3em" }}>
                        ✗ {rowError.message}
                      </div>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      ))}

      {flags.length === 0 && (
        <p style={{ color: "var(--muted)", textAlign: "center", padding: "2em 0" }}>
          No flags registered. Dev 1's <code>feature_flags::register()</code> calls populate this list.
        </p>
      )}
    </div>
  );
}

function groupByCategory(flags: FeatureFlag[]): Record<string, FeatureFlag[]> {
  const out: Record<string, FeatureFlag[]> = {};
  for (const f of flags) {
    const k = f.category ?? "_default";
    (out[k] ??= []).push(f);
  }
  return out;
}

interface ToggleProps {
  checked: boolean;
  busy: boolean;
  onChange: (next: boolean) => void;
  label: string;
}

function Toggle({ checked, busy, onChange, label }: ToggleProps): JSX.Element {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={busy}
      onClick={() => onChange(!checked)}
      style={{
        position: "relative",
        width: "3em",
        height: "1.6em",
        padding: 0,
        borderRadius: "1em",
        border: `1px solid var(--border)`,
        background: checked ? "var(--accent)" : "var(--code-bg)",
        cursor: busy ? "wait" : "pointer",
        transition: "background 0.15s",
      }}
    >
      <span
        style={{
          position: "absolute",
          top: "2px",
          left: checked ? "calc(100% - 1.4em)" : "2px",
          width: "1.2em",
          height: "1.2em",
          borderRadius: "50%",
          background: checked ? "var(--bg)" : "var(--fg)",
          transition: "left 0.15s",
        }}
      />
    </button>
  );
}
