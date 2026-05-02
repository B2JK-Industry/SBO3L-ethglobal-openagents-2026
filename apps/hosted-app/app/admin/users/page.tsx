import { auth } from "@/auth";
import { listKnownUsers } from "@/lib/known-users";
import { type Role } from "@/lib/roles";
import { UserRoleRow } from "./UserRoleRow";

export default async function AdminUsersPage() {
  const session = await auth();
  // middleware.ts ROLE_GATES enforces admin for /admin/*; the optional-
  // chain below is for type narrowing only.
  const handle = session?.user?.githubLogin ?? "admin";

  const users = listKnownUsers({
    githubLogin: session?.user?.githubLogin ?? null,
    email: null,
  });

  const counts = users.reduce<Record<Role, number>>(
    (acc, u) => ({ ...acc, [u.role]: (acc[u.role] ?? 0) + 1 }),
    { admin: 0, operator: 0, viewer: 0 },
  );

  return (
    <main>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1em" }}>
        <h1>Users</h1>
        <span style={{ color: "var(--muted)", fontSize: "0.85em" }}>
          admin <strong style={{ color: "var(--fg)" }}>{counts.admin}</strong> ·
          operator <strong style={{ color: "var(--fg)" }}>{counts.operator}</strong> ·
          viewer <strong style={{ color: "var(--fg)" }}>{counts.viewer}</strong>
        </span>
      </header>
      <p style={{ color: "var(--muted)", marginBottom: "1em", maxWidth: 760 }}>
        Hi @{handle}. Below is every identifier currently in env-config role lists
        (<code>ADMIN_*</code> / <code>OPERATOR_*</code>) plus your current session.
        Anyone who has signed in but isn't in any list is silently <code>viewer</code>
        and not shown — that's the default.
      </p>
      <aside
        role="status"
        style={{
          background: "var(--code-bg)",
          border: "1px solid var(--border)",
          borderLeft: "3px solid var(--accent)",
          borderRadius: "var(--r-md)",
          padding: "0.7em 1em",
          marginBottom: "1.5em",
          color: "var(--muted)",
          fontSize: "0.9em",
        }}
      >
        <strong style={{ color: "var(--fg)" }}>Persistence status:</strong> daemon{" "}
        <code>/v1/admin/users</code> endpoint not yet implemented (Grace's per-tenant slice).
        Save returns a pending-feature error explaining how to edit env vars in the meantime.
        Listings remain accurate — they re-read env on every page load.
      </aside>

      <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "0.92em" }}>
        <thead>
          <tr style={{ borderBottom: "1px solid var(--border)", textAlign: "left", color: "var(--muted)" }}>
            <th style={{ padding: "0.6em 0.8em" }}>Identifier</th>
            <th style={{ padding: "0.6em 0.8em" }}>Kind</th>
            <th style={{ padding: "0.6em 0.8em" }}>Role</th>
            <th style={{ padding: "0.6em 0.8em" }}>Source</th>
            <th style={{ padding: "0.6em 0.8em" }}></th>
          </tr>
        </thead>
        <tbody>
          {users.map((u) => (
            <UserRoleRow key={`${u.identifier_kind}:${u.identifier}`} user={u} />
          ))}
          {users.length === 0 && (
            <tr>
              <td colSpan={5} style={{ padding: "1em 0.8em", color: "var(--muted)", textAlign: "center" }}>
                No users in role config yet. Edit <code>ADMIN_GITHUB_LOGINS</code> in your Vercel project to grant yourself admin.
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </main>
  );
}
