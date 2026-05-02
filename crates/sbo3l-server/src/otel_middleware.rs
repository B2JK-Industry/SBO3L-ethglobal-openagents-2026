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

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use tracing::field::Empty;

/// axum middleware that opens a per-request span. Apply via
/// `axum::middleware::from_fn(trace_request)` — see
/// [`crate::router_with_otel`] in `lib.rs`.
pub async fn trace_request(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let target = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

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
    let _enter = span.enter();
    drop(_enter);

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
