# `sbo3l audit export` — backend selection

> *Companion to [`audit-bundle.md`](./audit-bundle.md). That doc covers the bundle's content and verification contract; this doc covers `--backend`, which controls only **where** the bundle is published.*

`sbo3l audit export` builds a self-contained, machine-verifiable bundle from a signed policy receipt + the audit chain prefix. By default, the bundle is written to disk (or stdout). The `--backend` flag selects a remote publishing target instead.

## Backends

| Backend | Default? | What it does |
|---|---|---|
| `local` | yes | Writes the bundle JSON to `--out` (or stdout). Identical to pre-Task-C behaviour. |
| `0g-storage` | no | Uploads the bundle to the [0G Storage Galileo testnet indexer](https://docs.0g.ai/) and embeds the returned `rootHash` in an envelope. |

### `--backend local` (default)

Unchanged. `sbo3l audit export --receipt … --db … --out bundle.json` continues to write the same `sbo3l.audit_bundle.v1` JSON it always did. No env vars consulted, no network access. **All existing scripts continue to work.**

### `--backend 0g-storage`

```bash
sbo3l audit export \
  --receipt        path/to/receipt.json \
  --db             path/to/sbo3l.sqlite \
  --receipt-pubkey <hex> \
  --audit-pubkey   <hex> \
  --backend        0g-storage \
  --out            path/to/envelope.json
```

What this does:

1. Builds the same `sbo3l.audit_bundle.v1` JSON the local backend would have produced.
2. POSTs the bundle bytes to the 0G Storage indexer's `/file/upload` endpoint.
3. Captures the `rootHash` the indexer returns and wraps the bundle in an "export envelope":
   ```json
   {
     "bundle":          { /* unmodified sbo3l.audit_bundle.v1 */ },
     "live_evidence": {
       "backend":     "0g-storage",
       "root_hash":   "0x…",
       "uploaded_at": "2026-05-02T12:34:56Z",
       "indexer_url": "https://indexer-storage-testnet-turbo.0g.ai"
     }
   }
   ```
4. Writes the envelope to `--out` (or stdout when omitted).
5. Prints `rootHash=0x…` on its own line so shell pipelines can capture it without parsing the envelope:
   ```sh
   ROOT=$(sbo3l audit export … --backend 0g-storage --out env.json | grep ^rootHash= | cut -d= -f2)
   ```

The envelope keeps the bundle field bit-for-bit identical to what `--backend local` produces, so a verifier that fetches the bundle bytes from 0G by their `rootHash` can re-run `sbo3l audit verify-bundle` directly.

#### Schema note (honest discrepancy)

The original Task C brief asked for `live_evidence` to be added under the existing `AuditBundle`. We carry it on a wrapping envelope instead because `AuditBundle` is `#[serde(deny_unknown_fields)]` and its `sbo3l.audit_bundle.v1` schema id is referenced from external schemas + signed evidence — adding a field would have required a v2 schema bump. Wrapping preserves bundle v1 and keeps `audit verify-bundle` backwards-compatible. Future v2 of the bundle could absorb `live_evidence` natively if it becomes a first-class part of every export.

## Configuration

| Knob | Where set | Default |
|---|---|---|
| Indexer URL | `--zerog-indexer-url <url>` flag, then `SBO3L_ZEROG_INDEXER_URL` env var | `https://indexer-storage-testnet-turbo.0g.ai` |
| Retry policy | (compile-time const, not user-tunable) | 3 attempts with 1s / 3s backoff between attempts |
| Per-request timeout | (compile-time, not user-tunable) | 30 s |

## Testnet flakiness — be honest about it

0G Galileo testnet is **documented-flaky**: the SDK times out, KV nodes intermittently disappear, the faucet has been seen off for hours. This is a testnet, not production infra; do not build runbooks that assume `audit export --backend 0g-storage` always succeeds.

The CLI gives you three layers of recourse before failing:

1. **Retry-with-backoff** (built-in, automatic). Three attempts at 1s / 3s exponential backoff. Worst-case wall-clock ~5 seconds before the operator sees an error. Transient hiccups recover automatically.
2. **Clear error pointing at the browser fallback.** When all attempts fail the CLI prints:
   ```
   0g-storage upload failed: 0G upload failed after 3 attempt(s): <last error>.
   Testnet is documented-flaky; fall back to the browser tool at
   https://storagescan-galileo.0g.ai/tool
   ```
   The browser tool accepts any payload and produces a `rootHash` you can paste back into the envelope manually.
3. **`--backend local` always works.** If you cannot reach 0G Storage at all, drop the `--backend 0g-storage` flag and ship the local bundle. No information loss — the bundle JSON is what carries the cryptographic proof; the 0G upload is just publishing infrastructure.

## Exit codes

Same as the rest of `sbo3l audit export`:

| Code | Meaning |
|---|---|
| 0 | Bundle built (and uploaded, when `--backend 0g-storage`). Envelope written to `--out` / stdout. |
| 1 | Bundle build failed *or* 0G upload failed after retries. |
| 2 | I/O error reading the receipt / chain inputs, or serialisation error. |

## Live test

A live integration test (gated behind `ZEROG_TESTNET_LIVE=1`) exercises the real Galileo indexer:

```sh
ZEROG_TESTNET_LIVE=1 cargo test -p sbo3l-storage \
  zerog_backend::tests::live_testnet_upload -- --nocapture
```

Skipped in CI on purpose: a flaky upstream is not allowed to red-light the build.
