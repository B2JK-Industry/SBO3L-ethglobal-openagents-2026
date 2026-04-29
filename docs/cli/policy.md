# `sbo3l policy` — local active-policy lifecycle

> *Local production-shaped lifecycle, not remote governance.*

`sbo3l policy {validate, current, activate, diff}` (PSM-A3) gives an operator a simple SQLite-backed lifecycle for the policy that the daemon evaluates. Every operation reads or writes the `active_policy` table (V006) on a SBO3L SQLite database. There is no network, no on-chain anchor, no consensus: whoever opens the database can activate a policy.

## What it is, and what it is not

|   | This CLI (PSM-A3) | A "real" policy registry |
| --- | --- | --- |
| Custody | Whoever has filesystem access to the daemon's SQLite DB | Multi-party signed, access-controlled, often on-chain or in a managed registry |
| Activation trust | Local — `activate` is a SQLite write | Often gated by signer-quorum, role-based ACLs, change-management workflows |
| Replay protection | Hash UNIQUE across all rows: re-activating a previously-deactivated hash is refused | Same idea but enforced server-side / on-chain |
| Audit of activation | The `activated_at`/`deactivated_at` timestamps + `source` label in this table | Cryptographically signed activation events, often broadcast |
| Network | None — fully offline | Yes (registry, governance, anchor) |

Concretely: this CLI is what a single operator uses to roll a policy forward on a single SBO3L daemon's storage. It is the production-shape of "the daemon now uses policy X version Y" — not the production-shape of "a multi-party governance flow agreed to activate policy X". Those are separate problems.

## Subcommands

### `sbo3l policy validate <file>`

Parse a candidate policy JSON file, run semantic validation (no duplicate agent IDs, no duplicate budget tuples, etc.), and print the canonical SHA-256 hash plus a small summary.

```text
$ sbo3l policy validate test-corpus/policy/reference_low_risk.json
ok: policy parses + validates
  policy_hash:   e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf
  agents:        1
  rules:         4
  providers:     1
  recipients:    2
  budgets:       3
```

| Exit code | Meaning |
| --- | --- |
| 0 | Policy parses + validates |
| 1 | File read failure (e.g. typo'd path) |
| 2 | Policy is invalid (parse error or semantic check failed) |

### `sbo3l policy current --db <path>`

Print the row of the currently-active policy.

```text
$ sbo3l policy current --db /var/sbo3l/sbo3l.db
active policy:
  version:       v3
  policy_hash:   3f4e…
  source:        operator-cli
  activated_at:  2026-04-28T19:13:04+00:00
```

If no policy has been activated yet, the command prints an honest note and exits with code **3** (not 1) so scripts can branch on "DB exists but empty" without confusing it with a real error:

```text
$ sbo3l policy current --db /tmp/empty.db
no active policy in this db. Run `sbo3l policy activate <file> --db /tmp/empty.db` to seed one.
$ echo $?
3
```

| Exit code | Meaning |
| --- | --- |
| 0 | An active policy is present |
| 1 | DB open / read failure |
| 3 | DB is fine, no policy active yet (the honest no-active path) |

### `sbo3l policy activate <file> --db <path> [--source <label>]`

Validate, hash, and activate a policy file. Idempotent: re-activating the already-active policy is a no-op (no new row is inserted, exit 0). Activating a *different* policy atomically marks the previous active row's `deactivated_at = now()` and inserts the new row at `version+1`.

```text
$ sbo3l policy activate test-corpus/policy/reference_low_risk.json --db /var/sbo3l/sbo3l.db
activated: policy_hash=e044…f event=v1 source=operator-cli

$ sbo3l policy activate test-corpus/policy/reference_low_risk.json --db /var/sbo3l/sbo3l.db
already active: policy_hash=e044…f version=v1 (no-op …)
```

`--source` is a free-form label written verbatim into the row. Default is `operator-cli`. Future surfaces (a sponsor-side activation pipe, an embedded reference seed, etc.) can use distinct labels so the row history is self-explanatory.

| Exit code | Meaning |
| --- | --- |
| 0 | Activated, or already active (idempotent) |
| 1 | DB open / write failure |
| 2 | Policy is invalid |
| 4 | Refused: this hash was activated before, deactivated, and you're trying to re-activate the same bytes. The lifecycle is monotonic — produce a fresh policy (even cosmetically different) instead. |

### `sbo3l policy diff <file-a> <file-b>`

Diff two candidate policy files at the canonical-JSON level. Both files must parse and validate; the output is a small added/removed line list.

```text
$ sbo3l policy diff a.json b.json
policies differ:
  - a.json (policy_hash = e044…)
  + b.json (policy_hash = 7c12…)
  …
+ "agent_id": "variant-agent-x"
  …
```

| Exit code | Meaning |
| --- | --- |
| 0 | Files parsed and were canonically identical |
| 1 | Files parsed and differed (diff is printed) |
| 2 | At least one file failed to parse / validate |

## Storage shape

The `active_policy` table (V006) holds one row per activation:

| Column | Type | Notes |
| --- | --- | --- |
| `version` | `INTEGER PRIMARY KEY` | Monotonic; assigned by the storage layer (`max(version)+1`). |
| `policy_hash` | `TEXT NOT NULL UNIQUE` | Canonical SHA-256 hex of the policy. UNIQUE → previously-seen hashes cannot be re-activated. |
| `policy_json` | `TEXT NOT NULL` | The canonicalised JSON the operator activated. The exact bytes the daemon evaluates. |
| `activated_at` | `TEXT NOT NULL` | RFC3339 timestamp of the activation. |
| `deactivated_at` | `TEXT NULL` | NULL while active; set when a newer version is activated. |
| `source` | `TEXT NOT NULL` | Free-form label (`operator-cli` by default). |

A partial UNIQUE index `idx_active_policy_singleton` on `deactivated_at WHERE deactivated_at IS NULL` enforces that **at most one row is active at any moment** — a buggy CLI cannot leave two simultaneously-active policies.

## Doctor integration

`sbo3l doctor` surfaces this table as the `active_policy` row:

| State | Doctor row | Reason |
| --- | --- | --- |
| Table present + active row | `ok` | Includes `active=v<N>` and the first 12 hex chars of the hash so an operator can confirm which policy is live. |
| Table present + no active row | `skip` | Truthful: the table is here but nothing has been activated yet. The reason points at `sbo3l policy activate`. |
| Table missing entirely | `skip` | Older daemon DB before V006. The reason points at PSM-A3 + V006. |

## Out of scope on this PR

- Daemon hot-reload of activated policies. Today the daemon still loads the embedded reference policy at startup; PSM-B will wire it to read from V006 on boot.
- Schema-versioned policy formats. The active-policy table stores the canonical JSON verbatim, so a future schema bump just means activating policies of the new shape — no migration on V006 itself.
- Multi-party / on-chain activation. As stated above, this is local lifecycle, not remote governance.
