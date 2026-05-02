// "Known users" surface for the /admin/users page.
//
// Today the system has no persistence layer — roles are env-config
// (ADMIN_GITHUB_LOGINS / OPERATOR_GITHUB_LOGINS / *_EMAILS). So
// "known users" = the union of every identifier currently listed in
// any of those env vars, plus the currently logged-in admin.
//
// Slice 2 (after Grace's per-tenant SQLite lands) replaces this with
// a daemon round-trip to GET /v1/admin/users. The shape returned
// matches KnownUser already so the swap is one-file.

import { resolveRole, type Role } from "./roles";

export interface KnownUser {
  identifier: string;       // GitHub login or email — whichever the env lists
  identifier_kind: "github_login" | "email";
  role: Role;
  source: "env_config" | "session";
  added_via: string;        // env var name OR "current session"
}

function csvEnv(name: string): string[] {
  const raw = process.env[name];
  if (!raw) return [];
  return raw.split(",").map((s) => s.trim().toLowerCase()).filter(Boolean);
}

export function listKnownUsers(currentUser?: { githubLogin?: string | null; email?: string | null }): KnownUser[] {
  const seen = new Map<string, KnownUser>();

  const add = (identifier: string, kind: KnownUser["identifier_kind"], envVar: string): void => {
    const key = `${kind}:${identifier}`;
    if (seen.has(key)) return;
    seen.set(key, {
      identifier,
      identifier_kind: kind,
      role: resolveRole(
        kind === "github_login" ? { githubLogin: identifier } : { email: identifier },
      ),
      source: "env_config",
      added_via: envVar,
    });
  };

  for (const v of csvEnv("ADMIN_GITHUB_LOGINS")) add(v, "github_login", "ADMIN_GITHUB_LOGINS");
  for (const v of csvEnv("ADMIN_EMAILS")) add(v, "email", "ADMIN_EMAILS");
  for (const v of csvEnv("OPERATOR_GITHUB_LOGINS")) add(v, "github_login", "OPERATOR_GITHUB_LOGINS");
  for (const v of csvEnv("OPERATOR_EMAILS")) add(v, "email", "OPERATOR_EMAILS");

  // Include the current session user if not already in env (so the
  // page shows them as "you, viewer, added via current session").
  if (currentUser?.githubLogin) {
    const id = currentUser.githubLogin.toLowerCase();
    if (!seen.has(`github_login:${id}`)) {
      seen.set(`github_login:${id}`, {
        identifier: id,
        identifier_kind: "github_login",
        role: resolveRole({ githubLogin: id, email: currentUser.email ?? undefined }),
        source: "session",
        added_via: "current session",
      });
    }
  }

  return [...seen.values()].sort((a, b) => {
    const rank = { admin: 0, operator: 1, viewer: 2 } as const;
    if (rank[a.role] !== rank[b.role]) return rank[a.role] - rank[b.role];
    return a.identifier.localeCompare(b.identifier);
  });
}
