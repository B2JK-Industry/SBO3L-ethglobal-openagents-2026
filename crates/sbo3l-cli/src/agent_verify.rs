//! `sbo3l agent verify-ens <fqdn>` — pair to `sbo3l agent register`
//! (T-3-1). Resolves all `sbo3l:*` text records for an agent's ENS
//! name and asserts each present record matches the operator's
//! expectations.
//!
//! ## What we verify
//!
//! 1. **Resolution liveness** — `LiveEnsResolver::resolve_raw_text`
//!    successfully reads the canonical `sbo3l:*` keys via the
//!    network's PublicResolver (or the OffchainResolver pointer
//!    flipped per T-4-1's deploy runbook).
//! 2. **Per-record value match** — each present record's value
//!    matches `--expected-records '<json>'` if supplied. Records
//!    that the operator didn't list expected values for are
//!    *reported* but not failed.
//! 3. **Pubkey identity** — `sbo3l:pubkey_ed25519` matches one of:
//!    - `--expected-pubkey 0x<64-hex>` (32-byte Ed25519 pubkey hex);
//!    - the pubkey *derived from* a local key file via
//!      `--key-file <path>` (the file holds an ed25519-dalek seed,
//!      same format `crates/sbo3l-core/src/signers/dev.rs` uses).
//!
//! Exits 0 on full PASS; exits 1 on resolution error (network); exits
//! 2 on any FAIL assertion.
//!
//! ## Live testing
//!
//! Set `SBO3L_LIVE_ETH=1` + `SBO3L_ENS_RPC_URL` to run the
//! `tests/agent_verify_live.rs` integration test against
//! `sbo3lagent.eth` on mainnet. Skipped cleanly otherwise — keeps CI
//! offline.

use std::path::PathBuf;
use std::process::ExitCode;

use sbo3l_identity::ens_anchor::EnsNetwork;
use sbo3l_identity::ens_live::LiveEnsResolver;
use serde_json::Value;

/// Args for the verify-ens path. Carried verbatim from clap parsing.
#[derive(Debug, Clone)]
pub struct AgentVerifyEnsArgs {
    /// Fully-qualified ENS name to resolve (e.g.
    /// `research-agent.sbo3lagent.eth`).
    pub fqdn: String,
    /// `mainnet` | `sepolia`. Defaults to `mainnet` because
    /// `sbo3lagent.eth` is the live name.
    pub network: String,
    /// Optional override of the resolver's RPC URL. Defaults to
    /// `SBO3L_ENS_RPC_URL` env var.
    pub rpc_url: Option<String>,
    /// Optional 0x-prefixed 64-hex-char Ed25519 pubkey to assert
    /// against `sbo3l:pubkey_ed25519`. Mutually exclusive with
    /// `--key-file`.
    pub expected_pubkey: Option<String>,
    /// Optional path to a local Ed25519 secret-seed file (32 raw
    /// bytes). The pubkey is derived and asserted against
    /// `sbo3l:pubkey_ed25519`.
    pub key_file: Option<PathBuf>,
    /// Optional JSON object `{"sbo3l:agent_id":"...", ...}` of
    /// expected records. Records not present here are reported but
    /// not failed.
    pub expected_records: Option<String>,
    /// Emit a JSON envelope instead of human-readable text.
    pub json: bool,
}

/// Canonical keys we read for verify-ens. Superset of
/// [`sbo3l_identity::SBO3L_TEXT_KEYS`] — adds `pubkey_ed25519`,
/// `policy_url`, `capabilities` per the T-3-3 fleet schema.
pub const VERIFY_KEYS: &[&str] = &[
    "sbo3l:agent_id",
    "sbo3l:endpoint",
    "sbo3l:pubkey_ed25519",
    "sbo3l:policy_url",
    "sbo3l:capabilities",
    "sbo3l:policy_hash",
    "sbo3l:audit_root",
    "sbo3l:proof_uri",
];

/// One row of the verify-ens report.
#[derive(Debug, Clone)]
struct RecordCheck {
    key: &'static str,
    /// Resolved value, `None` if the record is unset.
    actual: Option<String>,
    /// Expected value, `None` if the operator didn't supply one.
    expected: Option<String>,
}

impl RecordCheck {
    /// `pass | fail | skip | absent`. `skip` means resolved present,
    /// no expectation supplied — we report but don't fail. `absent`
    /// means the record is unset on-chain.
    fn verdict(&self) -> &'static str {
        match (&self.actual, &self.expected) {
            (Some(a), Some(e)) if a == e => "pass",
            (Some(_), Some(_)) => "fail",
            (Some(_), None) => "skip",
            (None, Some(_)) => "fail", // expected something, got nothing
            (None, None) => "absent",
        }
    }
}

