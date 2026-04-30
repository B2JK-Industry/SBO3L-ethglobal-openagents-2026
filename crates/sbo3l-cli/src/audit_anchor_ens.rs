//! `sbo3l audit anchor-ens` — write SBO3L's audit chain digest into
//! an ENS Public Resolver text record (`sbo3l:audit_root`).
//!
//! Three modes:
//! - `--dry-run` (default): build the envelope (namehash, calldata,
//!   resolver, audit_root) and print it. No network, no signing.
//! - `--offline-fixture <path>`: write the same envelope to disk
//!   for demo / CI fixture use. Default path
//!   `demo-fixtures/mock-ens-anchor.json`. No network.
//! - `--broadcast`: real broadcast to the supplied `--rpc-url` using
//!   the private key in the env var named by `--private-key-env-var`.
//!   **Not implemented in this PR** — gated behind a clear error
//!   message that points the operator at the dry-run for the same
//!   envelope content. A follow-up wires the broadcast path under a
//!   feature flag with mockito-fake-RPC tests so CI stays offline.
//!
//! Truthfulness rule: the dry-run is the *whole* artifact — anyone
//! with the same db + the same domain rebuilds bit-identical
//! calldata. That makes the dry-run a publishable demo on its own,
//! independent of whether broadcast eventually happens.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use chrono::Utc;
use sbo3l_identity::ens_anchor::{self, AnchorEnvelope, AnchorMode, AnchorParams, EnsNetwork};
use sbo3l_storage::audit_checkpoint_store::compute_chain_digest;
use sbo3l_storage::Storage;

/// CLI args carried verbatim from `main.rs` clap parsing. Kept as a
/// single struct so the dispatch line in `main.rs` is a one-liner.
#[derive(Debug, Clone)]
pub struct AnchorEnsArgs {
    pub db: PathBuf,
    pub domain: String,
    pub network: String,
    pub resolver: Option<String>,
    pub broadcast: bool,
    pub rpc_url: Option<String>,
    pub private_key_env_var: Option<String>,
    pub offline_fixture: Option<PathBuf>,
    pub out: Option<PathBuf>,
}

/// Default offline-fixture path. Intentionally relative to the repo
/// root so a `cargo run` from anywhere lands the same place.
pub const DEFAULT_OFFLINE_FIXTURE: &str = "demo-fixtures/mock-ens-anchor.json";

pub fn cmd_anchor_ens(args: AnchorEnsArgs) -> ExitCode {
    if args.broadcast {
        return broadcast_not_implemented(&args);
    }

    let mode = if args.offline_fixture.is_some() {
        AnchorMode::OfflineFixture
    } else {
        AnchorMode::DryRun
    };

    let network = match EnsNetwork::parse(&args.network) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("sbo3l audit anchor-ens: {e}");
            return ExitCode::from(2);
        }
    };

    let resolver_owned;
    let resolver: &str = match args.resolver.as_deref() {
        Some(s) => s,
        None => {
            resolver_owned = network.default_public_resolver().to_string();
            &resolver_owned
        }
    };

    // Read chain digest from the SQLite audit chain. Same algorithm
    // `sbo3l audit checkpoint create` uses — that's intentional: the
    // ENS-anchored audit_root and the mock-anchor checkpoint pin the
    // same byte string, so a third party can verify that the value
    // we wrote into ENS matches the local checkpoint.
    let audit_root = match read_chain_digest(&args.db) {
        Ok(d) => d,
        Err(rc) => return rc,
    };

    let now = Utc::now().to_rfc3339();
    let envelope = match ens_anchor::build_envelope(AnchorParams {
        network,
        domain: &args.domain,
        resolver,
        audit_root: &audit_root,
        mode,
        created_at: &now,
    }) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("sbo3l audit anchor-ens: {e}");
            return ExitCode::from(2);
        }
    };

    print_envelope(&envelope, mode);

    // Mode-specific side effects.
    match mode {
        AnchorMode::DryRun => {
            if let Some(out) = args.out.as_ref() {
                if let Err(rc) = write_json(&envelope, out, "anchor envelope") {
                    return rc;
                }
                say(format!("envelope written to {}", out.display()));
            }
        }
        AnchorMode::OfflineFixture => {
            // `--offline-fixture` always carries Some(path) thanks to
            // clap's default_value, so the unwrap below is safe.
            let path = args
                .offline_fixture
                .as_deref()
                .expect("AnchorMode::OfflineFixture set ⇒ args.offline_fixture must be Some");
            if let Err(rc) = write_json(&envelope, path, "offline fixture") {
                return rc;
            }
            say(format!("offline fixture written to {}", path.display()));
        }
    }

    ExitCode::SUCCESS
}

