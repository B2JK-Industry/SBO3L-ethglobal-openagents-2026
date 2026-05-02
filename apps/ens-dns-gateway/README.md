# SBO3L ENS DNS gateway

DNS-over-HTTPS bridge from legacy DNS clients to SBO3L ENS agent
identity records. Lets a client that speaks RFC 8484 DoH but not
ENS reach `<agent>.sbo3lagent.eth` by resolving the agent's
`sbo3l:endpoint` text record on demand.

**Status:** scaffold. See [`DEPLOY.md`](DEPLOY.md) for the
finish-the-scaffold checklist.

## Quickstart (after the scaffold is finished)

```bash
# Deploy via Vercel
vercel --prod

# Once domain wired:
curl -H 'Accept: application/dns-message' \
     "https://sbo3l-ens-dns.sbo3l.dev/dns-query?dns=$(echo -n '...' | base64url)"
```

## Architecture

```
DoH client (Firefox / curl / Pi-hole)
         │  RFC 8484 DoH binary message
         ▼
/dns-query route (Vercel Edge)
         │  decode message
         ▼
src/lib/dns-resolve.ts
         │ ENS read for *.eth
         │ Cloudflare DoH for everything else
         ▼
DNS message encode + Content-Type: application/dns-message
         │
         ▼
DoH client receives synthetic response
```

## What's in the box

- `src/app/page.tsx` — landing page describing the gateway
- `src/app/api/dns-query/route.ts` — DoH endpoint (currently 501
  scaffold; finish per [`DEPLOY.md`](DEPLOY.md))
- `src/lib/dns-resolve.ts` — ENS resolver logic (TXT + A/AAAA)

## Companion deploys

- [`apps/ccip-gateway/`](../ccip-gateway/) — the CCIP-Read gateway
  (different protocol, same Vercel/Next shape)
- [`apps/marketing/`](../marketing/) — public marketing site
