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
      if (profile?.login && typeof profile.login === "string") {
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

// Augment NextAuth's Session.user with our GitHub login claim. Keep base
// fields (name/email/image) reachable via DefaultSession. NextAuth v5 keeps
// the `next-auth/jwt` path through @auth/core; pre-installed in the app.
declare module "next-auth" {
  interface Session {
    user: {
      githubLogin?: string;
      name?: string | null;
      email?: string | null;
      image?: string | null;
    };
  }
}

declare module "@auth/core/jwt" {
  interface JWT {
    githubLogin?: string;
  }
}
