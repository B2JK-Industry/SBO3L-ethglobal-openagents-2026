//! Reputation publisher (T-4-6 — follow-up to T-4-3).
//!
//! Publishes an agent's v2 reputation score to its ENS resolver as
//! the `sbo3l:reputation_score` text record. Pure-function: takes
//! audit-event input + identity (fqdn, network, resolver), emits a
//! [`ReputationPublishEnvelope`] containing the `setText` calldata
//! ready for broadcast.
//!
//! The score itself is computed via
//! [`sbo3l_policy::reputation::compute_reputation_v2`] — same
//! 4-criteria weighted scoring used by the cross-agent attestation
//! refusal threshold. This module does not redefine the scoring
//! rules; it only ships the on-chain publication path.
//!
//! ## Wire format
//!
//! ENS text record key: `sbo3l:reputation_score`. Value: decimal
//! `0..=100` (e.g. `"87"`). Reads cleanly in `viem.getEnsText` /
//! ENS App / direct `text(node, key)` calls — no special decoding
//! on the consumer side.
//!
//! ## Truthfulness
//!
//! Same dry-run / fixture story as the audit-anchor envelope: the
//! envelope is publishable on its own — same audit-event input
//! always re-derives the same score and the same calldata. Caller
//! can pin the envelope hash in a receipt to prove "this score
//! was computed from these events at this time".
//!
//! ## CLI
//!
//! `sbo3l agent reputation publish <fqdn>` is the operator-facing
//! entry point. It reads events from a JSON file (decoupled from
//! the SQLite storage layer for portability — operators commonly
//! want to publish reputation from a CI artifact, not a live DB),
//! calls [`build_publish_envelope`], and prints / writes the
//! envelope. Broadcast wires through F-5 EthSigner once that lands.

use serde::{Deserialize, Serialize};

use crate::ens_anchor::{namehash, set_text_calldata, AnchorError, EnsNetwork};
use sbo3l_policy::reputation::{compute_reputation_v2, Reputation, ReputationEvent};

/// ENS text-record key under which the reputation score lives.
pub const REPUTATION_TEXT_KEY: &str = "sbo3l:reputation_score";

/// Envelope schema id. Bump on any breaking change to the JSON
/// shape — pinned in the JSON output so consumers can version-gate.
pub const REPUTATION_ENVELOPE_SCHEMA_ID: &str = "sbo3l.reputation_publish_envelope.v1";

/// Wire-format input for publisher CLI / library callers. Mirrors
/// [`ReputationEvent`] but uses `u64` seconds for `age` so the
/// shape is JSON-friendly without pulling Duration through serde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReputationEventInput {
    /// `"allow"` / `"deny"` / unknown.
    pub decision: String,
    /// Whether the signed receipt for this decision was carried
    /// through to a sponsor adapter and confirmed.
    pub executor_confirmed: bool,
    /// Age of the event at scoring time, in seconds. Recent events
    /// weigh more (per `compute_reputation_v2`'s recency penalty).
    pub age_secs: u64,
}

impl From<&ReputationEventInput> for ReputationEvent {
    fn from(input: &ReputationEventInput) -> Self {
        ReputationEvent {
            decision: input.decision.clone(),
            executor_confirmed: input.executor_confirmed,
            age: std::time::Duration::from_secs(input.age_secs),
        }
    }
}

/// Inputs to [`build_publish_envelope`]. Validated at build time:
/// `resolver` must be a 0x-prefixed 40-hex-char address, `domain`
/// must be a non-empty ENS name.
#[derive(Debug, Clone)]
pub struct ReputationPublishParams<'a> {
    pub network: EnsNetwork,
    pub domain: &'a str,
    pub resolver: &'a str,
    /// RFC-3339 timestamp pinned in the envelope.
    pub created_at: &'a str,
    /// Mode tag matching the audit-anchor convention (`"dry_run"`
    /// or `"offline_fixture"`). Stored verbatim in the envelope's
    /// `mode` field so consumers can tell publishable artifacts
    /// apart.
    pub mode: PublishMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishMode {
    DryRun,
    OfflineFixture,
}

impl PublishMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::OfflineFixture => "offline_fixture",
        }
    }

    fn explanation(self) -> &'static str {
        match self {
            Self::DryRun => {
                "Dry-run reputation envelope. Computed exactly what would be \
                             broadcast (namehash, calldata, score) but did NOT contact any \
                             RPC and did NOT sign anything. Re-run with --broadcast in a \
                             build that wires the broadcast path."
            }
            Self::OfflineFixture => {
                "Offline reputation fixture. Identical content to dry-run, \
                                      written to disk for demo / CI fixture use."
            }
        }
    }
}

