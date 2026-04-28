# `mandate audit checkpoint` — mock-anchored audit checkpoints

> *Mock anchoring, not real onchain anchoring.*

`mandate audit checkpoint {create, verify}` (PSM-A4) gives an operator a way to **snapshot the audit chain's tip** and stamp it with a deterministic local "anchor reference" that simulates the *shape* of an on-chain anchor. Every operation is local — there is no network, no chain RPC, no broadcast, no signature from any L1 / L2 contract. The CLI prefixes every output line with `mock-anchor:` for loud disclosure.

## What it is, and what it is not

|   | This CLI (PSM-A4) | A real onchain checkpoint |
| --- | --- | --- |
| Anchor target | A deterministic 16-hex local id (`local-mock-anchor-<…>`) derived from the checkpoint content | A Merkle root committed to an L2 contract or an Ethereum tx hash, broadcast to a public chain |
| Network | None — fully offline | Yes — chain RPC, gas, finality |
| Replay protection | `mock_anchor_ref` is UNIQUE in `audit_checkpoints` (V007) | Onchain finality + `block.number`/timestamp |
| Re-verifiability offline | Yes — `verify <file>` reads the JSON artifact | Requires chain access for the contract read |
| Production-ready? | **No.** This is the operational *shape* of a checkpoint, not a chain commitment. |

The CLI loudly refuses to verify a checkpoint with `mock_anchor: false` — there is no path through this code that produces or accepts a real-onchain claim. PSM-B will integrate a real anchoring backend when the time comes.

## Subcommands

### `mandate audit checkpoint create --db <path> [--out <file>]`

Reads the audit chain from `<path>`, computes a `chain_digest` (SHA-256 over every event_hash in the prefix), inserts a row into `audit_checkpoints`, and prints the result. With `--out`, the same JSON is also written to disk for offline distribution.

```text
$ mandate audit checkpoint create --db /var/mandate/mandate.db --out cp.json
mock-anchor: schema:            mandate.audit_checkpoint.v1
mock-anchor: sequence:          42
mock-anchor: latest_event_id:   evt-01KQATRMHXFWY58QQZV378JN9P
mock-anchor: latest_event_hash: 6cba2eed67c2dfd623521be0a692b8716f300eb27deb3a7e9ab06d5e8b3bb9e6
mock-anchor: chain_digest:      ed00a7f7d5caed85960dfb815d079531e6fd2f2019e61c655e5d156e5db0708a
mock-anchor: mock_anchor_ref:   local-mock-anchor-9202d6bc7b751225
mock-anchor: created_at:        2026-04-28T19:58:54.156053+00:00
mock-anchor: explanation:       Local mock anchor; not a real onchain anchor. See docs/cli/audit-checkpoint.md.
mock-anchor: written to cp.json
```

| Exit code | Meaning |
| --- | --- |
| 0 | Checkpoint created; row written; (optionally) JSON file written |
| 1 | DB open / read / write failure |
| 3 | Audit chain is empty — nothing to anchor (the honest "no chain to commit" path) |

### `mandate audit checkpoint verify <file> [--db <path>]`

Verifies a checkpoint JSON artifact. Without `--db`, only structural checks run: schema id, `mock_anchor: true`, hash field shapes, `mock_anchor_ref` format. With `--db`, the verifier additionally:

1. Re-derives the `chain_digest` from the live audit chain.
2. Looks up the checkpoint row in `audit_checkpoints` by `mock_anchor_ref`.
3. Confirms the persisted row's `chain_digest` and `latest_event_hash` match the artifact.
4. If the live chain has grown past the checkpoint's `sequence`, surfaces this as informational (`live chain has advanced beyond checkpoint`) — the prefix-through-doc-seq still has to match, otherwise it's a tamper signal.

