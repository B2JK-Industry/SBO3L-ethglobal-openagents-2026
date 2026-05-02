//! R13 P6 — operator-facing WebSocket event stream at
//! `GET /v1/admin/events`.
//!
//! Sister to [`crate::ws_events`] but tailored for an operator audience
//! (admin dashboards, alerting feeders) rather than the trust-dns viz.
//! Three differences:
//!
//! 1. **Server-side filter via query params.** A subscriber can ask for
//!    only `?tenant=X&decision=deny&severity=warn|error` so an admin
//!    looking at a single tenant doesn't have to client-side-filter
//!    every event. Filters are applied BEFORE the WS frame is sent.
//! 2. **Cursor reconnect via `?since_id=N`.** Each event is tagged with
//!    a monotonically increasing `id`. The bus keeps a fixed-size ring
//!    buffer of the last [`RING_CAPACITY`] events; on reconnect with
//!    `since_id`, the handler replays everything in the ring with
//!    `id > since_id` (filtered), then transitions to live broadcast.
//!    If `since_id` is older than the oldest entry in the ring, the
//!    handler closes with code `4409` ("cursor_expired") so the client
//!    knows to rebootstrap from `/v1/audit/...` rather than blindly
//!    treating the gap as zero-events.
//! 3. **Severity dimension.** Operators care about deny + denied-by-budget
//!    + executor-failure events more than allows. Severity defaults to
//!      `info`; deny becomes `warn`; pipeline error becomes `error`.
//!
//! # Wire format
//!
//! ```json
//! { "id": 42, "kind": "decision", "tenant_id": "default",
//!   "agent_id": "research-agent-01", "decision": "allow",
//!   "deny_code": null, "severity": "info",
//!   "audit_event_hash": "0x...", "chain_seq": 17, "ts_ms": 1714... }
//! ```
//!
//! Stable additive surface — new fields ship with `Option<>` defaults so
//! older subscribers ignore them.
//!
//! # Out of scope for this PR (R13 P6)
//!
//! - Hosted-app `/admin/audit` integration (Dev 3's territory; this PR
//!   ships the backend-side stream the hosted-app will subscribe to).
//! - 1000-concurrent sustained perf test. Broadcast capacity is set to
//!   [`BROADCAST_CAPACITY`]; tuning past that needs a separate perf pass.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::AppState;

/// How many recent events the bus keeps for cursor replay. Ring is
/// shared across all subscribers; a slow operator reconnecting after
/// >RING_CAPACITY events will get a `cursor_expired` close.
pub const RING_CAPACITY: usize = 1024;

/// SPMC fan-out capacity for connected subscribers. A subscriber that
/// can't drain its receiver fast enough will see `RecvError::Lagged`
/// and be disconnected; admin clients reconnect with a `since_id`
/// cursor so missing events get replayed from the ring.
pub const BROADCAST_CAPACITY: usize = 4096;

/// WebSocket close code used when the requested `since_id` is older
/// than the oldest entry the ring still holds. Picked from the
/// application-private 4000-4999 range (RFC 6455 §7.4).
pub const CLOSE_CURSOR_EXPIRED: u16 = 4409;

/// Severity bucket. JSON-serialised as the lowercase variant name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

impl Severity {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "info" => Some(Self::Info),
            "warn" | "warning" => Some(Self::Warn),
            "error" | "err" => Some(Self::Error),
            _ => None,
        }
    }
}

/// Decision dimension exposed to admins. Tracks the same allow/deny
/// shape as `crate::ws_events::DecisionKind` but with explicit JSON
/// tagging — admin clients tend to filter by this field directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdminDecision {
    Allow,
    Deny,
}

impl AdminDecision {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "allow" => Some(Self::Allow),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }
}

/// One operator-facing event. Always carries `id` (monotonic) +
/// `tenant_id` + `severity` so server-side filters can apply uniformly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum AdminEvent {
    /// Pipeline produced a final allow/deny decision.
    #[serde(rename = "decision")]
    Decision {
        id: u64,
        tenant_id: String,
        agent_id: String,
        decision: AdminDecision,
        deny_code: Option<String>,
        severity: Severity,
        audit_event_hash: String,
        chain_seq: u64,
        ts_ms: i64,
    },
    /// Out-of-band operational event — e.g. signer rotation, feature
    /// flag change. Reserved for future hooks; the variant exists so
    /// admin dashboards can render it without a schema break when the
    /// backend starts emitting it. The `op_kind` field is renamed
    /// from `kind` (the natural label) to avoid colliding with the
    /// serde-tag discriminator on the outer enum.
    #[serde(rename = "operational")]
    Operational {
        id: u64,
        tenant_id: String,
        op_kind: String,
        message: String,
        severity: Severity,
        ts_ms: i64,
    },
}

