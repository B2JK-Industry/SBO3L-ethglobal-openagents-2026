//! `sbo3l agent reputation-publish` — compute v2 score + emit a
//! `setText("sbo3l:reputation_score", "<n>")` envelope (T-4-6 / T-4-7).
//!
//! Two paths from the same command:
//!
//! * **Dry-run (default)** — build the envelope (calldata, namehash,
//!   resolver, score) and print as JSON. No network calls, no signing.
//!   The envelope is publishable on its own — same input always
//!   re-derives the same calldata, so an external auditor can replay
//!   the publisher and confirm without trusting SBO3L's reporting.
//! * **`--broadcast`** (T-4-7, requires `--features eth_broadcast`) —
//!   sign + send `setText` over JSON-RPC via the same alloy harness
//!   T-3-1 broadcast uses (`agent_broadcast.rs`). Mainnet path
//!   additionally requires `SBO3L_ALLOW_MAINNET_TX=1` plus an
//!   explicit `--network mainnet` — same double-gate the rest of
//!   SBO3L's chain ops use.
//!
//! Inputs: events JSON file (array of `ReputationEventInput`), FQDN,
//! network, optional resolver override. Output for dry-run: a
//! [`sbo3l_identity::ReputationPublishEnvelope`] printed to stdout
//! and (optionally) written to `--out`. Output for broadcast: the
//! tx hash + Etherscan link printed to stdout once the tx confirms.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use sbo3l_identity::ens_anchor::EnsNetwork;
use sbo3l_identity::reputation_publisher::{
    build_publish_envelope, PublishMode, ReputationEventInput, ReputationPublishParams,
};

/// Default resolver address per network. Same well-known PublicResolver
/// constants `sbo3l-identity::EnsNetwork::default_public_resolver` exposes.
fn default_resolver(network: EnsNetwork) -> &'static str {
    network.default_public_resolver()
}

#[derive(Debug, Clone)]
pub struct ReputationPublishArgs {
    pub fqdn: String,
    pub events: PathBuf,
    pub network: String,
    pub resolver: Option<String>,
    pub out: Option<PathBuf>,
    /// Sign + send the `setText` tx instead of just printing the
    /// envelope. Requires the `eth_broadcast` Cargo feature; without
    /// it the dispatch falls through to a clear "rebuild with
    /// --features eth_broadcast" error (exit code 3, matching
    /// T-3-1 broadcast's stub behaviour).
    pub broadcast: bool,
    /// JSON-RPC URL override. If unset, the broadcast path reads
    /// `SBO3L_RPC_URL`. Validated http/https.
    pub rpc_url: Option<String>,
    /// Override the env var that holds the 32-byte hex private key
    /// (default `SBO3L_SIGNER_KEY`). Same pattern T-3-1 broadcast uses.
    pub private_key_env_var: Option<String>,
}

pub fn cmd_agent_reputation_publish(args: ReputationPublishArgs) -> ExitCode {
    if args.broadcast {
        return broadcast_dispatch(args);
    }
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

/// `--broadcast` dispatch — selects between the live `eth_broadcast`
/// implementation (compiled with `--features eth_broadcast`) and the
/// honest "rebuild with the feature" stub. Exit code 3 matches the
/// T-3-1 broadcast contract for the stub branch.
fn broadcast_dispatch(args: ReputationPublishArgs) -> ExitCode {
    #[cfg(feature = "eth_broadcast")]
    {
        let network = match args.network.as_str() {
            "mainnet" => sbo3l_identity::ens_anchor::EnsNetwork::Mainnet,
            "sepolia" => sbo3l_identity::ens_anchor::EnsNetwork::Sepolia,
            other => {
                eprintln!(
                    "sbo3l agent reputation-publish --broadcast: unsupported network '{other}'. \
                     Expected 'mainnet' or 'sepolia'."
                );
                return ExitCode::from(2);
            }
        };
        // Mainnet double-gate: requires SBO3L_ALLOW_MAINNET_TX=1.
        if matches!(network, sbo3l_identity::ens_anchor::EnsNetwork::Mainnet)
            && std::env::var("SBO3L_ALLOW_MAINNET_TX").as_deref() != Ok("1")
        {
            eprintln!(
                "sbo3l agent reputation-publish --broadcast --network mainnet: \
                 refusing without SBO3L_ALLOW_MAINNET_TX=1. Mainnet broadcast \
                 costs gas (~$3-5 per tx at 50 gwei). Set the env var to \
                 acknowledge before re-running."
            );
            return ExitCode::from(2);
        }
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!(
                    "sbo3l agent reputation-publish --broadcast: tokio runtime init failed: {e}"
                );
                return ExitCode::from(1);
            }
        };
        rt.block_on(crate::agent_reputation_broadcast::cmd_broadcast(
            args, network,
        ))
    }
    #[cfg(not(feature = "eth_broadcast"))]
    {
        broadcast_not_available(&args)
    }
}

#[cfg(not(feature = "eth_broadcast"))]
fn broadcast_not_available(args: &ReputationPublishArgs) -> ExitCode {
    eprintln!(
        "sbo3l agent reputation-publish: --broadcast was accepted but this build \
         was compiled without `--features eth_broadcast`. The dry-run \
         output (drop --broadcast) is the complete envelope; pipe its \
         calldata to `cast send`, or rebuild with \
         `cargo install sbo3l-cli --features eth_broadcast` (pulls the \
         alloy stack)."
    );
    if args.rpc_url.is_some() || args.private_key_env_var.is_some() {
        eprintln!(
            "  --rpc-url / --private-key-env-var were accepted but ignored \
             (broadcast feature not enabled in this build)."
        );
    }
    if args.network == "mainnet" {
        eprintln!(
            "  Mainnet path will additionally require SBO3L_ALLOW_MAINNET_TX=1 \
             and an explicit --network mainnet at broadcast time."
        );
    }
    ExitCode::from(3)
}

