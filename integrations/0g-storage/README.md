# @sbo3l/0g-storage

Lightweight TypeScript client for **0G Storage** with a 5-second per-attempt timeout, automatic retry with backoff, and a manual-fallback URL when the testnet's having a bad day.

Built because the official `@0glabs/0g-ts-sdk` ships with native bindings, a heavy dep tree, and no per-attempt timeout — calling it directly hangs the demo loop for tens of seconds before surfacing a transport error. This package speaks the indexer's HTTP API directly (POST `/file/upload`), so no native deps; runs in Node, edge runtimes, and browsers.

## Install

```bash
npm install @sbo3l/0g-storage
```

## Usage in 5 lines

```ts
import { ZeroGStorageClient } from "@sbo3l/0g-storage";

const client = new ZeroGStorageClient(); // defaults to Galileo testnet indexer
const { rootHash, manifest } = await client.upload(payload);
console.log(rootHash);          // 0xc0ffee...
console.log(manifest.permalink); // https://storagescan-galileo.0g.ai/file/0xc0ffee...
```

## What you get back

```ts
interface UploadResult {
  rootHash: string;          // indexer-assigned content hash
  manifest: {
    rootHash: string;
    uploaded_at: string;     // RFC3339 timestamp
    endpoint: string;        // indexer URL the upload landed on
    signer_pubkey: string;   // empty when unsigned
    signature: string;       // empty when unsigned
    permalink: string;       // storagescan-galileo file URL
  };
}
```

## Signed manifests

Pass a signer to get an Ed25519-signed manifest over `<rootHash>|<uploaded_at>|<endpoint>` — useful when you want a downstream consumer to verify the upload was witnessed by your key without having to trust the indexer's reply alone.

```ts
import { ZeroGStorageClient } from "@sbo3l/0g-storage";
import { sign } from "@noble/ed25519";

const client = new ZeroGStorageClient();
const { manifest } = await client.upload(payload, {
  signer: {
    publicKey: pubkeyHex,
    sign: (msg) => sign(msg, privKey),
  },
});
// manifest.signature = "0x..." (128 hex chars = 64-byte Ed25519 sig)
```

The package itself doesn't pin a specific Ed25519 implementation — bring your own (`@noble/ed25519`, `tweetnacl`, the SBO3L core signer, etc.). The `sign` callback returns a 64-byte signature; the wrapper hex-encodes it.

## Browser-side liveness probe

```ts
const probe = await client.probe();
if (!probe.live) {
  // 0G testnet flake. Open the manual fallback in a new tab.
  window.open(client.getFallbackUrl(), "_blank");
}
```

Same pattern the SBO3L marketing site's `ZeroGUploader` uses. The fallback URL points at `storagescan-galileo.0g.ai/tool` where users can drop the same file manually and copy back the rootHash.

## Per-call options

```ts
await client.upload(payload, {
  chunkSize: 2 * 1024 * 1024,    // hint to indexer (default 1 MiB)
  replicationFactor: 5,          // hint to indexer (default 3)
  signer,                        // optional Ed25519 signer (see above)
});
```

## Constructor options

```ts
new ZeroGStorageClient({
  endpoint: "https://your-indexer.example.com",  // override testnet default
  timeoutMs: 5000,                               // per-attempt timeout
  retryDelaysMs: [1000, 3000],                   // 3 attempts total
  fetch: customFetchImpl,                        // override globalThis.fetch
});
```

## Worst-case timing

With defaults (5s timeout, 1s/3s backoff): `5 + 1 + 5 + 3 + 5 = 19s` from `upload()` call to the `ZeroGStorageError` rejection. The error carries `kind`, `attempts`, and `fallbackUrl` so calling code can branch cleanly:

```ts
import { ZeroGStorageError } from "@sbo3l/0g-storage";

try {
  await client.upload(payload);
} catch (err) {
  if (err instanceof ZeroGStorageError) {
    console.error(`tried ${err.attempts}× — fall back to ${err.fallbackUrl}`);
  }
}
```

## Why no `@0glabs/0g-ts-sdk` peer dep?

We tried it. The SDK works when the testnet is healthy, but it has no per-attempt timeout and no graceful fallback path — once the upload starts the only way out is an OS-level connection drop (~30s on most platforms). For a hackathon demo loop where every second between "click upload" and "see rootHash" is visible to a judge, that's unacceptable. This package speaks the same HTTP API the SDK ultimately uses, with the timeout + retry + fallback budget you actually want in production.

## License

MIT. Part of the [SBO3L](https://sbo3l.dev) project.
