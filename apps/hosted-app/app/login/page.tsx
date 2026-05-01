import { signIn } from "@/auth";

interface ProviderButton {
  id: "github" | "google" | "apple";
  label: string;
  envFlag: string;
}

const PROVIDERS: ProviderButton[] = [
  { id: "github", label: "Continue with GitHub", envFlag: "AUTH_GITHUB_ID" },
  { id: "google", label: "Continue with Google", envFlag: "AUTH_GOOGLE_ID" },
  { id: "apple",  label: "Continue with Apple",  envFlag: "AUTH_APPLE_ID" },
];

export default function LoginPage() {
  const enabled = PROVIDERS.filter((p) => Boolean(process.env[p.envFlag]));

  return (
    <main>
      <section style={{ maxWidth: 480, margin: "5em auto", textAlign: "center" }}>
        <h1 style={{ marginBottom: "0.6em" }}>Sign in to SBO3L</h1>
        <p style={{ color: "var(--muted)", marginBottom: "2em" }}>
          Pick any provider. Your account never holds an SBO3L signing key — every action is policy-checked + signed by the daemon.
        </p>

        {enabled.length === 0 && (
          <p style={{ color: "#ff6b6b", marginBottom: "1.5em" }}>
            No identity providers configured. Set <code>AUTH_GITHUB_ID</code> + <code>AUTH_GITHUB_SECRET</code> (or Google / Apple equivalents) in <code>.env.local</code> and restart.
          </p>
        )}

        <div style={{ display: "grid", gap: "0.7em" }}>
          {enabled.map((p) => (
            <form
              action={async () => {
                "use server";
                await signIn(p.id, { redirectTo: "/dashboard" });
              }}
            >
              <button type="submit" style={{ width: "100%" }}>{p.label}</button>
            </form>
          ))}
        </div>

        <p style={{ color: "var(--muted)", fontSize: "0.85em", marginTop: "2em" }}>
          Roles are assigned by the daemon owner via env config (see <code>.env.example</code>). New users default to <code>viewer</code>.
        </p>
      </section>
    </main>
  );
}