/// Output of [`build_publish_envelope`]. Carries everything a
/// downstream consumer needs to verify the envelope independently
/// (recompute the score from the events, recompute the calldata
/// from the score, recompute the namehash from the domain).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReputationPublishEnvelope {
    /// `sbo3l.reputation_publish_envelope.v1`.
    pub schema: String,
    pub mode: String,
    pub explanation: String,
    pub network: String,
    pub domain: String,
    pub namehash: String,
    pub resolver: String,
    /// Always `sbo3l:reputation_score`.
    pub text_record_key: String,
    pub score: u8,
    pub event_count: u64,
    /// Hex-encoded `setText(bytes32,string,string)` calldata, no
    /// `0x` prefix. Same form as the audit-anchor envelope.
    pub calldata: String,
    pub created_at: String,
}

/// Build a publish envelope from raw audit events.
pub fn build_publish_envelope(
    params: ReputationPublishParams<'_>,
    events: &[ReputationEventInput],
) -> Result<ReputationPublishEnvelope, AnchorError> {
    let resolver = validate_resolver(params.resolver)?;
    let node = namehash(params.domain)?;

    let policy_events = events.iter().map(ReputationEvent::from);
    let score: Reputation = compute_reputation_v2(policy_events);
    let value = score.to_text_record();

    let calldata = set_text_calldata(node, REPUTATION_TEXT_KEY, &value);

    Ok(ReputationPublishEnvelope {
        schema: REPUTATION_ENVELOPE_SCHEMA_ID.to_string(),
        mode: params.mode.as_str().to_string(),
        explanation: params.mode.explanation().to_string(),
        network: params.network.as_str().to_string(),
        domain: params.domain.to_string(),
        namehash: hex::encode(node),
        resolver,
        text_record_key: REPUTATION_TEXT_KEY.to_string(),
        score: score.as_u8(),
        event_count: events.len() as u64,
        calldata: hex::encode(&calldata),
        created_at: params.created_at.to_string(),
    })
}

