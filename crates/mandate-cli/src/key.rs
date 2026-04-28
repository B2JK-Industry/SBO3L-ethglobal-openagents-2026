//! `mandate key {init,list,rotate} --mock` — production-shaped mock KMS
//! keyring CLI (PSM-A1.9).
//!
//! These commands operate on the `mock_kms_keys` SQLite table (V005)
//! that stores per-version public-key metadata for the
//! `MockKmsSigner` keyring shipped in PSM-A1. They never touch private
//! key material on disk: the deterministic root-seed is supplied on
//! every CLI invocation; only the resulting public metadata is
//! persisted.
//!
//! The required `--mock` flag is an explicit acknowledgement that this
//! is mock infrastructure. A real KMS keyring CLI would never be
//! plug-compatible with these commands; documented in
//! `docs/cli/mock-kms.md`.

use std::path::Path;
use std::process::ExitCode;

use chrono::Utc;
use mandate_core::mock_kms;
use mandate_storage::mock_kms_store::MockKmsKeyRecord;
use mandate_storage::Storage;

const ROOT_SEED_HEX_LEN: usize = 64;

fn open_db(db: &Path) -> Result<Storage, String> {
    Storage::open(db).map_err(|e| format!("failed to open db {}: {e}", db.display()))
}

fn parse_root_seed(hex_str: &str) -> Result<[u8; 32], String> {
    if hex_str.len() != ROOT_SEED_HEX_LEN {
        return Err(format!(
            "--root-seed must be {ROOT_SEED_HEX_LEN} hex chars (32 bytes), got {}",
            hex_str.len()
        ));
    }
    let bytes = hex::decode(hex_str).map_err(|e| format!("--root-seed is not valid hex: {e}"))?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn require_mock_flag(mock: bool) -> Result<(), String> {
    if !mock {
        return Err(
            "this command operates on the mock KMS keyring; pass --mock to acknowledge \
             (production KMS backends are not implemented in this build)"
                .to_string(),
        );
    }
    Ok(())
}

/// `mandate key init --mock --role <name> --root-seed <hex64> [--genesis <ts>] --db <path>`
///
/// Creates the v1 row of a fresh keyring for `role`. Idempotent: if a
/// row for `(role, 1)` already exists (e.g. previous run with the same
/// args), the command no-ops with exit 0 and prints the existing meta.
pub fn cmd_init(
    mock: bool,
    role: &str,
    root_seed_hex: &str,
    genesis: Option<&str>,
    db: &Path,
) -> ExitCode {
    if let Err(e) = require_mock_flag(mock) {
        eprintln!("mandate key init: {e}");
        return ExitCode::from(2);
    }
    let root_seed = match parse_root_seed(root_seed_hex) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mandate key init: {e}");
            return ExitCode::from(2);
        }
    };
    let genesis_ts = match genesis {
        Some(g) => match chrono::DateTime::parse_from_rfc3339(g) {
            Ok(t) => t.with_timezone(&Utc),
            Err(e) => {
                eprintln!("mandate key init: --genesis must be RFC3339: {e}");
                return ExitCode::from(2);
            }
        },
        None => Utc::now(),
    };

    let mut storage = match open_db(db) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mandate key init: {e}");
            return ExitCode::from(2);
        }
    };

    let (key_id, public_hex) = mock_kms::derive_key_metadata(role, 1, &root_seed);
    let record = MockKmsKeyRecord {
        role: role.to_string(),
        version: 1,
        key_id: key_id.clone(),
        public_hex: public_hex.clone(),
        created_at: genesis_ts,
    };
    match storage.mock_kms_insert(&record) {
        Ok(true) => {
            println!(
                "mock-kms: initialised role={role} version=1 key_id={key_id} \
                 public_hex={public_hex} created_at={}",
                genesis_ts.to_rfc3339()
            );
            ExitCode::SUCCESS
        }
        Ok(false) => {
            // Row already existed — surface the existing meta so the
            // caller can confirm idempotence without consulting `list`.
            match storage.mock_kms_list(Some(role)) {
                Ok(rows) if !rows.is_empty() => {
                    let v1 = rows.iter().find(|r| r.version == 1);
                    match v1 {
                        Some(r) => {
                            println!(
                                "mock-kms: role={} v1 already initialised; key_id={} \
                                 public_hex={} created_at={}",
                                r.role,
                                r.key_id,
                                r.public_hex,
                                r.created_at.to_rfc3339()
                            );
                            ExitCode::SUCCESS
                        }
                        None => {
                            eprintln!(
                                "mandate key init: existing rows for role={role} but no v1; \
                                 db may have been hand-edited"
                            );
                            ExitCode::from(1)
                        }
                    }
                }
                Ok(_) => {
                    eprintln!(
                        "mandate key init: insert was rejected as duplicate but no rows \
                         found for role={role}; db inconsistency"
                    );
                    ExitCode::from(1)
                }
                Err(e) => {
                    eprintln!("mandate key init: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Err(e) => {
            eprintln!("mandate key init: {e}");
            ExitCode::from(1)
        }
    }
}

/// `mandate key list --mock [--role <name>] --db <path>`
///
/// Dumps the keyring (or only one role's worth) in `(role, version)`
/// order. Output explicitly prefixes every line with `mock-kms:` so a
/// human skimming the output cannot mistake it for production KMS.
pub fn cmd_list(mock: bool, role: Option<&str>, db: &Path) -> ExitCode {
    if let Err(e) = require_mock_flag(mock) {
        eprintln!("mandate key list: {e}");
        return ExitCode::from(2);
    }
    let storage = match open_db(db) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mandate key list: {e}");
            return ExitCode::from(2);
        }
    };
    let rows = match storage.mock_kms_list(role) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("mandate key list: {e}");
            return ExitCode::from(1);
        }
    };
    if rows.is_empty() {
        match role {
            Some(r) => println!("mock-kms: no keyring entries for role={r}"),
            None => println!("mock-kms: no keyring entries"),
        }
        return ExitCode::SUCCESS;
    }
    // Codex P2 on PR #28: every output line — header, column line,
    // each row — must start with the `mock-kms:` disclosure so a human
    // skimming a copy-pasted slice of the output cannot mistake any
    // single line for production KMS material.
    println!(
        "mock-kms: keyring ({} row{}):",
        rows.len(),
        if rows.len() == 1 { "" } else { "s" }
    );
    println!("mock-kms:   role                  ver  key_id                 public_hex                                                                          created_at");
    for r in rows {
        println!(
            "mock-kms:   {role:<22}{version:<5}{key_id:<23}{public_hex}  {ts}",
            role = r.role,
            version = r.version,
            key_id = r.key_id,
            public_hex = r.public_hex,
            ts = r.created_at.to_rfc3339()
        );
    }
    ExitCode::SUCCESS
}

