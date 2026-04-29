//! `sbo3l audit checkpoint {create,verify}` — **mock-anchored**
//! audit checkpoints (PSM-A4).
//!
//! A checkpoint snapshots the audit chain's tip — `sequence`,
//! `latest_event_id`, `latest_event_hash` — and aggregates every
//! `event_hash` in the prefix into a single SHA-256 `chain_digest`,
//! then stamps the bundle with a deterministic `mock_anchor_ref`
//! that simulates the *shape* of an on-chain anchor without ever
//! leaving the process.
//!
//! Truthfulness rules:
//! - `mock_anchor: true` is set on every JSON artifact and the
//!   string `mock-anchor:` is the prefix on every CLI output line.
//! - The mock anchor reference (`local-mock-anchor-<16 hex>`, the
//!   16-hex-char tail being the first 8 bytes of a SHA-256 digest)
//!   is derived from the checkpoint content; it is **not** broadcast,
//!   not signed by any chain, not attested by any oracle.
//! - `sbo3l audit checkpoint verify <file>` accepts a checkpoint
//!   JSON and runs structural checks. With `--db <path>` it
//!   additionally re-derives `chain_digest` + `latest_event_hash`
//!   from the live audit chain and compares — that's the only way
//!   to confirm the checkpoint actually anchors *this* DB's chain.

use std::path::Path;
use std::process::ExitCode;

use chrono::Utc;
use sbo3l_storage::audit_checkpoint_store::compute_chain_digest;
use sbo3l_storage::Storage;
use serde::{Deserialize, Serialize};

/// Stable on-disk shape of a `sbo3l audit checkpoint create` artifact.
/// Versioned schema id ensures forward-compatible changes can route
/// through a different parser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditCheckpointDoc {
    pub schema: String,
    pub mock_anchor: bool,
    pub explanation: String,
    pub sequence: u64,
    pub latest_event_id: String,
    pub latest_event_hash: String,
    pub chain_digest: String,
    pub mock_anchor_ref: String,
    pub created_at: String,
}

const SCHEMA_ID: &str = "sbo3l.audit_checkpoint.v1";
const EXPLANATION: &str = "Local mock anchor; not a real onchain anchor. \
                           See docs/cli/audit-checkpoint.md.";

fn open_db(db: &Path) -> Result<Storage, String> {
    Storage::open(db).map_err(|e| format!("failed to open db {}: {e}", db.display()))
}

/// Print a single line prefixed with `mock-anchor:` for loud
/// disclosure. Every stdout line in this module routes through here.
fn say(line: impl AsRef<str>) {
    println!("mock-anchor: {}", line.as_ref());
}

/// `sbo3l audit checkpoint create --db <path> [--out <file>]`
///
/// Reads the audit chain from `<path>`, computes the chain digest,
/// inserts a row into `audit_checkpoints` (V007), and prints the
/// resulting checkpoint to stdout. With `--out` the same JSON is
/// also written to disk for offline distribution.
///
/// Exit codes:
/// - 0 — checkpoint created
/// - 1 — db open / write failure
/// - 3 — audit chain is empty (the honest "nothing to checkpoint" path)
pub fn cmd_create(db: &Path, out: Option<&Path>) -> ExitCode {
    let mut storage = match open_db(db) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sbo3l audit checkpoint create: {e}");
            return ExitCode::from(1);
        }
    };

    let n = match storage.audit_count() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("sbo3l audit checkpoint create: {e}");
            return ExitCode::from(1);
        }
    };
    if n == 0 {
        eprintln!(
            "sbo3l audit checkpoint create: audit chain is empty in db {}; \
             nothing to anchor. Append at least one audit event before creating \
             a checkpoint.",
            db.display()
        );
        return ExitCode::from(3);
    }

    let hashes = match storage.audit_event_hashes_in_order() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("sbo3l audit checkpoint create: {e}");
            return ExitCode::from(1);
        }
    };
    let chain_digest = match compute_chain_digest(&hashes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("sbo3l audit checkpoint create: chain digest failed: {e}");
            return ExitCode::from(1);
        }
    };

    let now = Utc::now();
    let rec = match storage.audit_checkpoint_create(&chain_digest, now) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sbo3l audit checkpoint create: {e}");
            return ExitCode::from(1);
        }
    };

    let doc = AuditCheckpointDoc {
        schema: SCHEMA_ID.to_string(),
        mock_anchor: true,
        explanation: EXPLANATION.to_string(),
        sequence: rec.sequence,
        latest_event_id: rec.latest_event_id.clone(),
        latest_event_hash: rec.latest_event_hash.clone(),
        chain_digest: rec.chain_digest.clone(),
        mock_anchor_ref: rec.mock_anchor_ref.clone(),
        created_at: rec.created_at.to_rfc3339(),
    };

    say(format!("schema:            {}", doc.schema));
    say(format!("sequence:          {}", doc.sequence));
    say(format!("latest_event_id:   {}", doc.latest_event_id));
    say(format!("latest_event_hash: {}", doc.latest_event_hash));
    say(format!("chain_digest:      {}", doc.chain_digest));
    say(format!("mock_anchor_ref:   {}", doc.mock_anchor_ref));
    say(format!("created_at:        {}", doc.created_at));
    say(format!("explanation:       {}", doc.explanation));

    if let Some(p) = out {
        let json = match serde_json::to_string_pretty(&doc) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("sbo3l audit checkpoint create: serialise failed: {e}");
                return ExitCode::from(1);
            }
        };
        if let Err(e) = std::fs::write(p, json) {
            eprintln!(
                "sbo3l audit checkpoint create: write {} failed: {e}",
                p.display()
            );
            return ExitCode::from(1);
        }
        say(format!("written to {}", p.display()));
    }
    ExitCode::SUCCESS
}