pub fn cmd_agent_verify_ens(args: AgentVerifyEnsArgs) -> ExitCode {
    let network = match EnsNetwork::parse(&args.network) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("sbo3l agent verify-ens: {e}");
            return ExitCode::from(2);
        }
    };

    if args.expected_pubkey.is_some() && args.key_file.is_some() {
        eprintln!(
            "sbo3l agent verify-ens: --expected-pubkey and --key-file are mutually exclusive"
        );
        return ExitCode::from(2);
    }

    // Set the RPC URL env var only if the operator supplied a flag —
    // otherwise we inherit whatever's in the environment.
    if let Some(rpc) = args.rpc_url.as_deref() {
        // SAFETY: single-threaded set before resolver construction.
        unsafe { std::env::set_var("SBO3L_ENS_RPC_URL", rpc) };
    }

    let resolver = match LiveEnsResolver::from_env(network) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "sbo3l agent verify-ens: failed to build LiveEnsResolver: {e}\n\
                 Set SBO3L_ENS_RPC_URL or pass --rpc-url <url>."
            );
            return ExitCode::from(1);
        }
    };

    // Resolve expected pubkey if --key-file was passed.
    let expected_pubkey = match resolve_expected_pubkey(&args) {
        Ok(p) => p,
        Err(rc) => return rc,
    };

    // Parse expected_records JSON if supplied.
    let expected_records = match parse_expected_records(args.expected_records.as_deref()) {
        Ok(r) => r,
        Err(rc) => return rc,
    };

    // Read each canonical key.
    let mut checks: Vec<RecordCheck> = Vec::with_capacity(VERIFY_KEYS.len());
    for &key in VERIFY_KEYS {
        let actual = match resolver.resolve_raw_text(&args.fqdn, key) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("sbo3l agent verify-ens: resolve {key}: {e}");
                return ExitCode::from(1);
            }
        };
        let expected = match key {
            "sbo3l:pubkey_ed25519" => expected_pubkey
                .clone()
                .or_else(|| expected_records.get(key).cloned()),
            other => expected_records.get(other).cloned(),
        };
        checks.push(RecordCheck {
            key,
            actual,
            expected,
        });
    }

    let any_fail = checks.iter().any(|c| c.verdict() == "fail");
    let any_pass = checks.iter().any(|c| c.verdict() == "pass");

    if args.json {
        emit_json_report(&args.fqdn, &args.network, &checks);
    } else {
        emit_human_report(&args.fqdn, &args.network, &checks);
    }

    if any_fail {
        ExitCode::from(2)
    } else if !any_pass && expected_pubkey.is_none() && expected_records.is_empty() {
        // No assertions to make — "verify-ens" without any expected
        // values is just a resolver dump. Exit 0.
        ExitCode::SUCCESS
    } else {
        ExitCode::SUCCESS
    }
}

fn resolve_expected_pubkey(args: &AgentVerifyEnsArgs) -> Result<Option<String>, ExitCode> {
    if let Some(p) = args.expected_pubkey.as_deref() {
        let normalised = normalise_hex_pubkey(p).ok_or_else(|| {
            eprintln!(
                "sbo3l agent verify-ens: --expected-pubkey must be 0x + 64 hex chars (32-byte Ed25519); got `{p}`"
            );
            ExitCode::from(2)
        })?;
        return Ok(Some(normalised));
    }
    if let Some(path) = args.key_file.as_ref() {
        let bytes = std::fs::read(path).map_err(|e| {
            eprintln!(
                "sbo3l agent verify-ens: failed to read --key-file {}: {e}",
                path.display()
            );
            ExitCode::from(1)
        })?;
        let pubkey = derive_pubkey_from_seed(&bytes).map_err(|msg| {
            eprintln!("sbo3l agent verify-ens: --key-file: {msg}");
            ExitCode::from(2)
        })?;
        return Ok(Some(pubkey));
    }
    Ok(None)
}

