import NextAuth from "next-auth";
import { authConfig } from "@/auth.config";
import type { Role } from "@/lib/roles";

// NextAuth v5 (Auth.js) runtime export. Provider list + callbacks live
// in auth.config.ts so they can be imported from middleware (Edge
// runtime) without dragging in the full NextAuth instance.
export const { handlers, signIn, signOut, auth } = NextAuth(authConfig);

declare module "next-auth" {
  interface Session {
    user: {
      githubLogin?: string;
      role?: Role;
    } & NonNullable<unknown>;
  }
}

declare module "next-auth/jwt" {
  interface JWT {
    githubLogin?: string;
    role?: Role;
  }
}
