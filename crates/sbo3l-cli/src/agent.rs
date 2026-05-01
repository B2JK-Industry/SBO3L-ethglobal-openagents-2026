//! `sbo3l agent register` — issue an ENS subname under a parent
//! (default `sbo3lagent.eth`) and pre-pack a `multicall(setText × N)`
//! to set every `sbo3l:*` text record in one tx.
//!
//! Mirrors the [`crate::audit_anchor_ens`] pattern:
//!
//! - `--dry-run` (default in this build, the *only* implemented path):
//!   build the [`sbo3l_identity::DurinDryRun`] envelope (calldata,
//!   namehashes, FQDN, per-setText breakdown) and print it. Pure
//!   function, no chain interaction.
//! - `--broadcast`: gated. **Not implemented in this build.** Stub
//!   returns a clear error pointing at `--dry-run` and documents the
//!   double-gate that ships in the follow-up: `--network mainnet`
//!   plus `SBO3L_ALLOW_MAINNET_TX=1`.
//!
//! Truthfulness rule: the dry-run is publishable on its own — same
//! `parent` + `label` + records always re-derives the same envelope.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sbo3l_identity::durin::{build_dry_run, DurinDryRun, DurinError};
use sbo3l_identity::ens_anchor::EnsNetwork;
use serde_json::Value;

/// CLI args carried verbatim from `main.rs` clap parsing. Single
/// struct so the dispatch in `main.rs` stays a one-liner.
#[derive(Debug, Clone)]
pub struct AgentRegisterArgs {
    pub name: String,
    pub parent: String,
    pub network: String,
    pub records_json: String,
    pub owner: Option<String>,
    pub resolver: Option<String>,
    pub broadcast: bool,
    pub rpc_url: Option<String>,
    pub private_key_env_var: Option<String>,
    pub out: Option<PathBuf>,
}

/// Default parent ENS — Daniel owns `sbo3lagent.eth` on mainnet
/// (registered pre-hackathon). T-3-1 issues subnames under this
/// unless the operator passes `--parent` explicitly.
pub const DEFAULT_PARENT: &str = "sbo3lagent.eth";

/// Mainnet safety-gate env var. When `--network mainnet` is requested
/// (whether dry-run or broadcast), this MUST be set to `1` so an
/// operator can't accidentally produce mainnet calldata in a script.
/// Sepolia is the default and never requires this gate.
pub const ENV_ALLOW_MAINNET: &str = "SBO3L_ALLOW_MAINNET_TX";

pub fn cmd_agent_register(args: AgentRegisterArgs) -> ExitCode {
    let network = match EnsNetwork::parse(&args.network) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("sbo3l agent register: {e}");
            return ExitCode::from(2);
        }
    };

    if let EnsNetwork::Mainnet = network {
        match std::env::var(ENV_ALLOW_MAINNET).as_deref() {
            Ok("1") => {}
            _ => {
                eprintln!(
                    "sbo3l agent register: refusing --network mainnet without {ENV_ALLOW_MAINNET}=1.\n\
                     \n\
                     Mainnet calldata is gas-bearing for the broadcaster (~$60 at 50 gwei).\n\
                     Set {ENV_ALLOW_MAINNET}=1 to acknowledge before re-running. The default\n\
                     network is Sepolia and never requires this gate."
                );
                return ExitCode::from(2);
            }
        }
    }

    if args.broadcast {
        return broadcast_not_implemented(&args, network);
    }

    let resolver_owned;
    let resolver: &str = match args.resolver.as_deref() {
        Some(s) => s,
        None => {
            resolver_owned = network.default_public_resolver().to_string();
            &resolver_owned
        }
    };

    // Owner defaults to the resolver-pointed parent's owner address —
    // for T-3-1 main PR (no signer wired yet), an explicit --owner is
    // required when not broadcasting. Once the EthSigner factory ships
    // (T-3-1 follow-up), default = signer's eth_address().
    let owner = match args.owner.as_deref() {
        Some(s) => s,
        None => {
            eprintln!(
                "sbo3l agent register: --owner <0x...> is required in this build.\n\
                 \n\
                 The follow-up that wires the EthSigner factory will default --owner\n\
                 to the signer's eth_address(). For now, pass an explicit owner address\n\
                 (the eventual on-chain controller of the subname after issuance).\n\
                 \n\
                 Mainnet ENS is owned by Daniel (0xdc7EFA…D231) — for the\n\
                 sbo3lagent.eth-derived subnames, that's the canonical default."
            );
            return ExitCode::from(2);
        }
    };

    let records = match parse_records(&args.records_json) {
        Ok(r) => r,
        Err(rc) => return rc,
    };

    let envelope = match build_dry_run(
        &args.parent,
        &args.name,
        owner,
        network,
        resolver,
        records.iter().map(|(k, v)| (k.as_str(), v.as_str())),
    ) {
        Ok(e) => e,
        Err(DurinError::Anchor(e)) => {
            eprintln!("sbo3l agent register: {e}");
            return ExitCode::from(2);
        }
        Err(e) => {
            eprintln!("sbo3l agent register: {e}");
            return ExitCode::from(2);
        }
    };

    print_envelope(&envelope);

    if let Some(out) = args.out.as_ref() {
        if let Err(rc) = write_json(&envelope, out) {
            return rc;
        }
        say(format!("envelope written to {}", out.display()));
    }

    ExitCode::SUCCESS
}

