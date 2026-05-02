/**
 * Landing page for the SBO3L ENS DNS gateway.
 *
 * Static page describing what the gateway is, what it isn't, and
 * how to point a DoH-aware client at it. The actual resolution
 * work happens at `/dns-query` (RFC 8484 DNS-over-HTTPS).
 */

export default function Home() {
  return (
    <main
      style={{
        fontFamily: 'system-ui, -apple-system, sans-serif',
        maxWidth: 720,
        margin: '40px auto',
        padding: '0 16px',
        lineHeight: 1.5,
      }}
    >
      <h1>SBO3L ENS DNS gateway</h1>
      <p>
        DNS-over-HTTPS bridge that resolves SBO3L agent names like{' '}
        <code>research-agent.sbo3lagent.eth</code> via standard{' '}
        <a href="https://datatracker.ietf.org/doc/html/rfc8484">RFC 8484</a>{' '}
        DoH queries. Lets a legacy DNS client reach an agent's{' '}
        <code>sbo3l:endpoint</code> without speaking ENS.
      </p>
      <h2>Endpoint</h2>
      <p>
        DoH endpoint: <code>{`https://<your-deploy>.vercel.app/dns-query`}</code>
      </p>
      <p>
        Configure your DoH-aware client (Firefox, curl <code>--doh-url</code>,
        Cloudflare 1.1.1.1, AdGuard, Pi-hole) to use this endpoint as the
        upstream resolver. Queries for <code>*.eth</code> names are resolved
        via the SBO3L agent identity stack; everything else is forwarded to a
        public upstream (default: Cloudflare 1.1.1.1).
      </p>
      <h2>What gets resolved</h2>
      <ul>
        <li>
          <strong>A / AAAA</strong> for an SBO3L agent name → resolved by
          looking up the agent's <code>sbo3l:endpoint</code> text record on
          ENS, then resolving the host part of the URL through the public
          upstream.
        </li>
        <li>
          <strong>TXT</strong> for an SBO3L agent name → returns every{' '}
          <code>sbo3l:*</code> text record formatted as an RFC-style{' '}
          <code>k=v</code> token list.
        </li>
        <li>
          Anything else → forwarded transparently to the upstream resolver.
          The gateway does not block, censor, or log queries beyond Vercel's
          standard request log.
        </li>
      </ul>
      <h2>Out of scope (for the scaffold)</h2>
      <ul>
        <li>DNSSEC signing of the synthetic responses.</li>
        <li>DoT (DNS-over-TLS) — DoH is the simpler deploy target.</li>
        <li>ENS L2 / cross-chain resolution.</li>
        <li>
          Caching beyond Vercel's edge cache (60s public per the{' '}
          <code>vercel.json</code> default).
        </li>
      </ul>
      <h2>Status</h2>
      <p>
        Scaffold. Domain wiring + Vercel deploy gated on operator decision —
        see{' '}
        <a href="https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/apps/ens-dns-gateway/DEPLOY.md">
          DEPLOY.md
        </a>
        .
      </p>
    </main>
  );
}