impl AdminEvent {
    pub fn id(&self) -> u64 {
        match self {
            Self::Decision { id, .. } => *id,
            Self::Operational { id, .. } => *id,
        }
    }

    pub fn tenant_id(&self) -> &str {
        match self {
            Self::Decision { tenant_id, .. } => tenant_id,
            Self::Operational { tenant_id, .. } => tenant_id,
        }
    }

    pub fn severity(&self) -> Severity {
        match self {
            Self::Decision { severity, .. } => *severity,
            Self::Operational { severity, .. } => *severity,
        }
    }

    pub fn decision_kind(&self) -> Option<AdminDecision> {
        match self {
            Self::Decision { decision, .. } => Some(*decision),
            Self::Operational { .. } => None,
        }
    }
}

/// Subscriber filter compiled from `?tenant=&decision=&severity=`. A
/// `None` field means "don't filter on this dimension". Multiple
/// dimensions AND together; e.g. `?decision=deny&severity=warn`
/// matches `decision=deny AND severity=warn`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AdminEventsQuery {
    /// Tenant id; matches `AdminEvent.tenant_id` exactly. Empty string
    /// is treated as "no filter" so a clean `?tenant=` form does not
    /// silently match nothing.
    #[serde(default)]
    pub tenant: Option<String>,
    /// `allow` | `deny`. Other values are ignored (no filter applied)
    /// rather than rejected — admin tooling tends to ship loose query
    /// strings.
    #[serde(default)]
    pub decision: Option<String>,
    /// `info` | `warn` | `error`. Single value only — multi-severity
    /// filtering is client-side until we see a real demand signal.
    #[serde(default)]
    pub severity: Option<String>,
    /// Cursor: replay every ring entry with `id > since_id` before
    /// transitioning to live broadcast.
    #[serde(default)]
    pub since_id: Option<u64>,
}

/// Compiled filter — strings parsed once at handshake time so we don't
/// re-parse on every event.
#[derive(Debug, Clone, Default)]
struct CompiledFilter {
    tenant: Option<String>,
    decision: Option<AdminDecision>,
    severity: Option<Severity>,
}

impl CompiledFilter {
    fn from_query(q: &AdminEventsQuery) -> Self {
        let tenant = q.tenant.as_ref().filter(|s| !s.is_empty()).cloned();
        let decision = q.decision.as_ref().and_then(|s| AdminDecision::parse(s));
        let severity = q.severity.as_ref().and_then(|s| Severity::parse(s));
        Self {
            tenant,
            decision,
            severity,
        }
    }

    fn matches(&self, event: &AdminEvent) -> bool {
        if let Some(t) = &self.tenant {
            if event.tenant_id() != t {
                return false;
            }
        }
        if let Some(d) = self.decision {
            match event.decision_kind() {
                Some(ed) if ed == d => {}
                // operational events don't have a decision; if the
                // filter requires a specific decision, drop them.
                _ => return false,
            }
        }
        if let Some(s) = self.severity {
            if event.severity() != s {
                return false;
            }
        }
        true
    }
}

/// Admin events publish bus. Holds:
///
/// - `next_id`: monotonic event-id allocator. Wraps at `u64::MAX`
///   (~10^19) which we treat as unreachable.
/// - `tx`: SPMC broadcast channel for live subscribers.
/// - `ring`: bounded VecDeque of recent events for cursor replay.
pub struct AdminEventsBus {
    next_id: AtomicU64,
    tx: broadcast::Sender<AdminEvent>,
    ring: Mutex<VecDeque<AdminEvent>>,
}