fn run(args: ReputationPublishArgs) -> Result<(), ExitCode> {
    let network = match args.network.as_str() {
        "mainnet" => EnsNetwork::Mainnet,
        "sepolia" => EnsNetwork::Sepolia,
        other => {
            eprintln!("error: unsupported network '{other}'. Expected 'mainnet' or 'sepolia'.");
            return Err(ExitCode::from(2));
        }
    };

    let resolver = args
        .resolver
        .as_deref()
        .unwrap_or_else(|| default_resolver(network));

    let raw = fs::read_to_string(&args.events).map_err(|e| {
        eprintln!("error: read events file {}: {e}", args.events.display());
        ExitCode::from(2)
    })?;
    let events: Vec<ReputationEventInput> = serde_json::from_str(&raw).map_err(|e| {
        eprintln!("error: parse events file {}: {e}", args.events.display());
        ExitCode::from(2)
    })?;

    let created_at = current_rfc3339();
    let envelope = build_publish_envelope(
        ReputationPublishParams {
            network,
            domain: &args.fqdn,
            resolver,
            created_at: &created_at,
            mode: PublishMode::DryRun,
        },
        &events,
    )
    .map_err(|e| {
        eprintln!("error: build publish envelope: {e}");
        ExitCode::from(2)
    })?;

    let json = serde_json::to_string_pretty(&envelope).map_err(|e| {
        eprintln!("error: serialise envelope: {e}");
        ExitCode::from(2)
    })?;

    println!("{json}");

    if let Some(path) = args.out {
        fs::write(&path, &json).map_err(|e| {
            eprintln!("error: write {}: {e}", path.display());
            ExitCode::from(2)
        })?;
        eprintln!("wrote {}", path.display());
    }

    Ok(())
}

/// Best-effort RFC-3339 of "now". Falls back to a stable empty
/// string if the system clock is somehow unreadable — the envelope
/// remains structurally valid (just lossy on the timestamp), and
/// downstream consumers can re-stamp from the receipt that pins it.
fn current_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Tiny in-tree formatter that doesn't pull chrono into the CLI
    // for one timestamp. Delegates to the workspace's existing
    // chrono presence via the storage crate's dep tree.
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, 0)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_events(events: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(events.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn happy_path_prints_envelope() {
        let events = r#"[
            {"decision":"allow","executor_confirmed":true,"age_secs":0},
            {"decision":"deny","executor_confirmed":false,"age_secs":86400}
        ]"#;
        let f = write_events(events);
        let res = run(ReputationPublishArgs {
            fqdn: "research-agent.sbo3lagent.eth".to_string(),
            events: f.path().to_path_buf(),
            network: "mainnet".to_string(),
            resolver: None,
            out: None,
            broadcast: false,
            rpc_url: None,
            private_key_env_var: None,
        });
        assert!(res.is_ok());
    }

    #[test]
    fn unsupported_network_returns_exit2() {
        let f = write_events("[]");
        let res = run(ReputationPublishArgs {
            fqdn: "x.eth".to_string(),
            events: f.path().to_path_buf(),
            network: "polygon".to_string(),
            resolver: None,
            out: None,
            broadcast: false,
            rpc_url: None,
            private_key_env_var: None,
        });
        assert!(res.is_err());
    }

    #[test]
    fn malformed_events_returns_exit2() {
        let f = write_events("not json");
        let res = run(ReputationPublishArgs {
            fqdn: "x.eth".to_string(),
            events: f.path().to_path_buf(),
            network: "mainnet".to_string(),
            resolver: None,
            out: None,
            broadcast: false,
            rpc_url: None,
            private_key_env_var: None,
        });
        assert!(res.is_err());
    }

    #[test]
    fn writes_out_file_when_provided() {
        let events = r#"[{"decision":"allow","executor_confirmed":true,"age_secs":0}]"#;
        let f = write_events(events);
        let out = NamedTempFile::new().unwrap();
        let res = run(ReputationPublishArgs {
            fqdn: "research-agent.sbo3lagent.eth".to_string(),
            events: f.path().to_path_buf(),
            network: "mainnet".to_string(),
            resolver: None,
            out: Some(out.path().to_path_buf()),
            broadcast: false,
            rpc_url: None,
            private_key_env_var: None,
        });
        assert!(res.is_ok());
        let written = std::fs::read_to_string(out.path()).unwrap();
        assert!(written.contains("\"text_record_key\": \"sbo3l:reputation_score\""));
        assert!(written.contains("\"score\":"));
    }

    #[cfg(not(feature = "eth_broadcast"))]
    #[test]
    fn broadcast_without_feature_returns_exit3() {
        let events = r#"[{"decision":"allow","executor_confirmed":true,"age_secs":0}]"#;
        let f = write_events(events);
        let code = cmd_agent_reputation_publish(ReputationPublishArgs {
            fqdn: "research-agent.sbo3lagent.eth".to_string(),
            events: f.path().to_path_buf(),
            network: "sepolia".to_string(),
            resolver: None,
            out: None,
            broadcast: true,
            rpc_url: Some("https://example.invalid".to_string()),
            private_key_env_var: None,
        });
        // ExitCode doesn't expose its inner u8 in stable Rust; format
        // and inspect via Debug. The contract is "non-zero, specifically
        // 3 to mirror T-3-1 broadcast's stub exit." Format lands as
        // `ExitCode(unix_exit_status(3))` on Linux/macOS — pin that.
        let formatted = format!("{code:?}");
        assert!(
            formatted.contains('3'),
            "expected exit code 3 for broadcast-without-feature; got {formatted:?}"
        );
    }
}