fn validate_resolver(s: &str) -> Result<String, AnchorError> {
    let stripped = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));
    let body = match stripped {
        Some(b) => b,
        None => return Err(AnchorError::ResolverBadFormat(s.to_string())),
    };
    if body.len() != 40 || !body.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(AnchorError::ResolverBadFormat(s.to_string()));
    }
    Ok(format!("0x{}", body.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ens_anchor::SET_TEXT_SELECTOR;

    fn ev(decision: &str, executor_confirmed: bool, age_secs: u64) -> ReputationEventInput {
        ReputationEventInput {
            decision: decision.to_string(),
            executor_confirmed,
            age_secs,
        }
    }

    fn params() -> ReputationPublishParams<'static> {
        ReputationPublishParams {
            network: EnsNetwork::Mainnet,
            domain: "research-agent.sbo3lagent.eth",
            resolver: "0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63",
            created_at: "2026-05-02T00:00:00Z",
            mode: PublishMode::DryRun,
        }
    }

    #[test]
    fn empty_chain_publishes_max_score() {
        let env = build_publish_envelope(params(), &[]).unwrap();
        assert_eq!(env.score, 100);
        assert_eq!(env.event_count, 0);
    }

    #[test]
    fn fleet_of_clean_events_publishes_high_score() {
        let events: Vec<_> = (0..120).map(|_| ev("allow", true, 86400)).collect();
        let env = build_publish_envelope(params(), &events).unwrap();
        // 120 events, all clean+confirmed → 100.
        assert_eq!(env.score, 100);
        assert_eq!(env.event_count, 120);
    }

    #[test]
    fn calldata_is_set_text_with_decimal_score_value() {
        let events: Vec<_> = (0..10).map(|_| ev("allow", true, 0)).collect();
        let env = build_publish_envelope(params(), &events).unwrap();

        // First 4 bytes of calldata = SET_TEXT_SELECTOR.
        let bytes = hex::decode(&env.calldata).unwrap();
        assert_eq!(&bytes[..4], &SET_TEXT_SELECTOR);

        // The score string is included verbatim somewhere in the
        // tail (it's a short ASCII value, so check substring).
        let score_str = env.score.to_string();
        let calldata_lower = String::from_utf8_lossy(&bytes).to_lowercase();
        // Look for the score's hex form in the tail.
        let score_hex = hex::encode(score_str.as_bytes());
        assert!(
            env.calldata.contains(&score_hex),
            "calldata should contain hex-encoded score '{score_hex}': {calldata_lower}",
        );
    }

    #[test]
    fn namehash_pinned_for_known_domain() {
        let env = build_publish_envelope(params(), &[]).unwrap();
        assert_eq!(env.namehash.len(), 64);
        // namehash is deterministic — re-derive and compare.
        let recomputed = namehash("research-agent.sbo3lagent.eth").unwrap();
        assert_eq!(env.namehash, hex::encode(recomputed));
    }

    #[test]
    fn resolver_normalised_to_lowercase_0x_prefix() {
        let mut p = params();
        p.resolver = "0x231B0EE14048E9DCCD1D247744D114A4EB5E8E63";
        let env = build_publish_envelope(p, &[]).unwrap();
        assert_eq!(env.resolver, "0x231b0ee14048e9dccd1d247744d114a4eb5e8e63");
    }

    #[test]
    fn malformed_resolver_rejected() {
        let mut p = params();
        p.resolver = "not-an-address";
        let err = build_publish_envelope(p, &[]).unwrap_err();
        assert!(matches!(err, AnchorError::ResolverBadFormat(_)));
    }

    #[test]
    fn empty_domain_rejected() {
        let mut p = params();
        p.domain = "";
        let err = build_publish_envelope(p, &[]).unwrap_err();
        assert!(matches!(err, AnchorError::EmptyDomain));
    }

    #[test]
    fn schema_id_pinned() {
        let env = build_publish_envelope(params(), &[]).unwrap();
        assert_eq!(env.schema, "sbo3l.reputation_publish_envelope.v1");
        assert_eq!(env.text_record_key, "sbo3l:reputation_score");
    }

    #[test]
    fn mode_tag_round_trips() {
        let mut p = params();
        p.mode = PublishMode::OfflineFixture;
        let env = build_publish_envelope(p, &[]).unwrap();
        assert_eq!(env.mode, "offline_fixture");
        assert!(env.explanation.contains("Offline"));
    }

    #[test]
    fn json_round_trip() {
        let env = build_publish_envelope(params(), &[ev("allow", true, 0)]).unwrap();
        let s = serde_json::to_string(&env).unwrap();
        let back: ReputationPublishEnvelope = serde_json::from_str(&s).unwrap();
        assert_eq!(env, back);
    }

    #[test]
    fn input_event_round_trip_via_json() {
        let input = ev("allow", true, 12345);
        let s = serde_json::to_string(&input).unwrap();
        let back: ReputationEventInput = serde_json::from_str(&s).unwrap();
        assert_eq!(input, back);
    }

    #[test]
    fn input_event_rejects_unknown_fields() {
        let bad = r#"{"decision":"allow","executor_confirmed":true,"age_secs":0,"extra":"x"}"#;
        let res: Result<ReputationEventInput, _> = serde_json::from_str(bad);
        assert!(res.is_err());
    }

    /// Pinned: 5-agent fleet shape. Each agent has the same
    /// 12-event allow/deny mix; same input → same score.
    #[test]
    fn fleet_of_5_publishes_consistent_scores() {
        let mixed = vec![
            ev("allow", true, 0),
            ev("allow", true, 0),
            ev("allow", true, 0),
            ev("allow", true, 0),
            ev("allow", true, 0),
            ev("allow", true, 0),
            ev("allow", true, 0),
            ev("allow", true, 0),
            ev("allow", true, 0),
            ev("allow", false, 0),
            ev("deny", false, 0),
            ev("deny", false, 0),
        ];
        let scores: Vec<u8> = (0..5)
            .map(|_| build_publish_envelope(params(), &mixed).unwrap().score)
            .collect();
        assert!(scores.iter().all(|&s| s == scores[0]));
        // Sanity: not 100 (some denies), not 0 (mostly allows).
        assert!(scores[0] > 50 && scores[0] < 100, "got score {}", scores[0]);
    }

    /// Building the same envelope twice produces bit-identical
    /// output (modulo `created_at`, which is an explicit input).
    /// Truthfulness: an operator can replay the publisher and get
    /// the same calldata.
    #[test]
    fn envelope_is_deterministic() {
        let events = vec![ev("allow", true, 0), ev("deny", false, 86400 * 7)];
        let a = build_publish_envelope(params(), &events).unwrap();
        let b = build_publish_envelope(params(), &events).unwrap();
        assert_eq!(a, b);
    }
}
