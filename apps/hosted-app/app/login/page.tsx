import { signIn } from "@/auth";

export default function LoginPage() {
  return (
    <main>
      <section style={{ maxWidth: 480, margin: "5em auto", textAlign: "center" }}>
        <h1 style={{ marginBottom: "1em" }}>Sign in to SBO3L</h1>
        <p style={{ color: "var(--muted)", marginBottom: "2em" }}>
          GitHub OAuth — no extra account needed. We never read your private repos.
        </p>
        <form
          action={async () => {
            "use server";
            await signIn("github", { redirectTo: "/dashboard" });
          }}
        >
          <button type="submit">Sign in with GitHub</button>
        </form>
      </section>
    </main>
  );
}
