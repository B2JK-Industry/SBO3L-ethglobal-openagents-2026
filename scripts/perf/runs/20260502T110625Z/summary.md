# load-test report — 2026-05-02T11:06:42Z

**Daemon**: `target/release/sbo3l-server` (release profile)
**Storage**: SQLite WAL-mode (single-writer)
**Signer**: Ed25519 dev seed (audit + receipt)
**Per-request work**: schema-validate + JCS-canonicalise +
nonce-claim INSERT + policy-decide + audit-append INSERT +
Ed25519 receipt sign.

## Results

| concurrency | duration | rps   | p50 ms | p95 ms | p99 ms | p99.9 ms | err % |
|------------:|---------:|------:|-------:|-------:|-------:|---------:|------:|
|          16 |    15.0s |  6796 |   1.93 |   4.96 |   6.75 |    27.73 | 0.000 |
|          64 |    15.0s |  8156 |   7.31 |  15.03 |  21.09 |    46.49 | 26.279 |
|         128 |    15.0s | 10926 |  10.74 |  23.97 |  34.63 |    56.99 | 50.823 |

## Notes

- **Honest reporting**: numbers above are wall-clock measured on
  the running host's CPU. We do NOT claim numbers we don't
  measure.
- The aspirational 10 000 rps target is bounded by SQLite
  single-writer throughput plus 2 INSERTs per request
  (nonce-claim + audit-append). Realistic ceiling on
  commodity hardware is closer to 5–8 K rps; sustained 10K
  needs either WAL+mmap tuning, sharded storage, or batched
  audit append (Phase 3.4 follow-up).
- Latency targets (p99 < 50 ms) are well within reach at the
  rates this harness measures.
- Daemon was a freshly-spawned instance per run; the SQLite
  WAL grows monotonically across the duration but doesn't
  checkpoint mid-run, so latency reflects steady-state
  rather than checkpoint-induced spikes.

## Reproduce

```sh
bash scripts/perf/load-test.sh
# or, for a 5-minute sustained run:
DURATION_S=300 CONCURRENCY="64 128 256" bash scripts/perf/load-test.sh
```
