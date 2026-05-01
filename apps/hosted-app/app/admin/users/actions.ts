"use server";

import { auth } from "@/auth";
import { meetsRole, type Role } from "@/lib/roles";

export interface SetRoleResult {
  ok: boolean;
  error?: string;
}

// Today: env-only persistence. The daemon has no /v1/admin/users
// endpoint yet (Grace's per-tenant SQLite slice). So this server
// action returns a clear pending-feature message instead of silently
// no-oping. The UI surfaces the error inline next to the row.
//
// When persistence lands:
//   POST ${SBO3L_DAEMON_URL}/v1/admin/users
//   { identifier, identifier_kind, role }
// returns { ok: true } and the daemon appends a `role.changed`
// audit event with prev_role + new_role.
export async function setUserRole(
  _identifier: string,
  _identifierKind: "github_login" | "email",
  _newRole: Role,
): Promise<SetRoleResult> {
  const session = await auth();
  if (!meetsRole(session?.user?.role, "admin")) {
    return { ok: false, error: "admin role required" };
  }

  return {
    ok: false,
    error:
      "Daemon /v1/admin/users persistence endpoint not yet implemented (tracked as Grace's per-tenant slice). For now, edit the ADMIN_GITHUB_LOGINS / OPERATOR_GITHUB_LOGINS / *_EMAILS env vars in your Vercel project settings, then redeploy. Roles take effect on next session refresh — no re-login required.",
  };
}
