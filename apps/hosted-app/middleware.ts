import { NextResponse } from "next/server";
import NextAuth from "next-auth";
import { authConfig } from "@/auth.config";
import { meetsRole, type Role } from "@/lib/roles";

const { auth } = NextAuth(authConfig);

// Per-route role gate. Auth.config's `authorized` callback already
// blocks unauthenticated access to protected paths; this middleware
// adds the role check for routes that need above viewer.
const ROLE_GATES: Array<{ prefix: string; need: Role }> = [
  { prefix: "/admin",       need: "admin" },
  { prefix: "/policy/edit", need: "admin" },
  { prefix: "/policy",      need: "operator" },
  { prefix: "/agents",      need: "operator" },
  { prefix: "/audit",       need: "viewer" },
  { prefix: "/capsules",    need: "viewer" },
  { prefix: "/dashboard",   need: "viewer" },
];

export default auth((req) => {
  const path = req.nextUrl.pathname;
  const role = req.auth?.user?.role;

  for (const gate of ROLE_GATES) {
    if (path.startsWith(gate.prefix) && !meetsRole(role, gate.need)) {
      // Logged-in but wrong role → /403; logged-out → NextAuth
      // already redirected to /login via authConfig.authorized.
      if (req.auth) {
        return NextResponse.redirect(new URL("/403", req.nextUrl));
      }
      return; // let NextAuth handle the redirect
    }
  }
  return; // allow
});

export const config = {
  matcher: [
    "/dashboard/:path*",
    "/agents/:path*",
    "/audit/:path*",
    "/capsules/:path*",
    "/policy/:path*",
    "/admin/:path*",
  ],
};
