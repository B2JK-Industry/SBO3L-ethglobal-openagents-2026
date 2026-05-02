//! T-3-5 backend: real-time event stream for `apps/trust-dns-viz/`.
//!
//! `GET /v1/events` upgrades to a WebSocket and streams JSON-encoded
//! [`VizEvent`]s — one frame per event. Dev 3's
//! `apps/trust-dns-viz/src/source.ts::realWebSocketSource` consumes
//! this verbatim; until this PR shipped, that frontend used a mock
//! event source whose protocol matches [`VizEvent`] byte-for-byte.
//!
//! # Wire format
//!
//! Each WebSocket text frame is a JSON object with a `kind` discriminant
//! and kind-specific fields:
//!
//! ```json
//! { "kind": "agent.discovered", "agent_id": "...", "ens_name": "...", "pubkey_b58": "...", "ts_ms": 1714606800000 }
//! { "kind": "attestation.signed", "from": "...", "to": "...", "attestation_id": "...", "ts_ms": ... }
//! { "kind": "decision.made", "agent_id": "...", "decision": "allow"|"deny", "deny_code"?: "...", "ts_ms": ... }
//! { "kind": "audit.checkpoint", "agent_id": "...", "chain_length": 42, "root_hash": "...", "ts_ms": ... }
//! ```
//!
//! Mirrors `apps/trust-dns-viz/src/events.ts::VizEvent` exactly. When you
//! add a new variant here, also add it to `events.ts` (and vice versa);
//! `isVizEvent` on the JS side rejects unknown `kind` values silently
//! so a drift just means the viz drops the event without crashing.
//!
//! # Sourcing strategy
//!
//! The daemon publishes events from inside the request handler, after
//! the audit row is finalized. A `tokio::sync::broadcast::Sender<VizEvent>`
//! held in `AppInner` is the publish point; the WebSocket handler
//! subscribes via `Receiver::recv` and forwards each frame.
//!
//! - `agent.discovered` — fires once per new `agent_id` ever seen by
//!   this daemon process. An in-memory `HashSet<String>` tracks
//!   first-seen state; cleared on daemon restart (which is fine —
//!   subscribers reconnect and re-bootstrap).
//! - `decision.made` — fires for every successful pipeline run
//!   (allow + deny + denied-by-budget).
//! - `audit.checkpoint` — fires alongside `decision.made` carrying the
//!   chain length and the audit event hash as `root_hash`.
//! - `attestation.signed` — not yet emitted by the daemon. The
//!   cross-agent attestation primitive lands later (T-3-4 scope); the
//!   variant exists so the frontend handles future events without a
//!   schema break.
//!
//! # Feature gating
//!
//! Compiled only with `--features ws_events`. The publish path is
//! a no-op when no subscribers are connected (broadcast::Sender::send
//! returns `Err(ChannelClosed)` only when there are zero receivers,
//! and we ignore that). Existing daemon runs are not disturbed by the
//! presence of the route — auth middleware is still applied (Dev 3
//! reads `SBO3L_BEARER_TOKEN` from the URL or via the dev bypass).

use std::collections::HashSet;
use std::sync::Mutex;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::AppState;

