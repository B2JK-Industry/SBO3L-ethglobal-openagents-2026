# `@sbo3l/ccip-gateway`

ENSIP-25 / EIP-3668 CCIP-Read gateway for off-chain SBO3L `sbo3l:*` ENS
text records.

**Status:** pre-scaffold for T-4-1. Endpoint shape, error envelopes,
CORS headers and Vercel project layout are pinned. The actual record
lookup + signing logic ship in the T-4-1 main PR.

**Live:** [`https://sbo3l-ccip.vercel.app`](https://sbo3l-ccip.vercel.app)
(once Daniel deploys; not yet live).

## What it does

When a client (viem, ethers.js, or any ENSIP-10-aware library) resolves
an SBO3L agent name whose resolver is the OffchainResolver contract,
the resolver reverts with `OffchainLookup(...)`. The client picks a URL
from the revert payload and `GET`s this gateway. The gateway returns
the requested record, signed by `GATEWAY_PRIVATE_KEY`, and the
OffchainResolver's callback verifies the signature on-chain.

```
                     viem.getEnsText({name, key})
                              │
                              ▼
                      (resolver contract reverts)
                              │
                              ▼
              GET https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json
                              │
                              ▼
                     {"data":"0x...","ttl":60}
                              │
                              ▼
                  resolver.callback(data) verifies sig
                              │
                              ▼
                       returns "87"
```

## Endpoint

```
GET /api/{sender}/{data}.json
```

| Param      | Format                                   |
|------------|------------------------------------------|
| `{sender}` | `0x` + 40-hex-char OffchainResolver addr |
| `{data}`   | `0x` + ABI-encoded calldata + `.json`    |

### Response (200)

```json
{
  "data": "0x...",
  "ttl": 60
}
```

`data` is the ABI-encoded `(bytes value, uint64 expires, bytes
signature)` tuple. `ttl` is a hint for clients/proxies.

### Errors

| HTTP | `error` field    | Cause                                          |
|------|------------------|------------------------------------------------|
| 400  | `bad_request`    | malformed `{sender}` or `{data}`               |
| 404  | `not_found`      | sender not whitelisted, or record absent       |
| 500  | `signing_failed` | `GATEWAY_PRIVATE_KEY` missing / signer error   |
| 501  | `not_implemented`| pre-scaffold stub (current state)              |

## Local development

```bash
cd apps/ccip-gateway
cp .env.example .env.local
# generate a fresh dev key:
node -e "console.log('0x' + require('crypto').randomBytes(32).toString('hex'))"
# paste into .env.local as GATEWAY_PRIVATE_KEY
npm install
npm run dev
# open http://localhost:3000
# api: GET http://localhost:3000/api/0x.../0x....json
```

## Deploy (Vercel)

This directory is its own Vercel project rooted at `apps/ccip-gateway`.
Deploy:

1. Vercel project root: `apps/ccip-gateway`
2. Framework preset: Next.js (auto-detected via `vercel.json`)
3. Project env: set `GATEWAY_PRIVATE_KEY` to a fresh secp256k1 key
   (never reused with any wallet that holds funds).
4. The OffchainResolver contract on-chain MUST point at this gateway's
   public URL in its `urls` array.

## Security notes

1. `GATEWAY_PRIVATE_KEY` is a **read-side signing key**. Compromise =
   wrong record served, no fund loss. Rotation = redeploy
   OffchainResolver pointing at the new address.
2. CORS is wide open (`Access-Control-Allow-Origin: *`) — judges can
   evaluate from a browser console.
3. Cache headers are short (10s + 30s SWR) to keep records fresh.
   Reputation in particular is expected to update per checkpoint.
4. The endpoint never reflects user input verbatim back into HTML;
   only into ABI-encoded bytes that are signed and verified
   on-chain.

## References

- [EIP-3668 CCIP-Read](https://eips.ethereum.org/EIPS/eip-3668)
- [ENSIP-10](https://docs.ens.domains/ensip/10)
- [ENS Labs offchain-resolver reference](https://github.com/ensdomains/offchain-resolver)
- [T-4-1 design doc](../../docs/design/T-4-1-ccip-read-prep.md)
