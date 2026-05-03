# Changelog

## 1.2.0 — 2026-05-03

Initial release. Versioned at 1.2.0 to match the rest of the
`@sbo3l/*` family (`@sbo3l/sdk`, `@sbo3l/autogen`, etc.) so
`install-smoke` can pin everything at one tag.

- `ZeroGStorageClient` with HTTP-direct upload (no native deps)
- Per-attempt 5s timeout, 1s/3s retry backoff (3 attempts total)
- Browser fallback URL for storagescan-galileo manual-upload tool
- Optional Ed25519 signed manifest emit (`<rootHash>|<uploaded_at>|<endpoint>`)
- `probe()` liveness check honouring the same timeout
- `permalinkFor(rootHash)` builds the storagescan file URL
- `ZeroGStorageError` with `{ kind, attempts, fallbackUrl }`
- Type-safe per-call options: `chunkSize`, `replicationFactor`, `signer`
- 16 vitest tests, all green
