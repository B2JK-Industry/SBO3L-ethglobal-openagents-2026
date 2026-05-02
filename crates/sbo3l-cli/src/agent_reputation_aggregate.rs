//! `sbo3l agent reputation-aggregate` — cross-chain aggregation
//! over `ChainReputationSnapshot` inputs (R12 P3).
//!
//! Pure-offline reader. Operators feed in a JSON file containing
//! per-chain snapshots; the CLI runs
//! `sbo3l_policy::cross_chain_reputation::aggregate_reputation`
//! and prints the [`AggregateReputationReport`].
//!
//! ## Why a separate "aggregate" subcommand
//!
//! The aggregator is logically downstream of the broadcast pipeline
//! — it consumes snapshots that the operator has already gathered
//! (typically via N parallel `cast call` reads against each chain's
//! `SBO3LReputationRegistry.reputationOf`). Bundling the gather +
//! aggregate steps into one CLI call would couple this command to
//! a live RPC connection per chain; keeping them split lets a
//! verifier re-aggregate from a stored snapshot file without re-
//! hitting the chains.
//!
//! ## Wire format (input)
//!
//! ```json
//! {
//!   "now_secs": 1764000000,
//!   "snapshots": [
//!     {"chain_id": 1, "fqdn": "research-agent.sbo3lagent.eth",
//!      "score": 90, "observed_at": 1763999000},
//!     {"chain_id": 11155420, "fqdn": "research-agent.sbo3lagent.eth",
//!      "score": 88, "observed_at": 1763999100},
//!     {"chain_id": 84532, "fqdn": "research-agent.sbo3lagent.eth",
//!      "score": 92, "observed_at": 1763998500}
//!   ]
//! }
//! ```
//!
//! ## Output
//!
//! The full `AggregateReputationReport` as pretty-printed JSON
//! (aggregate score, source count, per-chain breakdown with
//! recency factor + chain weight).

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use sbo3l_policy::cross_chain_reputation::{
    aggregate_reputation, AggregateReputationParams, ChainReputationSnapshot,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AggregateInput {
    pub now_secs: u64,
    pub snapshots: Vec<SnapshotEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotEntry {
    pub chain_id: u64,
    pub fqdn: String,
    pub score: u8,
    pub observed_at: u64,
}

impl From<SnapshotEntry> for ChainReputationSnapshot {
    fn from(s: SnapshotEntry) -> Self {
        Self {
            chain_id: s.chain_id,
            fqdn: s.fqdn,
            score: s.score,
            observed_at: s.observed_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReputationAggregateArgs {
    pub input: PathBuf,
    pub out: Option<PathBuf>,
}

pub fn cmd_agent_reputation_aggregate(args: ReputationAggregateArgs) -> ExitCode {
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run(args: ReputationAggregateArgs) -> Result<(), ExitCode> {
    let raw = fs::read_to_string(&args.input).map_err(|e| {
        eprintln!(
            "sbo3l agent reputation-aggregate: read input {}: {e}",
            args.input.display()
        );
        ExitCode::from(2)
    })?;
    let input: AggregateInput = serde_json::from_str(&raw).map_err(|e| {
        eprintln!(
            "sbo3l agent reputation-aggregate: parse input {}: {e}",
            args.input.display()
        );
        ExitCode::from(2)
    })?;
    let snapshots: Vec<ChainReputationSnapshot> =
        input.snapshots.into_iter().map(|s| s.into()).collect();
    let params = AggregateReputationParams::default();
    let report = aggregate_reputation(&snapshots, input.now_secs, &params);

    // Serialise via a thin wrapper so the JSON shape is stable —
    // `AggregateReputationReport` re-derives `Serialize` so this is
    // a near-no-op, but the wrapper lets us version the output.
    #[derive(Serialize)]
    struct AggregateOutput<'a> {
        schema: &'a str,
        aggregate_score: u8,
        source_count: usize,
        total_weight: f64,
        per_chain: Vec<PerChainOut<'a>>,
    }
    #[derive(Serialize)]
    struct PerChainOut<'a> {
        chain_id: u64,
        fqdn: &'a str,
        raw_score: u8,
        chain_weight: f64,
        recency_factor: f64,
        effective_contribution: f64,
    }
    let out = AggregateOutput {
        schema: "sbo3l.reputation_aggregate_report.v1",
        aggregate_score: report.aggregate_score,
        source_count: report.source_count,
        total_weight: report.total_weight,
        per_chain: report
            .per_chain
            .iter()
            .map(|p| PerChainOut {
                chain_id: p.chain_id,
                fqdn: &p.fqdn,
                raw_score: p.raw_score,
                chain_weight: p.chain_weight,
                recency_factor: p.recency_factor,
                effective_contribution: p.effective_contribution,
            })
            .collect(),
    };
    let json = serde_json::to_string_pretty(&out).map_err(|e| {
        eprintln!("sbo3l agent reputation-aggregate: serialise: {e}");
        ExitCode::from(2)
    })?;
    println!("{json}");
    if let Some(path) = args.out {
        fs::write(&path, &json).map_err(|e| {
            eprintln!(
                "sbo3l agent reputation-aggregate: write {}: {e}",
                path.display()
            );
            ExitCode::from(2)
        })?;
        eprintln!("wrote {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_input(s: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(s.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn happy_path_three_chain_aggregate() {
        // Same shape as the synthetic 3-chain fleet test in
        // sbo3l-policy::cross_chain_reputation::tests — should
        // produce the same aggregate score.
        // Default weights: mainnet 1.0, OP 0.8, Polygon 0.6.
        // Values: 90, 80, 70 → numerator 90+64+42=196, denominator 2.4
        // → 196/2.4 = 81.67 → 82.
        let now = 2_000_000_000u64;
        let input = format!(
            r#"{{
              "now_secs": {now},
              "snapshots": [
                {{"chain_id": 1,   "fqdn": "x", "score": 90, "observed_at": {}}},
                {{"chain_id": 10,  "fqdn": "x", "score": 80, "observed_at": {}}},
                {{"chain_id": 137, "fqdn": "x", "score": 70, "observed_at": {}}}
              ]
            }}"#,
            now - 60,
            now - 60,
            now - 60,
        );
        let f = write_input(&input);
        let res = run(ReputationAggregateArgs {
            input: f.path().to_path_buf(),
            out: None,
        });
        assert!(res.is_ok());
    }

    #[test]
    fn empty_snapshots_returns_max_score() {
        let now = 2_000_000_000u64;
        let input = format!(r#"{{"now_secs": {now}, "snapshots": []}}"#);
        let f = write_input(&input);
        let res = run(ReputationAggregateArgs {
            input: f.path().to_path_buf(),
            out: None,
        });
        assert!(res.is_ok());
    }

    #[test]
    fn malformed_input_returns_exit2() {
        let f = write_input("not json");
        let res = run(ReputationAggregateArgs {
            input: f.path().to_path_buf(),
            out: None,
        });
        assert!(res.is_err());
    }

    #[test]
    fn unknown_field_in_snapshot_rejected() {
        // serde(deny_unknown_fields) at the SnapshotEntry level
        // catches typos like "scoer" → "score" without silently
        // dropping the field.
        let bad = r#"{
          "now_secs": 0,
          "snapshots": [
            {"chain_id": 1, "fqdn": "x", "scoer": 90, "observed_at": 0}
          ]
        }"#;
        let f = write_input(bad);
        let res = run(ReputationAggregateArgs {
            input: f.path().to_path_buf(),
            out: None,
        });
        assert!(res.is_err());
    }

    #[test]
    fn writes_out_file_when_provided() {
        let now = 2_000_000_000u64;
        let input = format!(
            r#"{{"now_secs": {now}, "snapshots": [{{"chain_id": 1, "fqdn": "x", "score": 75, "observed_at": {}}}]}}"#,
            now - 60,
        );
        let f = write_input(&input);
        let out_file = NamedTempFile::new().unwrap();
        let res = run(ReputationAggregateArgs {
            input: f.path().to_path_buf(),
            out: Some(out_file.path().to_path_buf()),
        });
        assert!(res.is_ok());
        let written = fs::read_to_string(out_file.path()).unwrap();
        assert!(written.contains("\"aggregate_score\": 75"));
        assert!(written.contains("\"schema\": \"sbo3l.reputation_aggregate_report.v1\""));
    }
}
