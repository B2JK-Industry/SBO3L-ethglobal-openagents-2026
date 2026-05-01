import Link from "next/link";
import { auth } from "@/auth";

export default async function ForbiddenPage() {
  const session = await auth();
  const role = session?.user?.role ?? "viewer";

  return (
    <main>
      <section style={{ maxWidth: 560, margin: "5em auto", textAlign: "center" }}>
        <h1 style={{ fontSize: "var(--fs-3)", marginBottom: "0.5em" }}>403 — Forbidden</h1>
        <p style={{ color: "var(--muted)", marginBottom: "1.5em" }}>
          Your account is signed in as <code>{role}</code>. The page you tried to open requires a higher role.
        </p>
        <p style={{ color: "var(--muted)", fontSize: "0.9em", marginBottom: "2em" }}>
          Operators and admins are added by env config (<code>OPERATOR_GITHUB_LOGINS</code>, <code>ADMIN_EMAILS</code>, etc.). Ask the daemon owner to grant the role you need, or sign in with a different account.
        </p>
        <Link href="/dashboard">
          <button>← Back to dashboard</button>
        </Link>
      </section>
    </main>
  );
}