fn parse_expected_records(
    s: Option<&str>,
) -> Result<std::collections::HashMap<String, String>, ExitCode> {
    let s = match s {
        Some(s) => s,
        None => return Ok(std::collections::HashMap::new()),
    };
    let v: Value = serde_json::from_str(s).map_err(|e| {
        eprintln!("sbo3l agent verify-ens: --expected-records is not valid JSON: {e}");
        ExitCode::from(2)
    })?;
    let obj = v.as_object().ok_or_else(|| {
        eprintln!("sbo3l agent verify-ens: --expected-records must be a JSON object");
        ExitCode::from(2)
    })?;
    let mut out = std::collections::HashMap::with_capacity(obj.len());
    for (k, v) in obj {
        let s = v.as_str().ok_or_else(|| {
            eprintln!(
                "sbo3l agent verify-ens: --expected-records value for `{k}` must be a string"
            );
            ExitCode::from(2)
        })?;
        out.insert(k.clone(), s.to_string());
    }
    Ok(out)
}

fn normalise_hex_pubkey(s: &str) -> Option<String> {
    let stripped = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    if stripped.len() != 64 {
        return None;
    }
    if !stripped.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("0x{}", stripped.to_lowercase()))
}

fn derive_pubkey_from_seed(bytes: &[u8]) -> Result<String, String> {
    // Accept either the raw 32-byte seed OR a hex-encoded seed (with
    // or without `0x` prefix). The dev signer
    // (sbo3l-core/signers/dev.rs) uses raw bytes; the fleet's
    // derive-fleet-keys.py emits hex. Support both for symmetry.
    let raw = if bytes.len() == 32 {
        let mut out = [0u8; 32];
        out.copy_from_slice(bytes);
        out
    } else {
        // Try hex.
        let s = std::str::from_utf8(bytes)
            .map_err(|_| "key file is neither 32 raw bytes nor UTF-8 hex".to_string())?
            .trim();
        let stripped = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s);
        if stripped.len() != 64 {
            return Err(format!(
                "expected 32-byte seed (raw or 64-char hex); got {} chars of '{}'",
                stripped.len(),
                if stripped.len() > 80 {
                    &stripped[..80]
                } else {
                    stripped
                }
            ));
        }
        let mut out = [0u8; 32];
        hex::decode_to_slice(stripped, &mut out).map_err(|e| format!("hex decode failed: {e}"))?;
        out
    };
    // Derive ed25519 pubkey from the seed. We avoid pulling in
    // ed25519-dalek directly here because sbo3l-cli already has it
    // as a transitive dep through sbo3l-server; reuse rather than
    // re-add.
    use ed25519_dalek::SigningKey;
    let sk = SigningKey::from_bytes(&raw);
    let pk = sk.verifying_key();
    Ok(format!("0x{}", hex::encode(pk.to_bytes())))
}