fn broadcast_not_implemented(args: &AnchorEnsArgs) -> ExitCode {
    eprintln!(
        "sbo3l audit anchor-ens: --broadcast is documented in the B3 brief but \
         NOT implemented in this build. The dry-run output (drop --broadcast) \
         contains the exact namehash, resolver, and calldata that would be sent. \
         Wiring the broadcast path is a follow-up gated on a Sepolia RPC URL + \
         signing key + mockito-fake-RPC test coverage so CI stays offline."
    );
    if args.rpc_url.is_some() || args.private_key_env_var.is_some() {
        eprintln!(
            "sbo3l audit anchor-ens: --rpc-url / --private-key-env-var were supplied \
             but ignored (broadcast not implemented)."
        );
    }
    ExitCode::from(2)
}

fn read_chain_digest(db: &Path) -> Result<String, ExitCode> {
    let storage = match Storage::open(db) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "sbo3l audit anchor-ens: open db {} failed: {e}",
                db.display()
            );
            return Err(ExitCode::from(1));
        }
    };

    let n = match storage.audit_count() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("sbo3l audit anchor-ens: audit_count: {e}");
            return Err(ExitCode::from(1));
        }
    };
    if n == 0 {
        eprintln!(
            "sbo3l audit anchor-ens: audit chain is empty in db {}; nothing to anchor. \
             Append at least one audit event before anchoring.",
            db.display()
        );
        return Err(ExitCode::from(3));
    }

    let hashes = match storage.audit_event_hashes_in_order() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("sbo3l audit anchor-ens: audit_event_hashes_in_order: {e}");
            return Err(ExitCode::from(1));
        }
    };
    compute_chain_digest(&hashes).map_err(|e| {
        eprintln!("sbo3l audit anchor-ens: chain digest failed: {e}");
        ExitCode::from(1)
    })
}

fn write_json(envelope: &AnchorEnvelope, path: &Path, what: &str) -> Result<(), ExitCode> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!(
                    "sbo3l audit anchor-ens: create_dir_all {} for {what} failed: {e}",
                    parent.display()
                );
                return Err(ExitCode::from(1));
            }
        }
    }
    let json = match serde_json::to_string_pretty(envelope) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sbo3l audit anchor-ens: serialise {what} failed: {e}");
            return Err(ExitCode::from(1));
        }
    };
    if let Err(e) = std::fs::write(path, json) {
        eprintln!(
            "sbo3l audit anchor-ens: write {} for {what} failed: {e}",
            path.display()
        );
        return Err(ExitCode::from(1));
    }
    Ok(())
}

/// Loud-disclosure prefix mirrors `audit checkpoint`. Both surfaces
/// emit one line per output field so a script can grep them out.
fn say(line: impl AsRef<str>) {
    println!("ens-anchor: {}", line.as_ref());
}

