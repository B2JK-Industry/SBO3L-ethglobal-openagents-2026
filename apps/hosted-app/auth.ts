import NextAuth from "next-auth";
import GitHub from "next-auth/providers/github";

// NextAuth v5 (Auth.js) configuration.
//
// Single GitHub provider — agent developer audience already lives there;
// they don't need yet another account. Sessions are JWT-backed (no DB)
// for prep; main PR adds a DB adapter once Postgres lands in Grace's
// Fly.io deploy.
export const { handlers, signIn, signOut, auth } = NextAuth({
  providers: [GitHub],
  pages: {
    signIn: "/login",
  },
  callbacks: {
    async jwt({ token, profile }) {
      if (profile?.login) {
        token.githubLogin = profile.login;
      }
      return token;
    },
    async session({ session, token }) {
      if (typeof token.githubLogin === "string") {
        session.user.githubLogin = token.githubLogin;
      }
      return session;
    },
  },
});

declare module "next-auth" {
  interface Session {
    user: {
      githubLogin?: string;
    } & NonNullable<unknown>;
  }
}

declare module "next-auth/jwt" {
  interface JWT {
    githubLogin?: string;
  }
}
