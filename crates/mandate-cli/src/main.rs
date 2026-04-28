use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use mandate_core::audit::{verify_chain, SignedAuditEvent};
use mandate_core::audit_bundle::{self, AuditBundle};
use mandate_core::receipt::PolicyReceipt;
use mandate_core::{schema, SchemaError};

mod doctor;
mod key;
mod policy;

#[derive(Parser, Debug)]
#[command(
    name = "mandate",
    version,
    about = "Mandate — spending mandates for autonomous agents.",
    long_about = "Mandate is a local policy, budget, receipt and audit firewall for AI agents.\n\
                  Public brand: Mandate. Tagline: Don't give your agent a wallet. Give it a mandate."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Agent Payment Request Protocol commands
    Aprp {
        #[command(subcommand)]
        op: AprpCmd,
    },
    /// Verify a Mandate audit hash chain (JSONL)
    VerifyAudit {
        /// Path to a JSONL audit log
        #[arg(long)]
        path: PathBuf,
        /// Skip recomputation of event_hash (for fixtures with placeholder hashes)
        #[arg(long, default_value_t = false)]
        skip_hash: bool,
        /// Public key (hex, 32 bytes) to verify each event's signature
        #[arg(long)]
        pubkey: Option<String>,
    },
    /// Print the schema id for a wire format
    Schema {
        /// One of: aprp | policy | decision-token | policy-receipt | audit-event | x402
        kind: String,
    },
    /// Verifiable audit export bundle commands.
    ///
    /// `mandate audit export` packages a signed receipt + the relevant audit
    /// chain segment + the public verification keys into a single JSON file
    /// that anyone can re-verify offline. `mandate audit verify-bundle`
    /// re-derives every signature, hash and chain link in that file and
    /// reports the result. Tagline: Mandate does not just decide. It leaves
    /// behind verifiable proof.
    Audit {
        #[command(subcommand)]
        op: AuditCmd,
    },
    /// Operator readiness summary.
    ///
    /// Inspects a Mandate SQLite database (or an in-memory fresh one) and
    /// reports per-feature status: storage open, migrations applied, audit
    /// chain integrity, nonce-replay table, idempotency table, mock KMS
    /// keyring, active policy. Each check is **honest about scope** — a
    /// feature that is not implemented yet surfaces as `skip`, never as
    /// fake `ok`. Output is a human-readable summary by default; `--json`
    /// emits a machine-readable envelope suitable for pipelines and the
    /// production-shaped runner.
    Doctor {
        /// Path to a Mandate SQLite database. If omitted, opens a fresh
        /// in-memory database (every check runs against a clean slate —
        /// useful for verifying the binary itself works).
        #[arg(long)]
        db: Option<PathBuf>,
        /// Emit JSON instead of human-readable text.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Mock KMS keyring commands (PSM-A1.9).
    ///
    /// Operate on the persistent `mock_kms_keys` SQLite table (V005).
    /// Every operation requires `--mock` for explicit disclosure — these
    /// commands are NOT plug-compatible with a production KMS. See
    /// `docs/cli/mock-kms.md`.
    Key {
        #[command(subcommand)]
        op: KeyCmd,
    },
    /// Local active-policy lifecycle (PSM-A3).
    ///
    /// Operates on the persistent `active_policy` SQLite table (V006).
    /// This is **local production-shaped lifecycle**, not remote
    /// governance: there is no on-chain anchor, no consensus, no
    /// signing on activation; whoever opens the DB activates the
    /// policy. See `docs/cli/policy.md`.
    Policy {
        #[command(subcommand)]
        op: PolicyCmd,
    },
}

