use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "mandate",
    version,
    about = "Mandate — spending mandates for autonomous agents.",
    long_about = "Mandate is a local policy, budget, receipt and audit firewall for AI agents."
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
        path: std::path::PathBuf,
    },
    /// Print the schema id for a wire format
    Schema {
        /// One of: aprp | policy | decision-token | policy-receipt | audit-event
        kind: String,
    },
}

#[derive(Subcommand, Debug)]
enum AprpCmd {
    /// Validate an APRP JSON document against schemas/aprp_v1.json
    Validate {
        /// Path to the APRP JSON file
        path: std::path::PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
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
        } => {
            println!("validate not yet implemented: {}", path.display());
            Ok(())
        }
        Command::VerifyAudit { path } => {
            println!("verify-audit not yet implemented: {}", path.display());
            Ok(())
        }
        Command::Schema { kind } => {
            println!("schema not yet implemented: {kind}");
            Ok(())
        }
    }
}
