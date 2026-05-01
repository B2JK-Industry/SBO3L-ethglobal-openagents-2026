import Link from "next/link";
import { auth } from "@/auth";

export default async function HomePage() {
  const session = await auth();

  return (
    <main>
      <section style={{ maxWidth: 720, margin: "4em auto" }}>
        <h1 style={{ fontSize: "var(--fs-4)", marginBottom: "0.5em" }}>
          SBO3L hosted preview
        </h1>
        <p style={{ color: "var(--muted)", fontSize: "var(--fs-2)", marginBottom: "1.5em" }}>
          Free-tier SBO3L in your browser. Login with GitHub, get a daemon, sign
          your first APRP envelope. Self-host any time.
        </p>
        {session?.user ? (
          <Link href="/dashboard">
            <button>Open dashboard →</button>
          </Link>
        ) : (
          <Link href="/login">
            <button>Sign in with GitHub →</button>
          </Link>
        )}
        <p style={{ color: "var(--muted)", marginTop: "2em", fontSize: "0.9em" }}>
          This is the prep slice of CTI-3-4. Live agent feed, audit explorer, and
          capsule library land in the main PR. See{" "}
          <a href="https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/issues/92">
            issue #92
          </a>
          .
        </p>
      </section>
    </main>
  );
}
