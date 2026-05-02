//! `sbo3l agent reputation-publish` — compute v2 score + emit a
//! `setText("sbo3l:reputation_score", "<n>")` envelope (T-4-6).
//!
//! Inputs: events JSON file (array of `ReputationEventInput`), FQDN,
//! network, optional resolver override. Output: a
//! [`sbo3l_identity::ReputationPublishEnvelope`] printed to stdout
//! and (optionally) written to `--out`.
//!
//! Dry-run only in this build. Broadcast wires through F-5
//! EthSigner once that lands. The dry-run envelope is publishable
//! on its own — same input always re-derives the same calldata,
//! so an external auditor can replay the publisher and confirm.

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
}

pub fn cmd_agent_reputation_publish(args: ReputationPublishArgs) -> ExitCode {
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
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
        });
        assert!(res.is_ok());
        let written = std::fs::read_to_string(out.path()).unwrap();
        assert!(written.contains("\"text_record_key\": \"sbo3l:reputation_score\""));
        assert!(written.contains("\"score\":"));
    }
}