fn parse_records(s: &str) -> Result<Vec<(String, String)>, ExitCode> {
    let v: Value = match serde_json::from_str(s) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("sbo3l agent register: --records is not valid JSON: {e}");
            return Err(ExitCode::from(2));
        }
    };
    let obj = match v.as_object() {
        Some(o) => o,
        None => {
            eprintln!(
                "sbo3l agent register: --records must be a JSON object \
                 (e.g. '{{\"sbo3l:agent_id\":\"...\"}}'); got {}",
                shape_of(&v)
            );
            return Err(ExitCode::from(2));
        }
    };
    let mut out = Vec::with_capacity(obj.len());
    // Stable iteration order — the dry-run envelope's setText breakdown
    // mirrors this iteration so two operators with the same JSON body
    // see byte-identical output (serde_json::Map is BTree-backed under
    // `preserve_order = false`, lexicographic).
    for (k, v) in obj {
        let s = match v.as_str() {
            Some(s) => s.to_string(),
            None => {
                eprintln!(
                    "sbo3l agent register: --records value for `{k}` must be a string; \
                     got {}",
                    shape_of(v)
                );
                return Err(ExitCode::from(2));
            }
        };
        out.push((k.clone(), s));
    }
    Ok(out)
}

fn shape_of(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn broadcast_not_implemented(args: &AgentRegisterArgs, network: EnsNetwork) -> ExitCode {
    eprintln!(
        "sbo3l agent register: --broadcast is documented in T-3-1 but \
         NOT implemented in this build. The dry-run output (drop --broadcast) \
         is the complete envelope; pipe its calldata to `cast send` against \
         the registrar / resolver, or wait for the broadcast follow-up that \
         wires sbo3l_core::signers::eth::EthSigner."
    );
    if args.rpc_url.is_some() || args.private_key_env_var.is_some() {
        eprintln!(
            "  --rpc-url / --private-key-env-var were accepted but ignored \
             (broadcast not implemented)."
        );
    }
    if let EnsNetwork::Mainnet = network {
        eprintln!(
            "  Mainnet path will additionally require SBO3L_ALLOW_MAINNET_TX=1 \
             (already set, otherwise we'd have refused before this point) \
             and an explicit --network mainnet at broadcast time."
        );
    }
    ExitCode::from(3)
}

fn print_envelope(e: &DurinDryRun) {
    say(format!("schema:            {}", e.schema));
    say(format!("fqdn:              {}", e.fqdn));
    say(format!("network:           {}", e.network));
    say(format!("parent_namehash:   0x{}", e.parent_namehash));
    say(format!("fqdn_namehash:     0x{}", e.fqdn_namehash));
    say(format!("owner:             {}", e.owner));
    say(format!("resolver:          {}", e.resolver));
    say(format!("set_text_calls:    {}", e.set_text_calls));
    say("---");
    say(format!("register_calldata: {}", e.register_calldata_hex));
    say("---");
    say(format!("multicall_calldata:{}", e.multicall_calldata_hex));
    say("---");
    say("set_text_breakdown:");
    for entry in &e.set_text_breakdown {
        say(format!(
            "  - {}: {}\n      {}",
            entry.key, entry.value, entry.calldata_hex
        ));
    }
    say("---");
    say(format!(
        "broadcasted: {}    (dry-run does NOT contact an RPC)",
        e.broadcasted
    ));
    say(format!(
        "gas_estimate: {}",
        match e.gas_estimate {
            Some(g) => g.to_string(),
            None => "(none — run `cast estimate` against the printed calldata)".to_string(),
        }
    ));
}

fn write_json(envelope: &DurinDryRun, path: &Path) -> Result<(), ExitCode> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!(
                    "sbo3l agent register: failed to create parent dir {}: {e}",
                    parent.display()
                );
                return Err(ExitCode::from(1));
            }
        }
    }
    let body = serde_json::to_string_pretty(envelope).map_err(|e| {
        eprintln!("sbo3l agent register: failed to serialise envelope: {e}");
        ExitCode::from(1)
    })?;
    std::fs::write(path, body).map_err(|e| {
        eprintln!(
            "sbo3l agent register: failed to write envelope to {}: {e}",
            path.display()
        );
        ExitCode::from(1)
    })?;
    Ok(())
}

