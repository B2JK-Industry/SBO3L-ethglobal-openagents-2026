//! Integration tests for `sbo3l uniswap swap` (Task D).
//!
//! Exercises the real `sbo3l` binary end-to-end. Three things to
//! pin here that pure unit tests can't (because they bypass clap):
//!
//! 1. `--dry-run` + `--broadcast` are mutually exclusive at the
//!    clap layer (clap rejects with exit status 2 BEFORE the
//!    command body sees the args).
//! 2. The `--help` text is generated and lists the subcommand —
//!    proves the wire-up survived Cargo build.
//! 3. A sepolia dry-run with no RPC produces a valid JSON envelope
//!    on stdout and exits zero.

use std::path::PathBuf;
use std::process::Command;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sbo3l"))
}

#[test]
fn uniswap_swap_help_lists_subcommand() {
    let out = Command::new(cli_bin())
        .args(["uniswap", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success(), "uniswap --help should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("swap"),
        "uniswap --help missing `swap` subcommand: {stdout}"
    );
}

#[test]
fn uniswap_swap_help_documents_flags() {
    let out = Command::new(cli_bin())
        .args(["uniswap", "swap", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for needle in [
        "--network",
        "--amount-in",
        "--token-out",
        "--recipient",
        "--slippage-bps",
        "--dry-run",
        "--broadcast",
        "--rpc-url",
        "--private-key-env-var",
    ] {
        assert!(
            stdout.contains(needle),
            "uniswap swap --help missing flag {needle}: {stdout}"
        );
    }
}

#[test]
fn dry_run_and_broadcast_are_mutually_exclusive() {
    let out = Command::new(cli_bin())
        .args([
            "uniswap",
            "swap",
            "--network",
            "sepolia",
            "--amount-in",
            "0.005ETH",
            "--token-out",
            "USDC",
            "--recipient",
            "0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231",
            "--dry-run",
            "--broadcast",
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "clap should reject --dry-run + --broadcast together"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--broadcast")
            || stderr.contains("conflicts")
            || stderr.contains("cannot be used"),
        "expected clap conflict-with error, got: {stderr}"
    );
}

#[test]
fn mainnet_without_gate_refused() {
    // Hermetic against caller env: explicitly clear the gate. We can't
    // un-set in the parent process but we CAN ensure the child doesn't
    // see it (by not propagating, then env-clearing).
    let out = Command::new(cli_bin())
        .env_remove("SBO3L_ALLOW_MAINNET_TX")
        .args([
            "uniswap",
            "swap",
            "--network",
            "mainnet",
            "--amount-in",
            "0.005ETH",
            "--token-out",
            "USDC",
            "--recipient",
            "0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231",
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "mainnet without SBO3L_ALLOW_MAINNET_TX=1 must refuse"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("SBO3L_ALLOW_MAINNET_TX"),
        "stderr must mention the gate var, got: {stderr}"
    );
}

#[test]
fn sepolia_dry_run_emits_envelope_to_stdout_and_out() {
    let tmp = tempfile::tempdir().unwrap();
    let out_path = tmp.path().join("envelope.json");
    let out = Command::new(cli_bin())
        .env_remove("SBO3L_RPC_URL") // ensure deterministic no-quote path
        .args([
            "uniswap",
            "swap",
            "--network",
            "sepolia",
            "--amount-in",
            "0.005ETH",
            "--token-out",
            "USDC",
            "--recipient",
            "0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("network:"),
        "expected envelope print, got: {stdout}"
    );
    assert!(stdout.contains("router:"), "missing router");
    assert!(stdout.contains("data ("), "missing calldata line");
    // The on-disk JSON must parse and have the right shape.
    let body = std::fs::read_to_string(&out_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["network"], "sepolia");
    assert_eq!(v["chain_id"], 11_155_111);
    assert_eq!(v["broadcasted"], false);
    assert_eq!(v["fee_tier"], 3000);
    assert_eq!(v["token_in"]["symbol"], "ETH");
    assert_eq!(v["token_out"]["symbol"], "USDC");
    // Hex-encoded calldata starts with the exactInputSingle selector.
    let data = v["data"].as_str().unwrap();
    assert!(
        data.starts_with("0x04e45aaf"),
        "unexpected calldata prefix: {data}"
    );
}
