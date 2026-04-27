use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use mandate_core::{schema, SchemaError};

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
    /// Verify a Mandate audit hash chain
    VerifyAudit {
        /// Path to a JSONL audit log
        #[arg(long)]
        path: PathBuf,
    },
    /// Print the schema id for a wire format
    Schema {
        /// One of: aprp | policy | decision-token | policy-receipt | audit-event | x402
        kind: String,
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
        Command::VerifyAudit { path: _ } => {
            eprintln!("verify-audit: not yet implemented");
            ExitCode::from(2)
        }
        Command::Schema { kind } => cmd_schema(&kind),
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
