# ENS DNS gateway — deploy guide

**Status:** codec wired (R11 P3). Domain wiring + Vercel deploy
gated on operator decision.
**Source:** [`apps/ens-dns-gateway/`](.)
**Companion:** [`apps/ccip-gateway/DEPLOY.md`](../ccip-gateway/DEPLOY.md) — same
patterns, different gateway.

## What this gateway does

A DNS-over-HTTPS bridge from legacy DNS clients to SBO3L ENS agent
identity records. Clients pointed at the deploy URL as their DoH
upstream get:

- `TXT <agent>.sbo3lagent.eth` → every `sbo3l:*` record formatted as
  RFC-style `k=v` tokens.
- `A`/`AAAA <agent>.sbo3lagent.eth` → resolves the agent's
  `sbo3l:endpoint` URL host through the public upstream.
- Anything else → forwarded transparently to upstream.

## Why this scaffold (not a finished gateway)

Three reasons to ship the scaffold rather than a finished gateway:

1. **Domain wiring is operator-gated.** A real deploy needs a
   public hostname (e.g. `sbo3l-ens-dns.sbo3l.dev`) and a Vercel
   project provisioned under the operator's account. Both are
   Daniel-side decisions; the scaffold lands so the implementation
   path is unblocked but doesn't fork on operator details.

2. **The DNS-over-HTTPS wire codec is non-trivial.** RFC 8484
   binary encoding/decoding is a known-quantity problem
   (`dns-packet` solves it on Node, ~150KB transitive); the
   scaffold flags the integration point clearly so the operator
   isn't surprised by what's missing on first deploy.

3. **It's parallel to `apps/ccip-gateway/`.** Same Vercel/Next.js
   shape, same env-var pattern, same DEPLOY.md structure — an
   operator who already deployed the CCIP-Read gateway can stand
   this one up by mirroring the steps.

## R11 P3: codec finished

The codec is now wired (R11 P3). The route at `/dns-query`:

- Accepts GET (\`?dns=<base64url>\`) and POST (\`Content-Type:
  application/dns-message\`) DoH transports per RFC 8484.
- Decodes the binary message via \`dns-packet\`, dispatches to
  \`resolveTxt\` / \`resolveAddress\` based on the question type, and
  encodes the response back to binary.
- Returns NOERROR + empty answers for non-\`.eth\` names (clients
  use their regular DoH upstream), SERVFAIL for RPC errors, NOERROR
  with answers for resolved records.
- \`Cache-Control: public, max-age=300, stale-while-revalidate=3600\`
  per the round 11 spec (5-minute fresh, 1-hour stale).

Resolution logic at \`src/lib/dns-resolve.ts\` is fully tested with
16 vitest cases covering 5 mock ENS names + edge cases (missing
endpoint, IPv6, partial records, non-ENS rejection, trailing-dot
normalisation).

Run tests:

```bash
cd apps/ens-dns-gateway
npm install
npm test
```

## Operator-side: 2 things to wire pre-deploy

### 1. DNS-message codec (DONE — kept here for reference)

```bash
cd apps/ens-dns-gateway
npm install dns-packet
```

In `src/app/api/dns-query/route.ts`, replace the 501 stub with:

```ts
import dnsPacket from 'dns-packet';
import { resolveTxt, resolveAddress } from '@/lib/dns-resolve';

export async function POST(request: Request) {
  const body = Buffer.from(await request.arrayBuffer());
  const query = dnsPacket.decode(body);
  const question = query.questions?.[0];
  if (!question) return new Response('no question', { status: 400 });

  const opts = {
    rpcUrl: process.env.SBO3L_ENS_RPC_URL ?? 'https://ethereum-rpc.publicnode.com',
    network: 'mainnet' as const,
    upstreamDohUrl:
      process.env.SBO3L_DNS_UPSTREAM_DOH_URL ?? 'https://cloudflare-dns.com/dns-query',
  };

  let answers: dnsPacket.Answer[] = [];
  if (question.type === 'TXT') {
    const records = await resolveTxt(question.name, opts);
    answers = records.map((r) => ({
      name: r.name,
      type: 'TXT',
      ttl: r.ttl,
      data: r.value,
    }));
  } else if (question.type === 'A' || question.type === 'AAAA') {
    const records = await resolveAddress(question.name, question.type, opts);
    answers = records.map((r) => ({
      name: r.name,
      type: question.type,
      ttl: r.ttl,
      data: r.value,
    }));
  }

  const response = dnsPacket.encode({
    type: 'response',
    id: query.id,
    flags: dnsPacket.RECURSION_DESIRED | dnsPacket.RECURSION_AVAILABLE,
    questions: query.questions,
    answers,
  });

  return new Response(response, {
    headers: { 'Content-Type': 'application/dns-message' },
  });
}
```

GET (DoH §4.1.1) is the same shape with `Buffer.from(searchParams.dns, 'base64url')`.

### 2. Env vars on the Vercel project

| Var | Default | Purpose |
|---|---|---|
| `SBO3L_ENS_RPC_URL` | PublicNode mainnet | Mainnet RPC for ENS lookups |
| `SBO3L_DNS_UPSTREAM_DOH_URL` | Cloudflare 1.1.1.1 DoH-JSON | Where non-ENS queries forward |

Set via `vercel env add` or the Vercel dashboard.

### 3. Custom domain

In the Vercel dashboard:

1. Project → Settings → Domains → add the public hostname (e.g.
   `sbo3l-ens-dns.sbo3l.dev`).
2. Configure the DNS at the apex domain to point at Vercel:
   `CNAME sbo3l-ens-dns sbo3l-ens-dns-gateway.vercel.app`.
3. Vercel auto-provisions TLS via Let's Encrypt; takes ~5 minutes.

Once live:

```bash
# Test from curl with DoH support
curl -H 'Accept: application/dns-message' \
     'https://sbo3l-ens-dns.sbo3l.dev/dns-query?dns=<base64url-binary>' \
     -o response.bin
```

Or point Firefox at it:

```
about:config →
  network.trr.mode = 2
  network.trr.uri = https://sbo3l-ens-dns.sbo3l.dev/dns-query
```

## What this gateway is NOT

- **Not DNSSEC-signed.** Synthetic responses are not signed with a
  DNSSEC key. Clients that require DNSSEC will reject them.
  Adding signing requires a long-lived signing key + key
  management — out of scope for the scaffold.
- **Not DoT (DNS-over-TLS).** DoH is the simpler transport for
  serverless deploys (Vercel doesn't natively support TCP/853).
  DoT is a separate gateway if needed.
- **Not censorship-resistant.** Vercel can take the deploy down.
  This gateway is a *bridge*, not a sovereign resolver.
- **Not a replacement for ENS-aware clients.** Tools that already
  speak ENS (viem, ethers.js, MetaMask) should keep speaking ENS
  directly; this gateway exists for clients that *don't* and
  can't be upgraded.

## Status

Scaffold ships in this PR. Domain wiring + codec integration land
once Daniel chooses a deploy domain and an operator picks up the
~1-day finishing work. The `/dns-query` route currently returns
501 with a clear "scaffold incomplete" JSON error so a deploy in
its current state doesn't pretend to resolve.

## Coordinate with operator

- Decide the public hostname (`sbo3l-ens-dns.sbo3l.dev` is the
  obvious match to the existing CCIP gateway naming).
- Provision the Vercel project from `apps/ens-dns-gateway/`.
- Land the codec PR (`npm install dns-packet` + the route
  implementation above).
- Add to `docs/submission/live-url-inventory.md` once live.
