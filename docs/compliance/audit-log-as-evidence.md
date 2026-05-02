# Audit log as compliance evidence

> **Reference document for auditor walkthroughs.** Maps SBO3L's hash-chained Ed25519-signed audit log to specific control IDs across SOC 2, GDPR, HIPAA, and PCI-DSS. The same artifact (one append-only SQLite table + Rust verifier) satisfies multiple frameworks because audit-logging requirements are structurally similar across standards.

## The artifact

| Item | Reference |
|---|---|
| Schema | `crates/sbo3l-storage/migrations/V001_audit_events.sql` (and successors) |
| Append code | `Storage::finalize_decision` in `crates/sbo3l-storage/src/lib.rs` |
| Linkage | `chain_hash_v2` — SHA-256 of `prev_chain_hash || payload_hash` |
| Signature | Ed25519 over `chain_hash_v2`, key managed by `sbo3l-identity` |
| Verifier | `sbo3l verify-audit --strict-hash` (CLI) + WASM verifier on `/proof` page |
| Format | JCS-canonical JSON for `payload`; SHA-256 of canonical bytes = `payload_hash` |

## Properties by construction

| Property | How |
|---|---|
| Append-only | DB constraint: no `UPDATE` or `DELETE` on `audit_events` outside the documented redaction path |
| Tamper-evident | `chain_hash_v2` makes any modification recoverable by any party with the chain |
| Signed | Ed25519 over `chain_hash_v2` proves the row was created by the daemon at that point in time |
| Linearizable | `seq` is a strict ascending integer; `chain_hash_v2` makes any reordering detectable |
| Replayable | The whole chain can be exported (`sbo3l audit export`) and re-verified offline |
| Forge-resistant | Without the daemon's Ed25519 private key, an attacker cannot extend the chain |
| Redactable (with proof) | Tombstone-and-resign approach (see `gdpr-posture.md`) preserves linkage while removing personal data |

## Mapping to SOC 2

| TSC ID | Description | How SBO3L's audit log satisfies |
|---|---|---|
| **CC7.2** | Monitor + log system activities | Every policy decision generates an audit row before any side effect |
| CC7.3 | Identify + analyze incidents | The chain itself is the forensic asset |
| CC8.1 | Authorize changes | Every config change (policy, identity, signer) is itself an audit row |
| CC4.1 | Periodic monitoring | `regression-on-main` workflow re-verifies the chain |
| **CC5.2** | Technology controls | The Ed25519 signature is itself a technology control |

## Mapping to GDPR

| Article | Description | How SBO3L's audit log satisfies |
|---|---|---|
| **Art. 30** | Records of Processing Activities | Audit chain IS the RoPA — every processing event recorded |
| Art. 32(1)(d) | Processes for testing + evaluating effectiveness of measures | Verifier provides on-demand effectiveness check |
| Art. 33(1) | Breach notification ≤ 72h | Audit chain provides forensic certainty about breach scope |
| **Art. 5(1)(f)** | Integrity + confidentiality principle | Ed25519 + hash linkage IS the integrity proof |
| Art. 5(2) | Accountability | Chain demonstrates compliance by construction |
| Art. 17(1) | Right to erasure | Tombstone-and-resign approach preserves audit integrity while honoring the right |

## Mapping to HIPAA

| Citation | Description | How SBO3L's audit log satisfies |
|---|---|---|
| **§164.312(b)** | Audit Controls | The chain IS the §164.312(b) implementation. **This is the exemplary case.** |
| §164.312(c)(1) | Integrity | `chain_hash_v2` linkage |
| §164.312(c)(2) | Mechanism to authenticate ePHI | Ed25519 signature over `chain_hash_v2` proves authenticity |
| §164.308(a)(1)(ii)(D) | Information System Activity Review | Periodic verification via `sbo3l verify-audit --strict-hash` |
| §164.308(a)(6)(ii) | Response + reporting | Chain-based forensic timeline |

## Mapping to PCI-DSS v4.0

| Requirement | Description | How SBO3L's audit log satisfies |
|---|---|---|
| **Req 10.2** | Implement automated audit logs | Every policy decision generates an audit row |
| Req 10.2.1 | All individual user accesses to CHD | _N/A — SBO3L is out of CHD scope_ |
| Req 10.3 | Record at least these audit log entries | Type, timestamp, user ID, success/failure, origin all in `payload` |
| Req 10.4 | Time synchronization | Daemon uses NTP-synced clock; `expiry` enforces ≤ 60s skew |
| **Req 10.5** | Secure audit trails | Ed25519 signature + `chain_hash_v2` make the trail immutable |
| Req 10.5.1 | Limit viewing of audit trails to need-to-know | Multi-tenant V010 enforces this |
| Req 10.5.2 | Protect audit trail files from modification | DB constraint + chain verification + Ed25519 signature |
| Req 10.5.3 | Promptly back up audit trails | SQLite WAL replication (deployment concern) |
| Req 10.5.5 | Use file-integrity monitoring tools | The `chain_hash_v2` IS file-integrity monitoring at row level |
| Req 10.7 | Retain audit trail history for ≥ 1 year | Configurable; default 7 years per industry standard |

