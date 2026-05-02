//! R14 P5 — per-request OpenTelemetry span enrichment middleware.
//!
//! Wraps every axum request in a `tracing` span carrying:
//! - `http.method`
//! - `http.target` (path + query)
//! - `http.status_code` (recorded after the handler returns)
//! - `sbo3l.tenant_id` (placeholder; populated when tenant headers land)
//! - `sbo3l.audit_event_id` (populated by the handler via
//!   `tracing::Span::current().record("sbo3l.audit_event_id", id)`)
//!
//! Only compiled with `--features otel`. Without the feature the
//! middleware doesn't exist and no per-request span overhead is paid.
//!
//! # Codex review fixes (#330)
//!
//! - **P1: trace context propagation.** Extracts W3C TraceContext
//!   (`traceparent` + `tracestate`) and baggage from inbound request
//!   headers via the global text-map propagator, and sets the resulting
//!   parent context on the new request span. Cross-service traces
//!   stitch together as expected when an upstream service is also
//!   OTEL-instrumented.
//! - **P2: runtime exporter gate.** When `SBO3L_OTEL_EXPORTER=none`
//!   (or unset) at process startup, the middleware fast-returns without
//!   building a span at all — same shape as the no-features path.
//!   Cached in a `OnceLock` so the env-read happens once per process.

use std::sync::OnceLock;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use opentelemetry::propagation::Extractor;
use tracing::field::Empty;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Cached "is OTEL exporting?" decision. Read once on first request via
/// `std::env::var(otel::ENV_EXPORTER)`. Tests that flip the env between
/// invocations need to re-init the process to pick up changes; that's
/// acceptable because the binary's startup path reads it exactly once
/// in `main` and the OnceLock matches that lifetime.
static OTEL_ACTIVE: OnceLock<bool> = OnceLock::new();

fn otel_active() -> bool {
    *OTEL_ACTIVE.get_or_init(|| {
        std::env::var(crate::otel::ENV_EXPORTER)
            .ok()
            .map(|v| v.trim().to_ascii_lowercase())
            .map(|v| !v.is_empty() && v != "none")
            .unwrap_or(false)
    })
}

/// Adapter that lets `opentelemetry::global::get_text_map_propagator`
/// pull headers out of an axum/hyper `HeaderMap`. Read-only; we never
/// write back through it.
struct HeaderExtractor<'a>(&'a axum::http::HeaderMap);

impl<'a> Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// axum middleware that opens a per-request span. Apply via
/// `axum::middleware::from_fn(trace_request)` — see `router()` in
/// `lib.rs`.
pub async fn trace_request(req: Request, next: Next) -> Response {
    // Codex P2: runtime gate. When OTEL is off at startup, skip the
    // span build entirely so this middleware has zero overhead in
    // deployments that compile with `--features otel` but disable
    // exporting via env.
    if !otel_active() {
        return next.run(req).await;
    }

    let method = req.method().clone();
    let target = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

    // Codex P1: extract upstream trace context from headers BEFORE
    // we build our span, so cross-service traces stitch together.
    // The global propagator is set in `otel::init_tracer` to W3C
    // TraceContext + Baggage; if no exporter ran, the propagator is
    // a no-op `NoopTextMapPropagator` and the extracted context is
    // empty (which `set_parent` accepts gracefully).
    let parent_cx = opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(req.headers()))
    });

    // Open the span with `Empty` placeholders for the fields that
    // either get filled in by the handler (`sbo3l.audit_event_id`,
    // `sbo3l.tenant_id`) or after the handler returns
    // (`http.status_code`). The `tracing-opentelemetry` layer picks
    // the span up via the global subscriber and ships it to the
    // configured exporter.
    let span = tracing::info_span!(
        "http.request",
        http.method = %method,
        http.target = %target,
        http.status_code = Empty,
        sbo3l.tenant_id = Empty,
        sbo3l.audit_event_id = Empty,
    );
    let _ = span.set_parent(parent_cx);

    // Run the handler under the span. We use `Instrument` for the
    // future so any `tracing::info!()` inside the handler attaches to
    // this span automatically.
    use tracing::Instrument;
    let response = next.run(req).instrument(span.clone()).await;

    // Record the resolved status code after the handler returns. If
    // the span has already been recorded by the handler (e.g., a
    // `tracing::Span::current().record(...)` call inside the
    // pipeline), those values are preserved; we only fill the status.
    span.record("http.status_code", response.status().as_u16());

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `otel_active` reads the env once and caches. We can't reliably
    /// test both true + false outcomes from a single process (OnceLock
    /// is one-shot) — pin one branch and document the other.
    #[test]
    fn otel_active_caches_first_read() {
        // Force-init under the current env. After this, subsequent
        // calls return the same value without re-reading env. That's
        // the contract: process-lifetime constant.
        let first = otel_active();
        // Sanity: idempotent (no double-init panic).
        let second = otel_active();
        assert_eq!(first, second);
    }
}
