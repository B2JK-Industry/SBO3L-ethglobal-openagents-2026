import Link from "next/link";
import { fetchKmsStatus } from "./actions";
import { KmsStatusCard } from "./KmsStatusCard";

export const dynamic = "force-dynamic";

export default async function AdminKeysPage() {
  const initial = await fetchKmsStatus();

  return (
    <main>
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: "1em" }}>
        <h1>Signing keys</h1>
        <nav style={{ fontSize: "0.85em", display: "flex", gap: "1em" }}>
          <Link href="/admin/users">Users</Link>
          <Link href="/admin/flags">Flags</Link>
          <Link href="/admin/audit">Audit</Link>
        </nav>
      </header>
      <p style={{ color: "var(--muted)", marginBottom: "1.5em", maxWidth: 760 }}>
        Visibility into the daemon's <code>Signer</code> trait
        implementation — which backend is configured, when it last
        produced a signature, and whether it currently responds. Read-
        only on purpose: production key rotation goes through the KMS
        provider's own console, not this UI.
      </p>

      <KmsStatusCard initial={initial} />

      <aside style={{ marginTop: "2em", padding: "1em 1.2em", background: "var(--code-bg)", border: "1px solid var(--border)", borderLeft: "3px solid var(--accent)", borderRadius: "var(--r-md)", color: "var(--muted)", fontSize: "0.9em" }}>
        <strong style={{ color: "var(--fg)" }}>What's signed:</strong>{" "}
        every PolicyReceipt + every audit-event linkage hash. The agent
        never sees the private key (identity sub-claim #2; demo gate 12
        grep-asserts this every CI run).{" "}
        <a href="https://sbo3l-docs.vercel.app/concepts/signing">Read the signing model →</a>
      </aside>
    </main>
  );
}