impl AdminEventsBus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            next_id: AtomicU64::new(1),
            tx,
            ring: Mutex::new(VecDeque::with_capacity(RING_CAPACITY)),
        }
    }

    /// Allocate the next monotonic id. Caller is responsible for
    /// stamping it into the event before [`publish`].
    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Subscribe to live events. Caller drives `Receiver::recv`.
    pub fn subscribe(&self) -> broadcast::Receiver<AdminEvent> {
        self.tx.subscribe()
    }

    /// Publish an event: append to the ring (evicting the oldest if
    /// full) AND fan out to live subscribers. Errors from broadcast
    /// (no subscribers connected) are intentionally swallowed — the
    /// ring still records the event for later cursor replay.
    pub fn publish(&self, event: AdminEvent) {
        {
            let mut ring = self.ring.lock().expect("admin_events ring lock poisoned");
            if ring.len() >= RING_CAPACITY {
                ring.pop_front();
            }
            ring.push_back(event.clone());
        }
        let _ = self.tx.send(event);
    }

    /// Snapshot the ring's id span — used by the handler to detect
    /// `cursor_expired`. Returns `(oldest_id, newest_id)` or `None`
    /// if the ring is empty.
    fn ring_id_span(&self) -> Option<(u64, u64)> {
        let ring = self.ring.lock().expect("admin_events ring lock poisoned");
        let first = ring.front()?.id();
        let last = ring.back()?.id();
        Some((first, last))
    }

    /// Drain ring entries with `id > since_id` matching `filter`.
    /// Returns the matching events in order (oldest first).
    fn replay_since(&self, since_id: u64, filter: &CompiledFilter) -> Vec<AdminEvent> {
        let ring = self.ring.lock().expect("admin_events ring lock poisoned");
        ring.iter()
            .filter(|e| e.id() > since_id && filter.matches(e))
            .cloned()
            .collect()
    }
}

impl Default for AdminEventsBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience: emit a `decision` event for a finalized pipeline run.
/// Severity defaults to `info` for allow, `warn` for deny.
pub fn publish_decision(
    bus: &AdminEventsBus,
    tenant_id: &str,
    agent_id: &str,
    decision: AdminDecision,
    deny_code: Option<String>,
    chain_seq: u64,
    audit_event_hash: &str,
) {
    let severity = match decision {
        AdminDecision::Allow => Severity::Info,
        AdminDecision::Deny => Severity::Warn,
    };
    let id = bus.next_id();
    let ts_ms = chrono::Utc::now().timestamp_millis();
    bus.publish(AdminEvent::Decision {
        id,
        tenant_id: tenant_id.to_string(),
        agent_id: agent_id.to_string(),
        decision,
        deny_code,
        severity,
        audit_event_hash: audit_event_hash.to_string(),
        chain_seq,
        ts_ms,
    });
}

/// `GET /v1/admin/events` — upgrade to WS, apply filter, optionally
/// replay from cursor, then forward live broadcast frames.
pub async fn admin_events_handler(
    State(state): State<AppState>,
    Query(q): Query<AdminEventsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let bus = state.0.admin_events.clone();
    let filter = CompiledFilter::from_query(&q);
    let since_id = q.since_id;
    ws.on_upgrade(move |socket| async move {
        let bus = match bus {
            Some(b) => b,
            None => {
                tracing::warn!("admin_events route hit without bus initialised");
                let _ = socket.close().await;
                return;
            }
        };

        // Cursor handling — must compute BEFORE subscribing to live
        // broadcast to avoid a double-emit window where an event
        // lands in both the ring snapshot and the live receiver.
        // We snapshot the ring under lock, then subscribe; any event
        // published after subscribe() is delivered via the receiver.
        let replay = match (since_id, bus.ring_id_span()) {
            (Some(want), Some((oldest, _newest))) if want < oldest.saturating_sub(1) => {
                close_cursor_expired(socket, want, oldest).await;
                return;
            }
            (Some(want), _) => bus.replay_since(want, &filter),
            (None, _) => Vec::new(),
        };
        let mut rx = bus.subscribe();

        let (mut sender, mut receiver) = socket.split();

        // 1. Drain replay buffer first (oldest → newest, filtered).
        for evt in replay {
            let payload = match serde_json::to_string(&evt) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "admin_events: serialise replay failed");
                    continue;
                }
            };
            if sender.send(Message::Text(payload)).await.is_err() {
                return;
            }
        }

        // 2. Forward live events with filter applied.
        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Ok(evt) => {
                            if !filter.matches(&evt) {
                                continue;
                            }
                            let payload = match serde_json::to_string(&evt) {
                                Ok(s) => s,
                                Err(e) => {
                                    tracing::warn!(error = %e, "admin_events: serialise failed");
                                    continue;
                                }
                            };
                            if sender.send(Message::Text(payload)).await.is_err() {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            // Slow subscriber: send a sentinel so the
                            // client knows to reconnect with the
                            // last-seen id, then close.
                            tracing::warn!(skipped = n, "admin_events: subscriber lagged; closing");
                            let _ = sender
                                .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                                    code: CLOSE_CURSOR_EXPIRED,
                                    reason: format!("lagged_{n}").into(),
                                })))
                                .await;
                            break;
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                // Detect peer disconnect promptly so we don't keep
                // forwarding into a dead socket.
                msg = receiver.next() => {
                    match msg {
                        None | Some(Err(_)) => break,
                        Some(Ok(Message::Close(_))) => break,
                        Some(Ok(_)) => continue,
                    }
                }
            }
        }
    })
}