fn print_envelope(e: &AnchorEnvelope, mode: AnchorMode) {
    say(format!("mode:           {}", e.mode));
    say(format!("network:        {}", e.network));
    say(format!("domain:         {}", e.domain));
    say(format!("namehash:       0x{}", e.namehash));
    say(format!("resolver:       {}", e.resolver));
    say(format!("text_record:    {}", e.text_record_key));
    say(format!("audit_root:     0x{}", e.audit_root));
    say(format!("calldata:       0x{}", e.calldata));
    say(format!("created_at:     {}", e.created_at));
    say(format!("explanation:    {}", e.explanation));
    let _ = mode; // reserved for future per-mode header
}

#[cfg(test)]
mod tests {
    use super::*;
    use sbo3l_identity::ens_anchor::ENVELOPE_SCHEMA_ID;

    #[test]
    fn broadcast_flag_returns_explicit_not_implemented_exit_code() {
        let args = AnchorEnsArgs {
            db: PathBuf::from("/tmp/does-not-exist.db"),
            domain: "sbo3l.eth".into(),
            network: "sepolia".into(),
            resolver: None,
            broadcast: true,
            rpc_url: Some("http://example.invalid".into()),
            private_key_env_var: Some("SBO3L_ANCHOR_KEY".into()),
            offline_fixture: None,
            out: None,
        };
        // Routes through broadcast_not_implemented before any DB access.
        let code = cmd_anchor_ens(args);
        // ExitCode is opaque; we only assert it isn't SUCCESS.
        assert_ne!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
    }

    #[test]
    fn offline_fixture_writes_envelope_to_disk() {
        // Set up a real chain in a temp DB.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("chain.db");
        let mut storage = Storage::open(&db_path).unwrap();

        // Append one signed audit event so the chain has a tip.
        use sbo3l_core::signer::DevSigner;
        use sbo3l_storage::audit_store::NewAuditEvent;
        let signer = DevSigner::from_seed("anchor-ens-test-signer", [3u8; 32]);
        storage
            .audit_append(
                NewAuditEvent::now("policy_decided", "anchor-ens-test", "subj-1"),
                &signer,
            )
            .unwrap();

        let fixture_path = dir.path().join("mock-ens-anchor.json");
        let args = AnchorEnsArgs {
            db: db_path.clone(),
            domain: "sbo3l.eth".into(),
            network: "sepolia".into(),
            resolver: None,
            broadcast: false,
            rpc_url: None,
            private_key_env_var: None,
            offline_fixture: Some(fixture_path.clone()),
            out: None,
        };
        let code = cmd_anchor_ens(args);
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
        let raw = std::fs::read_to_string(&fixture_path).unwrap();
        let envelope: AnchorEnvelope = serde_json::from_str(&raw).unwrap();
        assert_eq!(envelope.schema, ENVELOPE_SCHEMA_ID);
        assert_eq!(envelope.mode, "offline_fixture");
        assert_eq!(envelope.network, "sepolia");
        assert_eq!(envelope.domain, "sbo3l.eth");
        assert_eq!(envelope.text_record_key, "sbo3l:audit_root");
        // selector + node + 2 offsets + length-word + key bytes (16 → padded 32)
        // + length-word + value bytes (64 → padded 64). The selector at the
        // front is what we're really pinning here.
        assert!(envelope.calldata.starts_with("10f13a8c"));
        // audit_root is a 64-char hex SHA-256.
        assert_eq!(envelope.audit_root.len(), 64);
    }

    #[test]
    fn empty_db_exits_with_code_3() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("empty.db");
        // Storage::open creates the schema but appends no events.
        let _ = Storage::open(&db_path).unwrap();

        let args = AnchorEnsArgs {
            db: db_path,
            domain: "sbo3l.eth".into(),
            network: "sepolia".into(),
            resolver: None,
            broadcast: false,
            rpc_url: None,
            private_key_env_var: None,
            offline_fixture: None,
            out: None,
        };
        let code = cmd_anchor_ens(args);
        // Code 3 is the same "nothing to anchor" path used by
        // audit checkpoint create; its exact representation is opaque
        // — assert it differs from SUCCESS.
        assert_ne!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));
    }
}