/// One real-time event for the trust-dns viz. JSON-tagged on `kind`,
/// mirroring `apps/trust-dns-viz/src/events.ts::VizEvent`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum VizEvent {
    /// Fires once per never-before-seen `agent_id`. The pubkey is
    /// base58-encoded for parity with the JS side; today it's a
    /// best-effort placeholder until ENS-derived agent pubkeys are
    /// wired through the audit chain.
    #[serde(rename = "agent.discovered")]
    AgentDiscovered {
        agent_id: String,
        ens_name: String,
        pubkey_b58: String,
        ts_ms: i64,
    },
    /// Cross-agent attestation. Reserved for T-3-4. Not emitted by the
    /// current daemon — the variant exists so the frontend doesn't
    /// schema-break when the cross-agent verifier ships.
    #[serde(rename = "attestation.signed")]
    AttestationSigned {
        from: String,
        to: String,
        attestation_id: String,
        ts_ms: i64,
    },
    /// Allow / deny outcome of one pipeline run. `deny_code` carries
    /// the spec-canonical reason on deny (e.g. `policy.budget_exceeded`).
    #[serde(rename = "decision.made")]
    DecisionMade {
        agent_id: String,
        decision: DecisionKind,
        #[serde(skip_serializing_if = "Option::is_none")]
        deny_code: Option<String>,
        ts_ms: i64,
    },
    /// New audit chain tip. `chain_length` is the post-append length;
    /// `root_hash` is the new tip's `event_hash`.
    #[serde(rename = "audit.checkpoint")]
    AuditCheckpoint {
        agent_id: String,
        chain_length: u64,
        root_hash: String,
        ts_ms: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionKind {
    Allow,
    Deny,
}

/// Live publish bus + first-seen-agent tracker. Held inside `AppInner`
/// behind the `ws_events` feature. The broadcast channel is tokio's
/// SPMC (single-producer-multi-consumer) primitive — a fan-out to N
/// connected WebSocket subscribers with a bounded buffer; lagging
/// subscribers see `RecvError::Lagged` and are dropped (the JS source
/// reconnects automatically per Dev 3's `realWebSocketSource`).
pub struct WsEventsBus {
    tx: broadcast::Sender<VizEvent>,
    seen_agents: Mutex<HashSet<String>>,
}

impl WsEventsBus {
    /// Construct a bus with a 256-event buffer. Tuned for "one or two
    /// browser tabs subscribed during a demo"; production would lift
    /// this and add backpressure metrics.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(256);
        Self {
            tx,
            seen_agents: Mutex::new(HashSet::new()),
        }
    }

    /// Subscribe a new WebSocket client. Caller drives `Receiver::recv`.
    pub fn subscribe(&self) -> broadcast::Receiver<VizEvent> {
        self.tx.subscribe()
    }

    /// True iff this is the first time we've seen `agent_id`. Marks
    /// `agent_id` seen on the way out — a second call in the same
    /// process returns `false`. Cheap; protected by `Mutex`.
    pub fn first_seen_agent(&self, agent_id: &str) -> bool {
        let mut set = self.seen_agents.lock().expect("seen_agents lock poisoned");
        set.insert(agent_id.to_string())
    }

    /// Publish without surfacing send errors — when no subscribers are
    /// connected `broadcast::Sender::send` returns
    /// `Err(SendError(_))`. That's the steady state for a daemon
    /// running without the viz attached; it's noise, not a fault.
    pub fn publish(&self, event: VizEvent) {
        let _ = self.tx.send(event);
    }
}

impl Default for WsEventsBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience helper: emit `agent.discovered` (first-seen only) +
/// `decision.made` + `audit.checkpoint` for one finalized request. The
/// daemon's request handler calls this once per pipeline run.
pub fn publish_pipeline_run(
    bus: &WsEventsBus,
    agent_id: &str,
    ens_name: Option<&str>,
    decision: DecisionKind,
    deny_code: Option<String>,
    chain_length: u64,
    audit_event_hash: &str,
) {
    let ts_ms = chrono::Utc::now().timestamp_millis();
    if bus.first_seen_agent(agent_id) {
        bus.publish(VizEvent::AgentDiscovered {
            agent_id: agent_id.to_string(),
            ens_name: ens_name.unwrap_or("").to_string(),
            // Placeholder — ENS-derived agent pubkeys land with T-3-1.
            // Today's audit signer pubkey is daemon-wide, not per-agent.
            pubkey_b58: format!("agent:{agent_id}"),
            ts_ms,
        });
    }
    bus.publish(VizEvent::DecisionMade {
        agent_id: agent_id.to_string(),
        decision,
        deny_code,
        ts_ms,
    });
    bus.publish(VizEvent::AuditCheckpoint {
        agent_id: agent_id.to_string(),
        chain_length,
        root_hash: audit_event_hash.to_string(),
        ts_ms,
    });
}