/// `sbo3l audit checkpoint verify <file> [--db <path>]`
///
/// Verifies a checkpoint JSON artifact. Runs structural checks
/// unconditionally:
/// - schema is `sbo3l.audit_checkpoint.v1`
/// - `mock_anchor` is `true`
/// - `mock_anchor_ref` has the expected `local-mock-anchor-<16 hex>`
///   shape
/// - `latest_event_hash` and `chain_digest` are 64 hex chars each
///
/// With `--db <path>` it additionally re-derives `chain_digest` from
/// the actual audit chain and confirms the checkpoint was issued by
/// **this** DB. The DB must contain a matching row in
/// `audit_checkpoints` keyed on `mock_anchor_ref` so a forged JSON
/// (right shape, never persisted) is rejected.
///
/// Exit codes:
/// - 0 — verified
/// - 1 — IO / parse / db error
/// - 2 — verification failed (tampered, wrong DB, missing row, …)
pub fn cmd_verify(file: &Path, db: Option<&Path>) -> ExitCode {
    let raw = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "sbo3l audit checkpoint verify: read {} failed: {e}",
                file.display()
            );
            return ExitCode::from(1);
        }
    };
    let doc: AuditCheckpointDoc = match serde_json::from_str(&raw) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("sbo3l audit checkpoint verify: parse failed: {e}");
            return ExitCode::from(1);
        }
    };

    if doc.schema != SCHEMA_ID {
        eprintln!(
            "sbo3l audit checkpoint verify: bad schema id {:?} (expected {SCHEMA_ID})",
            doc.schema
        );
        return ExitCode::from(2);
    }
    if !doc.mock_anchor {
        eprintln!(
            "sbo3l audit checkpoint verify: mock_anchor must be true; this CLI \
             does not produce or verify real onchain anchors"
        );
        return ExitCode::from(2);
    }
    if !is_hex_64(&doc.latest_event_hash) {
        eprintln!("sbo3l audit checkpoint verify: latest_event_hash must be 64 hex chars");
        return ExitCode::from(2);
    }
    if !is_hex_64(&doc.chain_digest) {
        eprintln!("sbo3l audit checkpoint verify: chain_digest must be 64 hex chars");
        return ExitCode::from(2);
    }
    if !is_mock_anchor_ref(&doc.mock_anchor_ref) {
        eprintln!(
            "sbo3l audit checkpoint verify: mock_anchor_ref must match \
             `local-mock-anchor-<16 hex>`"
        );
        return ExitCode::from(2);
    }

    say(format!("schema:            {}", doc.schema));
    say(format!("mock_anchor:       {}", doc.mock_anchor));
    say(format!("sequence:          {}", doc.sequence));
    say(format!("latest_event_id:   {}", doc.latest_event_id));
    say(format!("latest_event_hash: {}", doc.latest_event_hash));
    say(format!("chain_digest:      {}", doc.chain_digest));
    say(format!("mock_anchor_ref:   {}", doc.mock_anchor_ref));
    say("structural verify: ok");

    if let Some(db_path) = db {
        let storage = match open_db(db_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("sbo3l audit checkpoint verify: {e}");
                return ExitCode::from(1);
            }
        };

        // 1. Re-derive the chain digest from the live chain.
        let hashes = match storage.audit_event_hashes_in_order() {
            Ok(h) => h,
            Err(e) => {
                eprintln!("sbo3l audit checkpoint verify: read chain: {e}");
                return ExitCode::from(1);
            }
        };
        let computed = match compute_chain_digest(&hashes) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("sbo3l audit checkpoint verify: chain digest: {e}");
                return ExitCode::from(1);
            }
        };

        // 2. Confirm the persisted row matches the artifact (catches
        //    "forged JSON, never inserted" and "checkpoint JSON came
        //    from a different DB" simultaneously).
        let row = match storage.audit_checkpoint_by_anchor_ref(&doc.mock_anchor_ref) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("sbo3l audit checkpoint verify: lookup: {e}");
                return ExitCode::from(1);
            }
        };
        let row = match row {
            Some(r) => r,
            None => {
                eprintln!(
                    "sbo3l audit checkpoint verify: no row in {} matches \
                     mock_anchor_ref={}; checkpoint was not issued by this DB",
                    db_path.display(),
                    doc.mock_anchor_ref
                );
                return ExitCode::from(2);
            }
        };
        if row.chain_digest != doc.chain_digest {
            eprintln!(
                "sbo3l audit checkpoint verify: chain_digest mismatch (db={}, doc={}); \
                 the checkpoint JSON has been tampered with or came from a different DB",
                row.chain_digest, doc.chain_digest
            );
            return ExitCode::from(2);
        }
        if row.latest_event_hash != doc.latest_event_hash {
            eprintln!(
                "sbo3l audit checkpoint verify: latest_event_hash mismatch (db={}, doc={})",
                row.latest_event_hash, doc.latest_event_hash
            );
            return ExitCode::from(2);
        }

        // 3. Catch "checkpoint over a stale prefix": if the live chain
        //    has grown past the checkpoint's `sequence`, that's
        //    informational not an error — the checkpoint still
        //    correctly anchors the prefix it claims. We surface the
        //    drift as a `mock-anchor:` line so an operator notices.
        if computed != doc.chain_digest {
            // The live chain digest doesn't match the checkpoint's
            // digest. Either the chain advanced (informational) OR a
            // historical event was tampered (bad). Distinguish by
            // re-deriving the digest of the prefix through `sequence`.
            let prefix: Vec<String> = hashes.iter().take(doc.sequence as usize).cloned().collect();
            let prefix_digest = match compute_chain_digest(&prefix) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("sbo3l audit checkpoint verify: prefix digest: {e}");
                    return ExitCode::from(1);
                }
            };
            if prefix_digest == doc.chain_digest {
                say(format!(
                    "live chain has advanced beyond checkpoint (seq doc={} live={}); \
                     prefix-through-doc-seq still matches",
                    doc.sequence,
                    hashes.len(),
                ));
            } else {
                eprintln!(
                    "sbo3l audit checkpoint verify: chain prefix through seq={} \
                     no longer matches the checkpoint — the historical chain has \
                     been tampered with",
                    doc.sequence
                );
                return ExitCode::from(2);
            }
        }

        say("db cross-check:    ok (chain_digest, latest_event_hash, anchor row all match)");
    } else {
        say("db cross-check:    skipped (no --db provided)");
    }

    say("verify result:     ok");
    ExitCode::SUCCESS
}

fn is_hex_64(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_mock_anchor_ref(s: &str) -> bool {
    let prefix = "local-mock-anchor-";
    if !s.starts_with(prefix) {
        return false;
    }
    let tail = &s[prefix.len()..];
    tail.len() == 16 && tail.chars().all(|c| c.is_ascii_hexdigit())
}