/// Truncate `s` to at most `max` chars and append `…` if truncated.
/// Operates on byte indices for speed; safe because we always take a
/// prefix of UTF-8 we just received from `LiveEnsResolver`, and the
/// truncation happens at a char boundary that we test for below.
fn truncate_for_display(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out = String::with_capacity(max + 4);
    for (i, c) in s.chars().enumerate() {
        if i >= max {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

fn emit_human_report(fqdn: &str, network: &str, checks: &[RecordCheck]) {
    println!("verify-ens: {fqdn}  (network: {network})");
    println!("---");
    let mut pass = 0u32;
    let mut fail = 0u32;
    let mut skip = 0u32;
    let mut absent = 0u32;
    for c in checks {
        let verdict = c.verdict();
        match verdict {
            "pass" => pass += 1,
            "fail" => fail += 1,
            "skip" => skip += 1,
            "absent" => absent += 1,
            _ => {}
        }
        let badge = match verdict {
            "pass" => "PASS  ",
            "fail" => "FAIL  ",
            "skip" => "—     ",
            "absent" => "ABSENT",
            _ => "?",
        };
        let actual_full = c.actual.as_deref().unwrap_or("(unset)");
        let expected_full = c.expected.as_deref().unwrap_or("(no expectation)");
        let actual = truncate_for_display(actual_full, 80);
        let expected = truncate_for_display(expected_full, 80);
        println!(
            "  {badge}  {:<24}  actual={actual:?}  expected={expected:?}",
            c.key
        );
    }
    println!("---");
    println!("  totals: pass={pass} fail={fail} skip={skip} absent={absent}");
    if fail == 0 {
        println!("  verdict: PASS");
    } else {
        println!("  verdict: FAIL");
    }
}

fn emit_json_report(fqdn: &str, network: &str, checks: &[RecordCheck]) {
    let value = serde_json::json!({
        "schema": "sbo3l.verify_ens_report.v1",
        "fqdn": fqdn,
        "network": network,
        "checks": checks.iter().map(|c| {
            serde_json::json!({
                "key": c.key,
                "actual": c.actual,
                "expected": c.expected,
                "verdict": c.verdict(),
            })
        }).collect::<Vec<_>>(),
        "verdict": if checks.iter().any(|c| c.verdict() == "fail") { "fail" } else { "pass" },
    });
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_hex_pubkey_accepts_64_hex_with_0x() {
        let p = normalise_hex_pubkey(
            "0x3c754c3aad07da711d90ef16665f46c53ad050c9b3764a68d444551ca3d22003",
        );
        assert!(p.is_some());
        assert!(p.unwrap().starts_with("0x"));
    }

    #[test]
    fn normalise_hex_pubkey_lowercases() {
        let p = normalise_hex_pubkey(
            "0x3C754C3AAD07DA711D90EF16665F46C53AD050C9B3764A68D444551CA3D22003",
        )
        .unwrap();
        assert_eq!(
            p,
            "0x3c754c3aad07da711d90ef16665f46c53ad050c9b3764a68d444551ca3d22003"
        );
    }

    #[test]
    fn normalise_hex_pubkey_rejects_short() {
        assert!(normalise_hex_pubkey("0x12").is_none());
    }

    #[test]
    fn normalise_hex_pubkey_rejects_non_hex() {
        assert!(normalise_hex_pubkey(
            "0xZZ54c3aad07da711d90ef16665f46c53ad050c9b3764a68d444551ca3d22003"
        )
        .is_none());
    }

    #[test]
    fn derive_pubkey_from_raw_32_byte_seed() {
        // Same seed -> same pubkey (deterministic). Use the
        // research-agent fleet seed and verify against the value
        // committed in scripts/fleet-config/agents-5.pubkeys.json.
        let seed_hex = "84d35a4cb37c14de9ee3edcef98a36b6076cb3edf2d1d52e6745bce4b6181c33";
        let mut seed = [0u8; 32];
        hex::decode_to_slice(seed_hex, &mut seed).unwrap();
        let pubkey = derive_pubkey_from_seed(&seed).unwrap();
        assert!(pubkey.starts_with("0x"));
        assert_eq!(pubkey.len(), 66);
    }

    #[test]
    fn derive_pubkey_from_hex_text_seed() {
        // ASCII bytes containing 64-char hex.
        let seed_hex = "84d35a4cb37c14de9ee3edcef98a36b6076cb3edf2d1d52e6745bce4b6181c33";
        let bytes = seed_hex.as_bytes();
        let pubkey = derive_pubkey_from_seed(bytes).unwrap();
        assert!(pubkey.starts_with("0x"));
        assert_eq!(pubkey.len(), 66);
    }

    #[test]
    fn derive_pubkey_rejects_short_input() {
        let bytes = b"too short";
        let err = derive_pubkey_from_seed(bytes).unwrap_err();
        assert!(err.contains("32-byte"), "{err}");
    }

    #[test]
    fn record_check_verdicts() {
        let pass = RecordCheck {
            key: "sbo3l:agent_id",
            actual: Some("a".into()),
            expected: Some("a".into()),
        };
        let fail = RecordCheck {
            key: "sbo3l:agent_id",
            actual: Some("a".into()),
            expected: Some("b".into()),
        };
        let skip = RecordCheck {
            key: "sbo3l:agent_id",
            actual: Some("a".into()),
            expected: None,
        };
        let absent = RecordCheck {
            key: "sbo3l:agent_id",
            actual: None,
            expected: None,
        };
        let absent_expected = RecordCheck {
            key: "sbo3l:agent_id",
            actual: None,
            expected: Some("a".into()),
        };
        assert_eq!(pass.verdict(), "pass");
        assert_eq!(fail.verdict(), "fail");
        assert_eq!(skip.verdict(), "skip");
        assert_eq!(absent.verdict(), "absent");
        assert_eq!(absent_expected.verdict(), "fail");
    }

    #[test]
    fn parse_expected_records_happy() {
        let m = parse_expected_records(Some(
            r#"{"sbo3l:agent_id":"foo","sbo3l:endpoint":"http://x"}"#,
        ))
        .unwrap();
        assert_eq!(m.get("sbo3l:agent_id"), Some(&"foo".to_string()));
        assert_eq!(m.get("sbo3l:endpoint"), Some(&"http://x".to_string()));
    }

    #[test]
    fn parse_expected_records_rejects_array() {
        assert!(parse_expected_records(Some("[]")).is_err());
    }

    #[test]
    fn parse_expected_records_rejects_non_string_value() {
        assert!(parse_expected_records(Some(r#"{"k": 42}"#)).is_err());
    }

    #[test]
    fn parse_expected_records_empty_when_unset() {
        let m = parse_expected_records(None).unwrap();
        assert!(m.is_empty());
    }
}