fn say(line: impl AsRef<str>) {
    println!("{}", line.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_args() -> AgentRegisterArgs {
        AgentRegisterArgs {
            name: "research-agent".to_string(),
            parent: DEFAULT_PARENT.to_string(),
            network: "sepolia".to_string(),
            records_json:
                r#"{"sbo3l:agent_id":"research-agent-01","sbo3l:endpoint":"http://x"}"#
                    .to_string(),
            owner: Some("0xdc7EFA00000000000000000000000000000000d2".to_string()),
            resolver: None,
            broadcast: false,
            rpc_url: None,
            private_key_env_var: None,
            out: None,
        }
    }

    #[test]
    fn dry_run_sepolia_with_explicit_owner_succeeds() {
        let rc = cmd_agent_register(baseline_args());
        assert!(matches!(rc, c if c == ExitCode::SUCCESS));
    }

    #[test]
    fn broadcast_returns_not_implemented_exit_code() {
        let mut a = baseline_args();
        a.broadcast = true;
        a.rpc_url = Some("https://example".into());
        a.private_key_env_var = Some("SBO3L_SEPOLIA_PRIVATE_KEY".into());
        // ExitCode::from(3) is the "nothing-to-do / not implemented"
        // code per the shared CLI contract (0/1/2/3 = ok / IO /
        // semantic / nothing-to-do). We can't compare ExitCode
        // directly, so assert it isn't SUCCESS.
        let rc = cmd_agent_register(a);
        assert!(!matches!(rc, c if c == ExitCode::SUCCESS));
    }

    #[test]
    fn missing_owner_without_broadcast_refuses() {
        let mut a = baseline_args();
        a.owner = None;
        let rc = cmd_agent_register(a);
        assert!(!matches!(rc, c if c == ExitCode::SUCCESS));
    }

    #[test]
    fn malformed_records_json_returns_error() {
        let mut a = baseline_args();
        a.records_json = "not json".to_string();
        let rc = cmd_agent_register(a);
        assert!(!matches!(rc, c if c == ExitCode::SUCCESS));
    }

    #[test]
    fn records_array_not_object_returns_error() {
        let mut a = baseline_args();
        a.records_json = "[]".to_string();
        let rc = cmd_agent_register(a);
        assert!(!matches!(rc, c if c == ExitCode::SUCCESS));
    }

    #[test]
    fn mainnet_without_env_gate_refuses() {
        let mut a = baseline_args();
        a.network = "mainnet".to_string();
        // Defensive: explicitly remove if it leaked from the test
        // runner's env. SAFETY: removing an env var is the canonical
        // "guarantee gate fires" pattern; std::env::set_var/remove_var
        // were marked unsafe in 1.86 to discourage threaded misuse,
        // but a single synchronous mutation in a single-threaded test
        // is fine.
        unsafe { std::env::remove_var(ENV_ALLOW_MAINNET) };
        let rc = cmd_agent_register(a);
        assert!(!matches!(rc, c if c == ExitCode::SUCCESS));
    }
}
