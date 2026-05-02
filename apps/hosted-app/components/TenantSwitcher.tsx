"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import type { TenantMembership, Tenant } from "@/lib/tenants";

interface Props {
  current: string;
  memberships: TenantMembership[];
  tenants: Tenant[];
  basePath: string;
}

export function TenantSwitcher({ current, memberships, tenants, basePath }: Props): JSX.Element {
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const currentTenant = tenants.find((t) => t.slug === current);

  return (
    <div className="tenant-switcher">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={`Tenant: ${currentTenant?.display_name ?? current}`}
      >
        <span className="dot" aria-hidden="true"></span>
        <strong>{currentTenant?.display_name ?? current}</strong>
        <code>{current}</code>
        <span aria-hidden="true">▾</span>
      </button>
      {open && (
        <ul role="listbox" aria-label="Switch tenant">
          {memberships.map((m) => {
            const t = tenants.find((x) => x.slug === m.tenant_slug);
            if (!t) return null;
            return (
              <li key={m.tenant_slug}>
                <button
                  type="button"
                  onClick={() => { setOpen(false); router.push(`/t/${m.tenant_slug}${basePath}`); }}
                  aria-current={m.tenant_slug === current ? "true" : undefined}
                >
                  <span><strong>{t.display_name}</strong> <code>{t.slug}</code></span>
                  <span className="role">{m.role}</span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
      <style jsx>{`
        .tenant-switcher { position: relative; }
        button {
          background: var(--code-bg); color: var(--fg); border: 1px solid var(--border);
          border-radius: var(--r-sm); padding: 0.4em 0.8em; font: inherit; font-size: 0.9em;
          cursor: pointer; display: inline-flex; align-items: center; gap: 0.5em;
        }
        .dot { width: 0.5em; height: 0.5em; border-radius: 50%; background: var(--accent); }
        button code { color: var(--muted); font-size: 0.78em; }
        ul {
          position: absolute; top: calc(100% + 4px); right: 0; margin: 0; padding: 0.4em 0;
          list-style: none; background: var(--bg); border: 1px solid var(--border);
          border-radius: var(--r-sm); min-width: 240px; box-shadow: 0 4px 16px rgba(0,0,0,0.4); z-index: 20;
        }
        li button {
          display: grid; grid-template-columns: 1fr auto; gap: 0.4em;
          padding: 0.55em 0.9em; width: 100%; background: transparent; border: none;
          color: var(--fg); font-size: 0.9em; text-align: left; cursor: pointer; border-radius: 0;
        }
        li button:hover { background: var(--code-bg); }
        li button .role { color: var(--accent); font-family: var(--font-mono); font-size: 0.8em; }
        li button code { color: var(--muted); margin-left: 0.6em; font-size: 0.85em; }
      `}</style>
    </div>
  );
}