/// `GET /v1/events` — upgrade to WebSocket and forward every published
/// [`VizEvent`] as a JSON text frame. The handler is mounted by
/// `router()` only when the `ws_events` feature is enabled.
pub async fn ws_events_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    let bus = state.0.ws_events.clone();
    ws.on_upgrade(move |socket| async move {
        if let Some(bus) = bus {
            forward_loop(socket, bus.subscribe()).await;
        } else {
            // Feature flag was on at compile time but bus wasn't
            // initialised. Should be unreachable when the flag is on
            // — log and close cleanly.
            tracing::warn!("ws_events route hit without bus initialised");
            let _ = socket.close().await;
        }
    })
}

async fn forward_loop(socket: WebSocket, mut rx: broadcast::Receiver<VizEvent>) {
    let (mut sender, mut receiver) = socket.split();
    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(evt) => {
                        let payload = match serde_json::to_string(&evt) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!(error = %e, "ws_events: serialise failed");
                                continue;
                            }
                        };
                        if sender.send(Message::Text(payload)).await.is_err() {
                            // Client disconnected.
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        // Slow client. Drop them — Dev 3's frontend
                        // auto-reconnects and re-subscribes; better
                        // than buffering unboundedly server-side.
                        tracing::warn!(skipped = n, "ws_events: subscriber lagged; dropping");
                        let _ = sender.send(Message::Close(None)).await;
                        break;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            // Ignore any incoming frames from the client. The endpoint
            // is server-push only; we only watch for the close so we
            // can reap the task.
            client_msg = receiver.next() => {
                match client_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    Some(Ok(_)) => {} // ignore pings, text, binary
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viz_event_serialises_to_dev3_contract_shape() {
        let evt = VizEvent::DecisionMade {
            agent_id: "research-agent-01".into(),
            decision: DecisionKind::Allow,
            deny_code: None,
            ts_ms: 1_714_606_800_000,
        };
        let json = serde_json::to_value(&evt).unwrap();
        assert_eq!(json["kind"], "decision.made");
        assert_eq!(json["agent_id"], "research-agent-01");
        assert_eq!(json["decision"], "allow");
        assert!(
            json.get("deny_code").is_none(),
            "deny_code omitted when None"
        );
        assert_eq!(json["ts_ms"], 1_714_606_800_000_i64);
    }

    #[test]
    fn deny_code_serialises_when_present() {
        let evt = VizEvent::DecisionMade {
            agent_id: "research-agent-01".into(),
            decision: DecisionKind::Deny,
            deny_code: Some("policy.budget_exceeded".into()),
            ts_ms: 1_714_606_800_000,
        };
        let json = serde_json::to_value(&evt).unwrap();
        assert_eq!(json["decision"], "deny");
        assert_eq!(json["deny_code"], "policy.budget_exceeded");
    }

    #[test]
    fn first_seen_agent_returns_true_then_false() {
        let bus = WsEventsBus::new();
        assert!(bus.first_seen_agent("a-01"));
        assert!(!bus.first_seen_agent("a-01"));
        assert!(bus.first_seen_agent("a-02"));
    }

    #[test]
    fn publish_with_no_subscribers_does_not_panic() {
        let bus = WsEventsBus::new();
        bus.publish(VizEvent::DecisionMade {
            agent_id: "x".into(),
            decision: DecisionKind::Allow,
            deny_code: None,
            ts_ms: 0,
        });
    }

    #[test]
    fn subscriber_receives_published_event() {
        let bus = WsEventsBus::new();
        let mut rx = bus.subscribe();
        bus.publish(VizEvent::DecisionMade {
            agent_id: "x".into(),
            decision: DecisionKind::Allow,
            deny_code: None,
            ts_ms: 42,
        });
        let got = rx
            .try_recv()
            .expect("subscriber must receive after publish");
        let json = serde_json::to_value(&got).unwrap();
        assert_eq!(json["ts_ms"], 42);
    }
}