## Auditor walkthrough script

This is the script we recommend an external auditor follow when validating SBO3L's audit posture for any of the four frameworks. Every command below maps to a real subcommand on the shipped `sbo3l` CLI (run `sbo3l --help` for the full surface).

### 1. Verify chain by construction

The daemon stores its audit chain in a SQLite file plus exposes a JSONL export. To verify the entire chain:

```bash
# Step A: ask the daemon for a JSONL dump of the chain (or use a saved one)
sbo3l audit export \
  --receipt /path/to/some-receipt.json \
  --db /path/to/sbo3l.db \
  --receipt-pubkey <hex32> \
  --audit-pubkey <hex32> \
  --out /tmp/chain.jsonl-bundle.json

# Step B: re-verify the bundle
sbo3l audit verify-bundle --path /tmp/chain.jsonl-bundle.json
# Expected: exit 0
```

Or for a standalone JSONL file (one `SignedAuditEvent` per line):

```bash
sbo3l verify-audit --path /path/to/chain.jsonl --pubkey <hex32>
# Expected: exit 0
```

If verification fails:
- The DB has been tampered with (cause for incident response), OR
- A bug exists in the verifier (cause for `SECURITY.md` report)

### 2. Verify by spot-check

The CLI does not currently expose a single-row inspector — `sbo3l audit show` and `sbo3l audit canonical-hash` are NOT shipped subcommands (verified in `crates/sbo3l-cli/src/main.rs::AuditCmd`). To spot-check one event:

```bash
# Use sqlite to read one row directly
sqlite3 /path/to/sbo3l.db \
  "SELECT seq, payload, chain_hash_v2 FROM audit_events WHERE seq = 42"
# Then re-export the bundle prefix containing that seq via
# sbo3l audit export and run sbo3l audit verify-bundle on it.
```

A single-row CLI inspector + canonical-hash printer is on the
roadmap in [`docs/dev3/scope-cut-report.md`](../dev3/scope-cut-report.md).

### 3. Verify by external proof

```bash
# Drop the corresponding capsule into the /proof page WASM verifier
# (https://sbo3l-marketing.vercel.app/proof or local equivalent)
# Confirm all strict-mode checks pass
```

The capsule itself is produced by `sbo3l passport run` and verified offline by `sbo3l passport verify --strict --receipt-pubkey <hex> --audit-bundle <bundle.json> --policy <policy.json>`.

### 4. Verify the policy boundary

```bash
# Re-derive the policy hash from a snapshot
sbo3l audit export ... --out /tmp/bundle.json
# Inspect bundle.policy.policy_hash; compare to ENS text record
```

`sbo3l policy current` is not a shipped subcommand. The bundle exported in step 1 carries the active policy hash inline; that is the canonical surface.

### 5. Verify chain Ed25519 signature

`sbo3l verify-audit --pubkey <hex>` from step 1 already verifies every event's signature. There is no `sbo3l audit verify-row` subcommand.

### 6. Verify chain Ed25519 cannot be forged without the key

This is the structural argument. The auditor confirms (without running an experiment):
- Ed25519 has 128-bit security against existential forgery (Bernstein 2011, Brendel et al. 2021).
- A daemon without access to the private key cannot extend the chain.
- The dev signer mode (`SBO3L_DEV_ONLY_SIGNER=1`) is gated behind a startup banner; the production deploy uses KMS-backed signing.

## When the audit log is NOT sufficient

The audit log is necessary but not sufficient for full compliance. It does not, by itself, satisfy:

- **Personnel controls** (SOC 2 CC1, HIPAA §164.308(a)(3)) — these are about humans, not data.
- **Physical access** (SOC 2 CC6.4, HIPAA §164.310) — delegated to cloud DC.
- **Network security** (PCI-DSS Req 1) — customer-side firewall + network segmentation.
- **Encryption at rest** (cross-cutting) — V011 work is the gap.

The audit log is **most aligned** with the "logging + monitoring + integrity" controls across all four frameworks (SOC 2 CC7, GDPR Art. 5/30, HIPAA §164.312(b), PCI-DSS Req 10).

## See also

- [`README.md`](README.md) — top-level posture.
- [`shared-controls.md`](shared-controls.md) — controls satisfying multiple frameworks.
