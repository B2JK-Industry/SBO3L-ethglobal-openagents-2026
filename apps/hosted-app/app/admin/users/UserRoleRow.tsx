"use client";

import { useState } from "react";
import type { KnownUser } from "@/lib/known-users";
import type { Role } from "@/lib/roles";
import { setUserRole } from "./actions";

const ROLES: Role[] = ["viewer", "operator", "admin"];

export function UserRoleRow({ user }: { user: KnownUser }): JSX.Element {
  const [role, setRole] = useState<Role>(user.role);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dirty = role !== user.role;

  const onSave = async (): Promise<void> => {
    setBusy(true);
    setError(null);
    const res = await setUserRole(user.identifier, user.identifier_kind, role);
    if (!res.ok) setError(res.error ?? "save failed");
    setBusy(false);
  };

  return (
    <tr style={{ borderBottom: "1px solid var(--border)" }}>
      <td style={{ padding: "0.7em 0.8em" }}>
        <code>{user.identifier}</code>
      </td>
      <td style={{ padding: "0.7em 0.8em", color: "var(--muted)" }}>
        {user.identifier_kind === "github_login" ? "GitHub" : "Email"}
      </td>
      <td style={{ padding: "0.7em 0.8em" }}>
        <select
          value={role}
          onChange={(ev) => setRole(ev.target.value as Role)}
          aria-label={`Role for ${user.identifier}`}
          style={{
            background: "var(--code-bg)",
            color: "var(--fg)",
            border: "1px solid var(--border)",
            borderRadius: "var(--r-sm)",
            padding: "0.3em 0.6em",
            fontFamily: "var(--font-mono)",
            fontSize: "0.92em",
          }}
        >
          {ROLES.map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
        </select>
      </td>
      <td style={{ padding: "0.7em 0.8em", color: "var(--muted)", fontSize: "0.85em" }}>
        {user.source === "env_config" ? <code>{user.added_via}</code> : <em>{user.added_via}</em>}
      </td>
      <td style={{ padding: "0.7em 0.8em" }}>
        {dirty && (
          <button
            type="button"
            onClick={onSave}
            disabled={busy}
            style={{ padding: "0.4em 0.9em", fontSize: "0.85em" }}
          >
            {busy ? "Saving…" : "Save"}
          </button>
        )}
        {error && (
          <span
            style={{ color: "#ff6b6b", fontSize: "0.8em", marginLeft: "0.6em" }}
            role="alert"
            title={error}
          >
            ✗ {error.length > 70 ? `${error.slice(0, 70)}…` : error}
          </span>
        )}
      </td>
    </tr>
  );
}
