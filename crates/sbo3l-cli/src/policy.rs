//! `sbo3l policy {validate,current,activate,diff}` — local active-policy
//! lifecycle CLI (PSM-A3).
//!
//! Operates on the `active_policy` SQLite table (V006). Every operation
//! is a local SQLite write/read; there is no on-chain anchor, no
//! consensus, no signing on activation. Whoever opens the DB activates
//! the policy. This is **production-shaped lifecycle**, not remote
//! governance — documented in `docs/cli/policy.md`.

use std::path::Path;
use std::process::ExitCode;

use chrono::Utc;
use sbo3l_policy::Policy;
use sbo3l_storage::policy_store::ActivateOutcome;
use sbo3l_storage::Storage;

fn open_db(db: &Path) -> Result<Storage, String> {
    Storage::open(db).map_err(|e| format!("failed to open db {}: {e}", db.display()))
}

/// Parse + semantic-validate + canonical-hash a JSON policy file.
/// Returns `(canonical_json, hash, parsed_policy)` so callers can reuse.
fn parse_and_hash(path: &Path) -> Result<(String, String, Policy), String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let policy: Policy = Policy::parse_json(&raw).map_err(|e| format!("invalid policy: {e}"))?;
    let hash = policy
        .canonical_hash()
        .map_err(|e| format!("hash failed: {e}"))?;
    // Re-emit the policy as canonical JSON so the stored bytes are
    // independent of the operator's source file's whitespace.
    let value = serde_json::to_value(&policy).map_err(|e| format!("serialise failed: {e}"))?;
    let canonical = serde_json_canonicalizer::to_string(&value)
        .map_err(|e| format!("canonicalise failed: {e}"))?;
    Ok((canonical, hash, policy))
}

/// `sbo3l policy validate <file>` — parse a candidate policy and print
/// its canonical hash + a tiny summary. Exit codes:
/// - 0 — valid
/// - 1 — file read failure
/// - 2 — invalid policy
pub fn cmd_validate(path: &Path) -> ExitCode {
    match parse_and_hash(path) {
        Ok((_canonical, hash, policy)) => {
            println!("ok: policy parses + validates");
            println!("  policy_hash:   {hash}");
            println!("  agents:        {}", policy.agents.len());
            println!("  rules:         {}", policy.rules.len());
            println!("  providers:     {}", policy.providers.len());
            println!("  recipients:    {}", policy.recipients.len());
            println!("  budgets:       {}", policy.budgets.len());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("sbo3l policy validate: {e}");
            // Distinguish "file IO" (exit 1) from "invalid policy" (exit 2)
            // — the former usually means a typo'd path; the latter is a
            // real policy authoring problem the operator must address.
            if e.starts_with("invalid policy") {
                ExitCode::from(2)
            } else {
                ExitCode::from(1)
            }
        }
    }
}

/// `sbo3l policy current --db <path>` — print the currently-active
/// policy row. Exit codes:
/// - 0 — an active policy is present
/// - 1 — db open failure / read failure
/// - 3 — DB is open but no policy is active (the "honest no-active" path
///   — distinct from open-failure so scripts can react sensibly)
pub fn cmd_current(db: &Path) -> ExitCode {
    let storage = match open_db(db) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sbo3l policy current: {e}");
            return ExitCode::from(1);
        }
    };
    match storage.policy_current() {
        Ok(Some(rec)) => {
            println!("active policy:");
            println!("  version:       v{}", rec.version);
            println!("  policy_hash:   {}", rec.policy_hash);
            println!("  source:        {}", rec.source);
            println!("  activated_at:  {}", rec.activated_at.to_rfc3339());
            ExitCode::SUCCESS
        }
        Ok(None) => {
            // Don't pretend there is an active policy. The exit code is
            // the signal: 3 means "DB is fine, just no policy yet".
            println!(
                "no active policy in this db. \
                 Run `sbo3l policy activate <file> --db {}` to seed one.",
                db.display()
            );
            ExitCode::from(3)
        }
        Err(e) => {
            eprintln!("sbo3l policy current: {e}");
            ExitCode::from(1)
        }
    }
}

/// `sbo3l policy activate <file> --db <path> [--source <name>]` —
/// validate, hash, and activate a policy. Idempotent: re-activating the
/// already-active policy is a no-op.
pub fn cmd_activate(path: &Path, db: &Path, source: Option<&str>) -> ExitCode {
    let (canonical, hash, _policy) = match parse_and_hash(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("sbo3l policy activate: {e}");
            return if e.starts_with("invalid policy") {
                ExitCode::from(2)
            } else {
                ExitCode::from(1)
            };
        }
    };
    let mut storage = match open_db(db) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sbo3l policy activate: {e}");
            return ExitCode::from(1);
        }
    };
    let source_label = source.unwrap_or("operator-cli");
    match storage.policy_activate(&canonical, &hash, source_label, Utc::now()) {
        Ok(ActivateOutcome::Activated { version }) => {
            println!("activated: policy_hash={hash} version=v{version} source={source_label}");
            ExitCode::SUCCESS
        }
        Ok(ActivateOutcome::AlreadyActive { version }) => {
            println!(
                "already active: policy_hash={hash} version=v{version} (no-op — \
                 the supplied policy is already the currently-active one)"
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            // The storage layer surfaces a UNIQUE-constraint failure when
            // an operator tries to re-activate a hash that has already
            // been seen (deactivated rows still hold their hash). Map
            // that to a clear, structured exit so scripts can branch on
            // it without parsing the message.
            let msg = e.to_string();
            if msg.to_lowercase().contains("unique") || msg.to_lowercase().contains("constraint") {
                eprintln!(
                    "sbo3l policy activate: this policy hash ({hash}) was already \
                     activated previously and then deactivated; re-activating an \
                     already-seen hash is refused so the lifecycle history stays \
                     monotonic. Edit the policy (even cosmetically) and re-run."
                );
                return ExitCode::from(4);
            }
            eprintln!("sbo3l policy activate: {msg}");
            ExitCode::from(1)
        }
    }
}