```text
$ mandate audit checkpoint verify cp.json --db /var/mandate/mandate.db
mock-anchor: schema:            mandate.audit_checkpoint.v1
mock-anchor: mock_anchor:       true
mock-anchor: sequence:          42
…
mock-anchor: structural verify: ok
mock-anchor: db cross-check:    ok (chain_digest, latest_event_hash, anchor row all match)
mock-anchor: verify result:     ok
```

| Exit code | Meaning |
| --- | --- |
| 0 | Verified |
| 1 | IO / parse / DB error |
| 2 | Verification failed (tampered, wrong DB, missing row, bad schema, `mock_anchor: false`, ...) |

The verifier explicitly distinguishes "checkpoint over a stale prefix" (informational) from "checkpoint over a tampered prefix" (exit 2) by re-deriving the digest of the chain prefix through the checkpoint's `sequence`.

## Storage shape

The `audit_checkpoints` table (V007) holds one row per checkpoint:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | `INTEGER PRIMARY KEY AUTOINCREMENT` | Monotonic per-DB checkpoint id. |
| `sequence` | `INTEGER NOT NULL` | Highest `audit_events.seq` covered by the checkpoint. |
| `latest_event_id` | `TEXT NOT NULL` | Chain tip's `id` at creation. |
| `latest_event_hash` | `TEXT NOT NULL` | 64-hex SHA-256 of the chain tip's canonical JSON. |
| `chain_digest` | `TEXT NOT NULL` | 64-hex SHA-256 over every `event_hash` in the prefix (in seq order). |
| `mock_anchor_ref` | `TEXT NOT NULL UNIQUE` | `local-mock-anchor-<16 hex>`; `<16 hex>` is the first 8 bytes of `SHA-256(chain_digest ‖ sequence ‖ created_at)`. |
| `created_at` | `TEXT NOT NULL` | RFC3339 timestamp of checkpoint creation. |

Indexes on `sequence` and `chain_digest` so a future verifier can locate "the latest checkpoint covering seq N" or "any checkpoint with this digest" in O(log n).

## Doctor integration

`mandate doctor` surfaces this table as the `audit_checkpoints` row:

| State | Doctor row |
| --- | --- |
| Table present + at least one row | `ok` — `table present, rows=N, latest=seq<X>, anchor=<12-hex-prefix>… (mock — see docs/cli/audit-checkpoint.md)` |
| Table present + no rows | `skip` — points at `mandate audit checkpoint create` and explicitly mentions "PSM-A4 — mock anchoring, not onchain" |
| Table missing entirely | `skip` — older daemon DB before V007; references PSM-A4 + V007 |

## Truthfulness rules

1. The string `mock-anchor:` is the prefix on every CLI output line (parallel to `mock-kms:` in PSM-A1.9). A copy-pasted single line cannot be misread as production-anchor output.
2. The JSON artifact carries `mock_anchor: true` and an `explanation` field that points at this doc. The verifier refuses any artifact with `mock_anchor: false` (exit 2).
3. The mock anchor reference is **deterministic** — same content + same `created_at` produces the same ref, and the storage layer's UNIQUE constraint refuses duplicates. There is no global "anchor service"; each DB is its own ledger.
4. The chain digest is computed over the **canonical event_hash bytes** (not their hex encoding). Anyone with the same chain prefix can re-derive the digest exactly.
5. The doctor's `ok` row carries only the first 12 hex chars of the anchor ref tail. Full ref via storage list or via the JSON artifact.

## Out of scope

- Real onchain anchoring (broadcast to L2, Ethereum, etc.). PSM-B will wire a real backend.
- Checkpoint signing. Today the row is "whoever runs the CLI on this DB" — no signature on the checkpoint itself; the underlying audit chain's per-event signatures still verify independently.
- Pruning / Merkle-tree variants. The current digest is a flat SHA-256 fold over all event_hashes. A Merkle root is a natural future extension.
- Cross-daemon anchoring. Each DB is its own ledger; there is no notion of "anchor seq N from daemon A in daemon B's ledger".