#[derive(Subcommand, Debug)]
enum PolicyCmd {
    /// Parse + semantic-validate + canonical-hash a policy JSON file.
    /// Stdout: policy_hash + summary counts. No DB access.
    Validate {
        /// Path to a policy JSON file.
        path: PathBuf,
    },
    /// Print the currently-active policy row from the DB. Exits non-
    /// zero (code 3) if no policy has been activated yet — that is the
    /// honest signal, not a fake "ok".
    Current {
        /// SQLite database path.
        #[arg(long)]
        db: PathBuf,
    },
    /// Validate, hash, and activate a policy. Idempotent: re-running
    /// with the same policy is a no-op.
    Activate {
        /// Path to a policy JSON file.
        path: PathBuf,
        /// SQLite database path.
        #[arg(long)]
        db: PathBuf,
        /// Optional source label recorded in the row (default
        /// `operator-cli`).
        #[arg(long)]
        source: Option<String>,
    },
    /// Diff two candidate policy files at the canonical-JSON level.
    /// Exits 0 if identical, 1 if they differ (with a printed diff),
    /// 2 if either file fails to parse / validate.
    Diff {
        /// Left-hand policy file ("from").
        a: PathBuf,
        /// Right-hand policy file ("to").
        b: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum KeyCmd {
    /// Initialise a mock keyring's v1 row for the given `--role`.
    /// Idempotent: running again with the same args is a no-op.
    Init {
        /// Required acknowledgement that this is mock KMS infrastructure.
        #[arg(long)]
        mock: bool,
        /// Stable role name (e.g. `audit-mock`, `decision-mock`).
        #[arg(long)]
        role: String,
        /// 32-byte deterministic root seed, hex-encoded (64 chars). The
        /// seed never enters the SQLite database — only its derived
        /// public keys do.
        #[arg(long)]
        root_seed: String,
        /// Optional v1 timestamp (RFC3339). Defaults to "now()".
        #[arg(long)]
        genesis: Option<String>,
        /// SQLite database path (the same one the daemon writes to).
        #[arg(long)]
        db: PathBuf,
    },
    /// List keyring rows in `(role, version)` order.
    List {
        #[arg(long)]
        mock: bool,
        /// Restrict to a single role.
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        db: PathBuf,
    },
    /// Add the next version of `--role` to the keyring. Reads the
    /// existing maximum version, derives the new version's public
    /// material from `(role, n+1, root_seed)`, inserts the row.
    Rotate {
        #[arg(long)]
        mock: bool,
        #[arg(long)]
        role: String,
        #[arg(long)]
        root_seed: String,
        #[arg(long)]
        db: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum AuditCmd {
    /// Build a verifiable bundle from a signed receipt + audit chain.
    ///
    /// Exactly one chain source must be supplied:
    ///   --chain <jsonl-path>  reads SignedAuditEvent[] from a JSONL file
    ///                         (one event per line, genesis through the
    ///                         receipt's `audit_event_id`, in seq order).
    ///   --db    <sqlite-path> reads the chain directly from a Mandate
    ///                         daemon's SQLite storage (`mandate-storage`),
    ///                         slicing the prefix through the receipt's
    ///                         `audit_event_id`. Performs a pre-flight
    ///                         `verify_chain` and a receipt-signature
    ///                         check before writing the bundle.
    Export {
        /// Path to the signed PolicyReceipt JSON (the body returned by
        /// `POST /v1/payment-requests`, field `receipt`).
        #[arg(long)]
        receipt: PathBuf,
        /// Path to a JSONL audit chain (one SignedAuditEvent per line).
        /// Mutually exclusive with `--db`; exactly one must be supplied.
        #[arg(long, conflicts_with = "db", required_unless_present = "db")]
        chain: Option<PathBuf>,
        /// Path to a Mandate SQLite storage file (the `MANDATE_DB` the
        /// daemon writes to). Mutually exclusive with `--chain`; exactly
        /// one must be supplied. Reads the audit chain prefix through
        /// the receipt's `audit_event_id` directly from the daemon's
        /// persisted log — no out-of-band JSONL export required.
        #[arg(long, conflicts_with = "chain", required_unless_present = "chain")]
        db: Option<PathBuf>,
        /// Public verification key (hex) for the receipt signer (32 bytes).
        #[arg(long)]
        receipt_pubkey: String,
        /// Public verification key (hex) for the audit signer (32 bytes).
        #[arg(long)]
        audit_pubkey: String,
        /// Output path. If omitted, the bundle JSON is written to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Verify a previously-exported bundle.
    ///
    /// Re-derives every receipt + audit signature, every audit event_hash,
    /// and the prev_event_hash linkage of the included chain segment. Exits
    /// with code 0 on success, 1 on any verification failure, 2 on I/O or
    /// JSON-parse errors.
    VerifyBundle {
        /// Path to a bundle JSON file produced by `mandate audit export`.
        #[arg(long)]
        path: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum AprpCmd {
    /// Validate an APRP JSON document against schemas/aprp_v1.json
    Validate {
        /// Path to the APRP JSON file
        path: PathBuf,
    },
    /// Compute the canonical SHA-256 request hash of an APRP document
    Hash {
        /// Path to the APRP JSON file
        path: PathBuf,
    },
    /// Validate every APRP fixture under test-corpus/ and report pass/fail
    RunCorpus {
        /// Path to the test-corpus directory (defaults to ./test-corpus)
        #[arg(long, default_value = "test-corpus")]
        root: PathBuf,
    },
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Aprp {
            op: AprpCmd::Validate { path },
        } => match cmd_aprp_validate(&path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(rc) => rc,
        },
        Command::Aprp {
            op: AprpCmd::Hash { path },
        } => match cmd_aprp_hash(&path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(rc) => rc,
        },
        Command::Aprp {
            op: AprpCmd::RunCorpus { root },
        } => cmd_aprp_corpus(&root),
        Command::VerifyAudit {
            path,
            skip_hash,
            pubkey,
        } => cmd_verify_audit(&path, !skip_hash, pubkey.as_deref()),
        Command::Schema { kind } => cmd_schema(&kind),
        Command::Audit {
            op:
                AuditCmd::Export {
                    receipt,
                    chain,
                    db,
                    receipt_pubkey,
                    audit_pubkey,
                    out,
                },
        } => cmd_audit_export(
            &receipt,
            chain.as_deref(),
            db.as_deref(),
            &receipt_pubkey,
            &audit_pubkey,
            out.as_deref(),
        ),
        Command::Audit {
            op: AuditCmd::VerifyBundle { path },
        } => cmd_audit_verify_bundle(&path),
        Command::Doctor { db, json } => doctor::run(db.as_deref(), json),
        Command::Key {
            op:
                KeyCmd::Init {
                    mock,
                    role,
                    root_seed,
                    genesis,
                    db,
                },
        } => key::cmd_init(mock, &role, &root_seed, genesis.as_deref(), &db),
        Command::Key {
            op: KeyCmd::List { mock, role, db },
        } => key::cmd_list(mock, role.as_deref(), &db),
        Command::Key {
            op:
                KeyCmd::Rotate {
                    mock,
                    role,
                    root_seed,
                    db,
                },
        } => key::cmd_rotate(mock, &role, &root_seed, &db),
        Command::Policy {
            op: PolicyCmd::Validate { path },
        } => policy::cmd_validate(&path),
        Command::Policy {
            op: PolicyCmd::Current { db },
        } => policy::cmd_current(&db),
        Command::Policy {
            op: PolicyCmd::Activate { path, db, source },
        } => policy::cmd_activate(&path, &db, source.as_deref()),
        Command::Policy {
            op: PolicyCmd::Diff { a, b },
        } => policy::cmd_diff(&a, &b),
    }
}

fn cmd_aprp_validate(path: &Path) -> Result<(), ExitCode> {
    let value = read_json(path).map_err(|e| {
        eprintln!("error: {e}");
        ExitCode::from(2)
    })?;
    match schema::validate_aprp(&value) {
        Ok(()) => {
            println!("ok: {}", path.display());
            Ok(())
        }
        Err(err) => {
            eprintln!("invalid: {} -> {} ({err})", path.display(), err.code());
            Err(ExitCode::from(1))
        }
    }
}

fn cmd_aprp_hash(path: &Path) -> Result<(), ExitCode> {
    let value = read_json(path).map_err(|e| {
        eprintln!("error: {e}");
        ExitCode::from(2)
    })?;
    let h = mandate_core::hashing::request_hash(&value).map_err(|e| {
        eprintln!("error: {e}");
        ExitCode::from(2)
    })?;
    println!("{h}");
    Ok(())
}

#[derive(Debug)]
struct CorpusCase {
    relative: &'static str,
    expect_valid: bool,
    expect_code: Option<&'static str>,
}

const APRP_CORPUS: &[CorpusCase] = &[
    CorpusCase {
        relative: "aprp/golden_001_minimal.json",
        expect_valid: true,
        expect_code: None,
    },
    CorpusCase {
        relative: "aprp/deny_prompt_injection_request.json",
        expect_valid: true,
        expect_code: None,
    },
    CorpusCase {
        relative: "aprp/adversarial_unknown_field.json",
        expect_valid: false,
        expect_code: Some("schema.unknown_field"),
    },
];

fn cmd_aprp_corpus(root: &Path) -> ExitCode {
    let mut all_ok = true;
    for case in APRP_CORPUS {
        let path = root.join(case.relative);
        match read_json(&path) {
            Ok(value) => {
                let result = schema::validate_aprp(&value);
                let actual_valid = result.is_ok();
                let actual_code = result.as_ref().err().map(SchemaError::code);
                let status_ok = actual_valid == case.expect_valid
                    && match (case.expect_code, actual_code) {
                        (None, _) => true,
                        (Some(want), Some(got)) => want == got,
                        _ => false,
                    };
                if status_ok {
                    println!(
                        "ok    {} expect_valid={} actual={} code={:?}",
                        case.relative, case.expect_valid, actual_valid, actual_code
                    );
                } else {
                    all_ok = false;
                    println!(
                        "FAIL  {} expect_valid={} expect_code={:?} actual_valid={} actual_code={:?}",
                        case.relative,
                        case.expect_valid,
                        case.expect_code,
                        actual_valid,
                        actual_code
                    );
                }
            }
            Err(e) => {
                all_ok = false;
                println!("ERROR {}: {e}", path.display());
            }
        }
    }
    if all_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn cmd_schema(kind: &str) -> ExitCode {
    let id = match kind {
        "aprp" => schema::APRP_SCHEMA_ID,
        "policy" => schema::POLICY_SCHEMA_ID,
        "x402" => schema::X402_SCHEMA_ID,
        "policy-receipt" => schema::POLICY_RECEIPT_SCHEMA_ID,
        "decision-token" => schema::DECISION_TOKEN_SCHEMA_ID,
        "audit-event" => schema::AUDIT_EVENT_SCHEMA_ID,
        other => {
            eprintln!("unknown schema kind: {other}");
            return ExitCode::from(2);
        }
    };
    println!("{id}");
    ExitCode::SUCCESS
}

fn read_json(path: &Path) -> anyhow::Result<serde_json::Value> {
    let data = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&data)?;
    Ok(value)
}

fn cmd_verify_audit(path: &Path, verify_hashes: bool, pubkey: Option<&str>) -> ExitCode {
    let data = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {e}", path.display());
            return ExitCode::from(2);
        }
    };
    let mut events: Vec<SignedAuditEvent> = Vec::new();
    for (i, line) in data.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let signed: SignedAuditEvent = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("invalid JSON at line {}: {e}", i + 1);
                return ExitCode::from(1);
            }
        };
        // Schema-validate too.
        let raw: serde_json::Value = serde_json::from_str(line).unwrap();
        if let Err(e) = schema::validate_audit_event(&raw) {
            eprintln!(
                "schema invalid at line {} (seq={}): {e}",
                i + 1,
                signed.event.seq
            );
            return ExitCode::from(1);
        }
        events.push(signed);
    }
    match verify_chain(&events, verify_hashes, pubkey) {
        Ok(()) => {
            println!(
                "ok: {} events verified (hashes={}, sig={})",
                events.len(),
                verify_hashes,
                pubkey.is_some()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("audit chain invalid: {e}");
            ExitCode::from(1)
        }
    }
}

fn read_audit_chain_jsonl(path: &Path) -> anyhow::Result<Vec<SignedAuditEvent>> {
    let data = std::fs::read_to_string(path)?;
    let mut events: Vec<SignedAuditEvent> = Vec::new();
    for (i, line) in data.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let signed: SignedAuditEvent = serde_json::from_str(line).map_err(|e| {
            anyhow::anyhow!("chain JSONL line {} is not a SignedAuditEvent: {e}", i + 1)
        })?;
        events.push(signed);
    }
    Ok(events)
}

/// Open a Mandate SQLite store and slice the audit chain prefix through
/// the receipt's `audit_event_id`. Pre-flights the chain segment with
/// `verify_chain` against the supplied audit pubkey AND verifies the
/// receipt signature against the supplied receipt pubkey, so a DB-backed
/// export with mismatched keys or a corrupt chain fails immediately
/// with a clear message instead of producing an unverifiable bundle.
fn read_audit_chain_from_db(
    db_path: &Path,
    receipt: &PolicyReceipt,
    receipt_pubkey_hex: &str,
    audit_pubkey_hex: &str,
) -> anyhow::Result<Vec<SignedAuditEvent>> {
    if !db_path.exists() {
        anyhow::bail!("db path does not exist: {}", db_path.display());
    }
    let storage = mandate_storage::Storage::open(db_path)
        .map_err(|e| anyhow::anyhow!("opening db {}: {e}", db_path.display()))?;
    let chain = storage
        .audit_chain_prefix_through(&receipt.audit_event_id)
        .map_err(|e| anyhow::anyhow!("reading chain prefix from db: {e}"))?;
    // Pre-flight: chain integrity under the supplied audit pubkey. Catches
    // (a) a tampered DB, (b) a wrong --audit-pubkey, (c) a malformed pubkey
    // hex string — all surface here, not later in verify-bundle.
    verify_chain(&chain, true, Some(audit_pubkey_hex))
        .map_err(|e| anyhow::anyhow!("audit chain pre-flight failed: {e}"))?;
    // Pre-flight: receipt signature under the supplied receipt pubkey.
    receipt
        .verify(receipt_pubkey_hex)
        .map_err(|e| anyhow::anyhow!("receipt signature pre-flight failed: {e:?}"))?;
    Ok(chain)
}

fn cmd_audit_export(
    receipt_path: &Path,
    chain_path: Option<&Path>,
    db_path: Option<&Path>,
    receipt_pubkey_hex: &str,
    audit_pubkey_hex: &str,
    out: Option<&Path>,
) -> ExitCode {
    let receipt: PolicyReceipt = match std::fs::read_to_string(receipt_path)
        .map_err(anyhow::Error::from)
        .and_then(|s| serde_json::from_str(&s).map_err(anyhow::Error::from))
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error reading receipt {}: {e}", receipt_path.display());
            return ExitCode::from(2);
        }
    };
    // Clap enforces "exactly one of --chain / --db"; this match is a guard
    // against future flag rearrangements that would break that invariant.
    let chain = match (chain_path, db_path) {
        (Some(p), None) => match read_audit_chain_jsonl(p) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error reading chain {}: {e}", p.display());
                return ExitCode::from(2);
            }
        },
        (None, Some(p)) => {
            match read_audit_chain_from_db(p, &receipt, receipt_pubkey_hex, audit_pubkey_hex) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error reading chain from db {}: {e}", p.display());
                    return ExitCode::from(1);
                }
            }
        }
        _ => {
            eprintln!("internal error: exactly one of --chain or --db must be supplied");
            return ExitCode::from(2);
        }
    };
    let bundle = match audit_bundle::build(
        receipt,
        chain,
        receipt_pubkey_hex.to_string(),
        audit_pubkey_hex.to_string(),
        chrono::Utc::now(),
    ) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error building bundle: {e}");
            return ExitCode::from(1);
        }
    };
    // Pretty-print so humans can diff bundles visually; structure is the
    // same as the compact form because field order is fixed by the derive.
    let serialised = match serde_json::to_string_pretty(&bundle) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error serialising bundle: {e}");
            return ExitCode::from(2);
        }
    };
    match out {
        Some(p) => {
            if let Err(e) = std::fs::write(p, serialised.as_bytes()) {
                eprintln!("error writing {}: {e}", p.display());
                return ExitCode::from(2);
            }
            eprintln!(
                "wrote bundle to {} (chain length: {}, audit_event_id: {})",
                p.display(),
                bundle.audit_chain_segment.len(),
                bundle.audit_event.event.id
            );
        }
        None => {
            println!("{serialised}");
        }
    }
    ExitCode::SUCCESS
}

fn cmd_audit_verify_bundle(path: &Path) -> ExitCode {
    let data = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {e}", path.display());
            return ExitCode::from(2);
        }
    };
    let bundle: AuditBundle = match serde_json::from_str(&data) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("invalid bundle JSON: {e}");
            return ExitCode::from(2);
        }
    };
    match audit_bundle::verify(&bundle) {
        Ok(summary) => {
            println!(
                "ok: bundle verified (decision={:?}, deny_code={:?}, chain_length={}, audit_event_id={})",
                summary.decision,
                summary.deny_code,
                summary.audit_chain_length,
                summary.audit_event_id
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("bundle invalid: {e}");
            ExitCode::from(1)
        }
    }
}