/// `sbo3l policy diff <file-a> <file-b>` — diff two candidate policy
/// files at the *canonical-JSON* level. Both files must parse and
/// validate; the output is a small unified-diff-ish list of added and
/// removed lines from the canonicalised JSONs.
///
/// Exit codes:
/// - 0 — files parsed and were identical
/// - 1 — files parsed and differed (the diff is printed)
/// - 2 — at least one file failed to parse / validate
pub fn cmd_diff(a: &Path, b: &Path) -> ExitCode {
    let parsed_a = parse_and_hash(a);
    let parsed_b = parse_and_hash(b);
    let (canon_a, hash_a) = match parsed_a {
        Ok((c, h, _)) => (c, h),
        Err(e) => {
            eprintln!("sbo3l policy diff: {} — {e}", a.display());
            return ExitCode::from(2);
        }
    };
    let (canon_b, hash_b) = match parsed_b {
        Ok((c, h, _)) => (c, h),
        Err(e) => {
            eprintln!("sbo3l policy diff: {} — {e}", b.display());
            return ExitCode::from(2);
        }
    };
    if hash_a == hash_b {
        println!("no differences (policy_hash = {hash_a})");
        return ExitCode::SUCCESS;
    }
    println!("policies differ:");
    println!("  - {} (policy_hash = {hash_a})", a.display());
    println!("  + {} (policy_hash = {hash_b})", b.display());
    let pretty_a = pretty_json_lines(&canon_a);
    let pretty_b = pretty_json_lines(&canon_b);
    let diff = simple_line_diff(&pretty_a, &pretty_b);
    for line in diff {
        println!("{line}");
    }
    ExitCode::from(1)
}

/// Re-pretty-print canonical JSON so a human-readable diff can show
/// per-key changes. Canonical JSON has no newlines; we ask
/// `serde_json` for indent=2 output.
fn pretty_json_lines(canonical: &str) -> Vec<String> {
    let value: serde_json::Value =
        serde_json::from_str(canonical).expect("canonical JSON must parse");
    let pretty = serde_json::to_string_pretty(&value).unwrap_or_else(|_| canonical.to_string());
    pretty.lines().map(|s| s.to_string()).collect()
}

/// Simple unified-diff-ish output. Walks both line vectors with a
/// longest-common-subsequence (DP) so equal lines anchor and only
/// differing regions get printed. For a 4-CLI-subcommand prototype
/// this is plenty; we don't pull in a diff crate.
fn simple_line_diff(a: &[String], b: &[String]) -> Vec<String> {
    let n = a.len();
    let m = b.len();
    // dp[i][j] = LCS length of a[..i], b[..j].
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            dp[i + 1][j + 1] = if a[i] == b[j] {
                dp[i][j] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    // Backtrack into a sequence of (kind, line) tokens.
    let mut out: Vec<String> = Vec::new();
    let (mut i, mut j) = (n, m);
    let mut tokens: Vec<(char, String)> = Vec::new();
    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            tokens.push((' ', a[i - 1].clone()));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            tokens.push(('-', a[i - 1].clone()));
            i -= 1;
        } else {
            tokens.push(('+', b[j - 1].clone()));
            j -= 1;
        }
    }
    while i > 0 {
        tokens.push(('-', a[i - 1].clone()));
        i -= 1;
    }
    while j > 0 {
        tokens.push(('+', b[j - 1].clone()));
        j -= 1;
    }
    tokens.reverse();

    // Print only ±-lines plus 1 line of context above and below each
    // change region (keeps the output focused).
    let mut keep = vec![false; tokens.len()];
    for (idx, (kind, _)) in tokens.iter().enumerate() {
        if *kind != ' ' {
            keep[idx] = true;
            if idx > 0 {
                keep[idx - 1] = true;
            }
            if idx + 1 < keep.len() {
                keep[idx + 1] = true;
            }
        }
    }
    let mut last_emitted: Option<usize> = None;
    for (idx, (kind, line)) in tokens.iter().enumerate() {
        if !keep[idx] {
            continue;
        }
        if let Some(prev) = last_emitted {
            if idx > prev + 1 {
                out.push("  …".to_string());
            }
        }
        out.push(format!("{kind} {line}"));
        last_emitted = Some(idx);
    }
    out
}