/// `mandate key rotate --mock --role <name> --root-seed <hex64> --db <path>`
///
/// Reads the highest existing version for `role`, derives the next
/// version's public material from `(role, n+1, root_seed)`, and
/// inserts the new row. The previous version is retained — receipts
/// signed under it stay verifiable via the keyring.
pub fn cmd_rotate(mock: bool, role: &str, root_seed_hex: &str, db: &Path) -> ExitCode {
    if let Err(e) = require_mock_flag(mock) {
        eprintln!("mandate key rotate: {e}");
        return ExitCode::from(2);
    }
    let root_seed = match parse_root_seed(root_seed_hex) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mandate key rotate: {e}");
            return ExitCode::from(2);
        }
    };
    let mut storage = match open_db(db) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mandate key rotate: {e}");
            return ExitCode::from(2);
        }
    };
    let current = match storage.mock_kms_current_version(role) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("mandate key rotate: {e}");
            return ExitCode::from(1);
        }
    };
    let current_version = match current {
        Some(v) => v,
        None => {
            eprintln!(
                "mandate key rotate: no keyring exists for role={role}. Run \
                 `mandate key init --mock --role {role} --root-seed <hex64> --db {} ` first.",
                db.display()
            );
            return ExitCode::from(1);
        }
    };

    // Codex P2 on PR #28: refuse to rotate when the supplied --root-seed
    // doesn't match the seed that produced the existing v(current_version).
    // Without this check, a typo or mismatched secret silently inserts a
    // v(n+1) row that the daemon's keyring can't actually re-derive — the
    // keyring would diverge from the operator's notion of "the rotation
    // seed", which is the entire authentication contract for the mock
    // KMS surface. We compare both `key_id` and `public_hex` because
    // either alone would already reveal a seed mismatch.
    let stored_current = match storage.mock_kms_list(Some(role)) {
        Ok(rows) => rows.into_iter().find(|r| r.version == current_version),
        Err(e) => {
            eprintln!("mandate key rotate: failed to read existing keyring: {e}");
            return ExitCode::from(1);
        }
    };
    let stored_current = match stored_current {
        Some(r) => r,
        None => {
            // current_version came back Some(v) but the row vanished —
            // means another process raced us between the two queries.
            // Treat as a transient inconsistency, not a seed mismatch.
            eprintln!(
                "mandate key rotate: db inconsistency: current_version={current_version} \
                 reported for role={role} but no matching row found"
            );
            return ExitCode::from(1);
        }
    };
    let (expected_current_key_id, expected_current_public_hex) =
        mock_kms::derive_key_metadata(role, current_version, &root_seed);
    if stored_current.key_id != expected_current_key_id
        || stored_current.public_hex != expected_current_public_hex
    {
        eprintln!(
            "mandate key rotate: --root-seed does not match the seed that produced \
             the stored v{current_version} for role={role}; refusing to rotate. \
             Pass the same --root-seed used at `mandate key init` (or the most \
             recent `mandate key rotate`)."
        );
        return ExitCode::from(2);
    }

    let next_version = current_version + 1;
    let (key_id, public_hex) = mock_kms::derive_key_metadata(role, next_version, &root_seed);
    let now = Utc::now();
    let record = MockKmsKeyRecord {
        role: role.to_string(),
        version: next_version,
        key_id: key_id.clone(),
        public_hex: public_hex.clone(),
        created_at: now,
    };
    match storage.mock_kms_insert(&record) {
        Ok(true) => {
            println!(
                "mock-kms: rotated role={role}: v{current_version} → v{next_version} \
                 key_id={key_id} public_hex={public_hex} created_at={}",
                now.to_rfc3339()
            );
            ExitCode::SUCCESS
        }
        Ok(false) => {
            eprintln!(
                "mandate key rotate: insert rejected — likely a parallel rotate \
                 already inserted v{next_version} for role={role}"
            );
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("mandate key rotate: {e}");
            ExitCode::from(1)
        }
    }
}

// Integration tests live in `crates/mandate-cli/tests/key_cli.rs` so they
// can reach the cargo-built binary via `env!("CARGO_BIN_EXE_mandate")`.