async fn close_cursor_expired(socket: WebSocket, want: u64, oldest: u64) {
    let (mut sender, _receiver) = socket.split();
    let reason = format!("cursor_expired_want_{want}_oldest_{oldest}");
    let _ = sender
        .send(Message::Close(Some(axum::extract::ws::CloseFrame {
            code: CLOSE_CURSOR_EXPIRED,
            reason: reason.into(),
        })))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: u64, tenant: &str, dec: AdminDecision, sev: Severity) -> AdminEvent {
        AdminEvent::Decision {
            id,
            tenant_id: tenant.to_string(),
            agent_id: "agent-x".to_string(),
            decision: dec,
            deny_code: None,
            severity: sev,
            audit_event_hash: "0xdeadbeef".to_string(),
            chain_seq: id,
            ts_ms: 0,
        }
    }

    #[test]
    fn filter_no_query_matches_everything() {
        let f = CompiledFilter::from_query(&AdminEventsQuery::default());
        assert!(f.matches(&ev(1, "t1", AdminDecision::Allow, Severity::Info)));
        assert!(f.matches(&ev(2, "t2", AdminDecision::Deny, Severity::Error)));
    }

    #[test]
    fn filter_tenant_excludes_other_tenants() {
        let f = CompiledFilter::from_query(&AdminEventsQuery {
            tenant: Some("t1".to_string()),
            ..Default::default()
        });
        assert!(f.matches(&ev(1, "t1", AdminDecision::Allow, Severity::Info)));
        assert!(!f.matches(&ev(2, "t2", AdminDecision::Allow, Severity::Info)));
    }

    #[test]
    fn filter_empty_tenant_string_is_no_filter() {
        let f = CompiledFilter::from_query(&AdminEventsQuery {
            tenant: Some("".to_string()),
            ..Default::default()
        });
        // Empty string must NOT silently match nothing.
        assert!(f.matches(&ev(1, "t1", AdminDecision::Allow, Severity::Info)));
        assert!(f.matches(&ev(2, "t2", AdminDecision::Allow, Severity::Info)));
    }

    #[test]
    fn filter_decision_excludes_other_decisions() {
        let f = CompiledFilter::from_query(&AdminEventsQuery {
            decision: Some("deny".to_string()),
            ..Default::default()
        });
        assert!(!f.matches(&ev(1, "t", AdminDecision::Allow, Severity::Info)));
        assert!(f.matches(&ev(2, "t", AdminDecision::Deny, Severity::Warn)));
    }

    #[test]
    fn filter_decision_excludes_operational_events() {
        let f = CompiledFilter::from_query(&AdminEventsQuery {
            decision: Some("allow".to_string()),
            ..Default::default()
        });
        let op = AdminEvent::Operational {
            id: 1,
            tenant_id: "t".to_string(),
            op_kind: "signer.rotate".to_string(),
            message: "rotated".to_string(),
            severity: Severity::Info,
            ts_ms: 0,
        };
        assert!(!f.matches(&op));
    }

    #[test]
    fn filter_invalid_decision_is_no_filter() {
        // Loose parsing — gibberish in the query string shouldn't
        // 4xx the WS upgrade. We just don't apply the dimension.
        let f = CompiledFilter::from_query(&AdminEventsQuery {
            decision: Some("maybe".to_string()),
            ..Default::default()
        });
        assert!(f.matches(&ev(1, "t", AdminDecision::Allow, Severity::Info)));
        assert!(f.matches(&ev(2, "t", AdminDecision::Deny, Severity::Warn)));
    }

    #[test]
    fn filter_severity_excludes_other_severities() {
        let f = CompiledFilter::from_query(&AdminEventsQuery {
            severity: Some("error".to_string()),
            ..Default::default()
        });
        assert!(!f.matches(&ev(1, "t", AdminDecision::Deny, Severity::Warn)));
        assert!(f.matches(&ev(2, "t", AdminDecision::Deny, Severity::Error)));
    }

    #[test]
    fn filter_combined_dimensions_and_together() {
        let f = CompiledFilter::from_query(&AdminEventsQuery {
            tenant: Some("t1".to_string()),
            decision: Some("deny".to_string()),
            severity: Some("warn".to_string()),
            ..Default::default()
        });
        assert!(f.matches(&ev(1, "t1", AdminDecision::Deny, Severity::Warn)));
        // Same severity + decision but wrong tenant.
        assert!(!f.matches(&ev(2, "t2", AdminDecision::Deny, Severity::Warn)));
        // Same tenant + decision but wrong severity.
        assert!(!f.matches(&ev(3, "t1", AdminDecision::Deny, Severity::Info)));
        // Same tenant + severity but wrong decision.
        assert!(!f.matches(&ev(4, "t1", AdminDecision::Allow, Severity::Warn)));
    }

    #[test]
    fn ring_evicts_oldest_when_full() {
        let bus = AdminEventsBus::new();
        // Fill ring + 5 extras.
        for _ in 0..(RING_CAPACITY + 5) {
            publish_decision(
                &bus,
                "default",
                "agent-x",
                AdminDecision::Allow,
                None,
                0,
                "0xff",
            );
        }
        let (oldest, newest) = bus.ring_id_span().unwrap();
        // The ring should now hold ids [6, RING_CAPACITY+5].
        assert_eq!(newest - oldest + 1, RING_CAPACITY as u64);
        assert_eq!(newest, (RING_CAPACITY + 5) as u64);
    }

    #[test]
    fn replay_since_returns_events_after_cursor() {
        let bus = AdminEventsBus::new();
        for _ in 0..10 {
            publish_decision(
                &bus,
                "default",
                "agent-x",
                AdminDecision::Allow,
                None,
                0,
                "0xff",
            );
        }
        let f = CompiledFilter::default();
        let replayed = bus.replay_since(7, &f);
        let ids: Vec<u64> = replayed.iter().map(|e| e.id()).collect();
        assert_eq!(ids, vec![8, 9, 10]);
    }

    #[test]
    fn replay_since_applies_filter() {
        let bus = AdminEventsBus::new();
        publish_decision(&bus, "t1", "agent-x", AdminDecision::Allow, None, 0, "0xff");
        publish_decision(&bus, "t2", "agent-x", AdminDecision::Allow, None, 0, "0xff");
        publish_decision(
            &bus,
            "t1",
            "agent-x",
            AdminDecision::Deny,
            Some("policy".to_string()),
            0,
            "0xff",
        );
        let f = CompiledFilter::from_query(&AdminEventsQuery {
            tenant: Some("t1".to_string()),
            ..Default::default()
        });
        let replayed = bus.replay_since(0, &f);
        let tenants: Vec<&str> = replayed.iter().map(|e| e.tenant_id()).collect();
        assert_eq!(tenants, vec!["t1", "t1"]);
    }

    #[test]
    fn next_id_is_monotonic_and_starts_at_one() {
        let bus = AdminEventsBus::new();
        assert_eq!(bus.next_id(), 1);
        assert_eq!(bus.next_id(), 2);
        assert_eq!(bus.next_id(), 3);
    }

    #[test]
    fn publish_decision_sets_severity_from_decision() {
        let bus = AdminEventsBus::new();
        publish_decision(
            &bus,
            "default",
            "agent-x",
            AdminDecision::Allow,
            None,
            0,
            "0xff",
        );
        publish_decision(
            &bus,
            "default",
            "agent-x",
            AdminDecision::Deny,
            Some("policy.budget".to_string()),
            0,
            "0xff",
        );
        let f = CompiledFilter::default();
        let evts = bus.replay_since(0, &f);
        assert_eq!(evts[0].severity(), Severity::Info);
        assert_eq!(evts[1].severity(), Severity::Warn);
    }

    #[test]
    fn admin_event_serialises_with_kind_tag() {
        let e = ev(42, "default", AdminDecision::Deny, Severity::Warn);
        let json = serde_json::to_string(&e).unwrap();
        // Stable wire format — admin clients pattern-match on `kind`.
        assert!(json.contains("\"kind\":\"decision\""));
        assert!(json.contains("\"id\":42"));
        assert!(json.contains("\"severity\":\"warn\""));
        assert!(json.contains("\"decision\":\"deny\""));
    }

    #[test]
    fn severity_parse_accepts_aliases() {
        assert_eq!(Severity::parse("info"), Some(Severity::Info));
        assert_eq!(Severity::parse("WARN"), Some(Severity::Warn));
        assert_eq!(Severity::parse("warning"), Some(Severity::Warn));
        assert_eq!(Severity::parse("Error"), Some(Severity::Error));
        assert_eq!(Severity::parse("err"), Some(Severity::Error));
        assert_eq!(Severity::parse("debug"), None);
    }
}
