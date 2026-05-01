// Role-based access control for the hosted app.
//
// Three roles, ordered by capability:
//   viewer    — read-only; sees own dashboard, audit, capsules
//   operator  — viewer + can register agents, run dry-run policy checks
//   admin     — operator + can edit policy, manage feature flags, see other tenants
//
// Role assignment today is env-config: ADMIN_GITHUB_LOGINS comma list,
// OPERATOR_GITHUB_LOGINS comma list, ADMIN_EMAILS comma list,
// OPERATOR_EMAILS comma list. Anyone not in a list is `viewer`.
//
// CTI-3-4 slice 3 swaps env config for a daemon-backed roles table
// (tenant-scoped); same RoleResolver interface.

export type Role = "admin" | "operator" | "viewer";

export const ROLE_RANK: Record<Role, number> = {
  viewer: 0,
  operator: 1,
  admin: 2,
};

export function meetsRole(have: Role | undefined, need: Role): boolean {
  if (!have) return false;
  return ROLE_RANK[have] >= ROLE_RANK[need];
}

export interface RoleResolverInput {
  githubLogin?: string | null;
  email?: string | null;
}

function csvEnv(name: string): string[] {
  const raw = process.env[name];
  if (!raw) return [];
  return raw
    .split(",")
    .map((s) => s.trim().toLowerCase())
    .filter(Boolean);
}

export function resolveRole({ githubLogin, email }: RoleResolverInput): Role {
  const handle = githubLogin?.toLowerCase();
  const mail = email?.toLowerCase();

  if (handle && csvEnv("ADMIN_GITHUB_LOGINS").includes(handle)) return "admin";
  if (mail && csvEnv("ADMIN_EMAILS").includes(mail)) return "admin";

  if (handle && csvEnv("OPERATOR_GITHUB_LOGINS").includes(handle)) return "operator";
  if (mail && csvEnv("OPERATOR_EMAILS").includes(mail)) return "operator";

  return "viewer";
}
