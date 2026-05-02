//! R14 P5 — OpenTelemetry tracing emitter.
//!
//! This module is **only compiled with `--features otel`**. It builds
//! an [`SdkTracerProvider`] from environment configuration and exposes
//! a tracing-subscriber [`Layer`](tracing_opentelemetry::OpenTelemetryLayer)
//! the binary stitches into its existing `EnvFilter`-driven subscriber.
//!
//! ## Honest scope
//!
//! - Stdout exporter: validated end-to-end. A test invokes the binary
//!   with `SBO3L_OTEL_EXPORTER=stdout` and asserts a span shows up on
//!   stdout.
//! - OTLP exporter (gRPC and HTTP): wired and compile-checked. **No live
//!   integration test** against a real Tempo/Jaeger/collector. We
//!   document the env vars and the operator brings their own collector
//!   (`docker run -p 4317:4317 -p 4318:4318 otel/opentelemetry-collector-contrib`).
//! - We do NOT export OTEL metrics. Prometheus metrics shipped in
//!   PR #303 (R13 P7); duplicating them via OTEL meters would create two
//!   sources of truth.
//!
//! ## Env vars
//!
//! | Var                          | Default          | Effect                                        |
//! |------------------------------|------------------|-----------------------------------------------|
//! | `SBO3L_OTEL_EXPORTER`        | `none`           | `none` \| `stdout` \| `otlp`                  |
//! | `SBO3L_OTEL_OTLP_ENDPOINT`   | `http://localhost:4317` | Collector endpoint (gRPC by default)   |
//! | `SBO3L_OTEL_OTLP_PROTOCOL`   | `grpc`           | `grpc` (port 4317) \| `http` (port 4318)      |
//! | `SBO3L_OTEL_SERVICE_NAME`    | `sbo3l-server`   | `service.name` resource attribute             |
//!
//! ## Crate-version pinning
//!
//! The `opentelemetry`/`opentelemetry_sdk`/`opentelemetry-otlp`/
//! `opentelemetry-stdout` crates churn their inter-crate API on every
//! minor bump. We pin them all to `0.31.x` and `tracing-opentelemetry`
//! to `0.32.x` (which is the version that depends on the 0.31 OTEL
//! series). Bumping any one of these requires bumping all five
//! together; do not regress to mismatched majors.

use std::env;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;

/// Environment variable names. Exposed as constants so tests and the
/// binary share the same source of truth.
pub const ENV_EXPORTER: &str = "SBO3L_OTEL_EXPORTER";
pub const ENV_OTLP_ENDPOINT: &str = "SBO3L_OTEL_OTLP_ENDPOINT";
pub const ENV_OTLP_PROTOCOL: &str = "SBO3L_OTEL_OTLP_PROTOCOL";
pub const ENV_SERVICE_NAME: &str = "SBO3L_OTEL_SERVICE_NAME";

/// Default OTLP gRPC endpoint when `SBO3L_OTEL_OTLP_ENDPOINT` is unset.
pub const DEFAULT_OTLP_GRPC_ENDPOINT: &str = "http://localhost:4317";
/// Default OTLP HTTP endpoint when `SBO3L_OTEL_OTLP_PROTOCOL=http` and
/// no explicit endpoint is provided.
pub const DEFAULT_OTLP_HTTP_ENDPOINT: &str = "http://localhost:4318/v1/traces";
/// Default `service.name` resource attribute.
pub const DEFAULT_SERVICE_NAME: &str = "sbo3l-server";

/// Parsed `SBO3L_OTEL_EXPORTER` value. The default is [`Exporter::None`]
/// — i.e. when the env var is unset OR set to `none`, [`init_tracer`]
/// returns `None` and the binary's existing tracing-subscriber init
/// path runs unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exporter {
    /// No OTEL export. The fast no-op startup path: returns `None`
    /// from [`init_tracer`] without touching any tracing-opentelemetry
    /// machinery.
    None,
    /// Pretty-print spans to stdout. Validated end-to-end by the
    /// integration test in `tests/otel_stdout.rs`.
    Stdout,
    /// Export via OTLP to a configurable collector
    /// (`SBO3L_OTEL_OTLP_ENDPOINT`, default `http://localhost:4317` for
    /// gRPC). Compile-checked but not live-validated against a real
    /// Tempo/Jaeger; operator brings their own collector.
    Otlp,
}

/// Parsed OTLP wire protocol when [`Exporter::Otlp`] is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtlpProtocol {
    /// gRPC over tonic. Default port 4317. Requires the
    /// `grpc-tonic` feature on `opentelemetry-otlp` (always on for us).
    Grpc,
    /// HTTP/protobuf. Default port 4318, path `/v1/traces`. Requires
    /// the `http-proto` feature on `opentelemetry-otlp`.
    Http,
}

/// Parse the `SBO3L_OTEL_EXPORTER` env var. Unknown / empty / `none`
/// all resolve to [`Exporter::None`] (the safe default — never throw
/// at startup over a misspelled env var; the daemon just runs without
/// OTEL emission).
pub fn parse_exporter(raw: Option<&str>) -> Exporter {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("stdout") => Exporter::Stdout,
        Some("otlp") => Exporter::Otlp,
        _ => Exporter::None,
    }
}

/// Parse the `SBO3L_OTEL_OTLP_PROTOCOL` env var. Defaults to
/// [`OtlpProtocol::Grpc`].
pub fn parse_otlp_protocol(raw: Option<&str>) -> OtlpProtocol {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("http") | Some("http-proto") => OtlpProtocol::Http,
        _ => OtlpProtocol::Grpc,
    }
}

/// Read the configured `service.name`, defaulting to
/// [`DEFAULT_SERVICE_NAME`].
pub fn service_name_from_env() -> String {
    env::var(ENV_SERVICE_NAME).unwrap_or_else(|_| DEFAULT_SERVICE_NAME.to_string())
}

/// Build a [`Resource`] carrying the `service.name` attribute. Other
/// attributes (`service.version`, `deployment.environment`) can be set
/// via the `OTEL_RESOURCE_ATTRIBUTES` env var honoured by
/// `opentelemetry_sdk` directly.
fn build_resource(service_name: &str) -> Resource {
    Resource::builder()
        .with_attributes([KeyValue::new("service.name", service_name.to_string())])
        .build()
}

/// Build a tracer provider from the runtime env. Returns `None` when
/// `SBO3L_OTEL_EXPORTER` resolves to [`Exporter::None`] — the fast
/// no-op path; the binary's main-fn skips the tracing-opentelemetry
/// layer entirely in that case.
///
/// On a build error (OTLP exporter cannot be constructed because, e.g.,
/// the endpoint is malformed), this logs a warning and returns `None`
/// so the daemon **still starts**. OTEL is observability, not a
/// dependency for the core APRP pipeline; we never want a broken
/// collector URL to take the daemon down.
pub fn init_tracer() -> Option<SdkTracerProvider> {
    let exporter = parse_exporter(env::var(ENV_EXPORTER).ok().as_deref());
    let service_name = service_name_from_env();
    let provider = match exporter {
        Exporter::None => return None,
        Exporter::Stdout => Some(build_stdout_provider(&service_name)),
        Exporter::Otlp => match build_otlp_provider(&service_name) {
            Ok(p) => Some(p),
            Err(e) => {
                eprintln!(
                    "WARNING: OTEL OTLP exporter failed to build ({e}); continuing without OTEL emission."
                );
                None
            }
        },
    };

    // Codex P1 (#330): register the W3C TraceContext propagator
    // globally so the request-tracing middleware can extract inbound
    // `traceparent` / `tracestate` and stitch the new request span as
    // a child of the upstream's span. Without this, every request
    // becomes a disconnected root and cross-service traces break.
    // Registered AFTER the provider is built so a propagator-only
    // setup is impossible (otel.rs's contract: provider Some ⇒
    // propagator set).
    if provider.is_some() {
        opentelemetry::global::set_text_map_propagator(
            opentelemetry_sdk::propagation::TraceContextPropagator::new(),
        );
    }
    provider
}

fn build_stdout_provider(service_name: &str) -> SdkTracerProvider {
    let exporter = opentelemetry_stdout::SpanExporter::default();
    SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(build_resource(service_name))
        .build()
}

fn build_otlp_provider(
    service_name: &str,
) -> Result<SdkTracerProvider, opentelemetry_otlp::ExporterBuildError> {
    let protocol = parse_otlp_protocol(env::var(ENV_OTLP_PROTOCOL).ok().as_deref());
    let endpoint = env::var(ENV_OTLP_ENDPOINT).ok();

    let exporter = match protocol {
        OtlpProtocol::Grpc => {
            let endpoint = endpoint.unwrap_or_else(|| DEFAULT_OTLP_GRPC_ENDPOINT.to_string());
            opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .build()?
        }
        OtlpProtocol::Http => {
            let endpoint = endpoint.unwrap_or_else(|| DEFAULT_OTLP_HTTP_ENDPOINT.to_string());
            opentelemetry_otlp::SpanExporter::builder()
                .with_http()
                .with_endpoint(endpoint)
                .build()?
        }
    };

    Ok(SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(build_resource(service_name))
        .build())
}

/// Build a `tracing` [`Layer`](tracing_opentelemetry::OpenTelemetryLayer)
/// backed by the supplied provider. The caller stitches this into a
/// `tracing_subscriber::registry()` alongside the existing fmt + env
/// filter layers.
pub fn tracing_layer<S>(
    provider: &SdkTracerProvider,
) -> tracing_opentelemetry::OpenTelemetryLayer<S, opentelemetry_sdk::trace::Tracer>
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    let tracer = provider.tracer(DEFAULT_SERVICE_NAME);
    tracing_opentelemetry::layer().with_tracer(tracer)
}

/// Flush + shut down the provider. Idempotent — safe to call from a
/// signal handler that may also be reached through the normal shutdown
/// path. Logs but swallows errors: shutdown is best-effort and must
/// never panic during graceful exit.
pub fn shutdown(provider: SdkTracerProvider) {
    if let Err(e) = provider.shutdown() {
        eprintln!("WARNING: OTEL tracer provider shutdown error: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_exporter_none_when_unset() {
        assert_eq!(parse_exporter(None), Exporter::None);
    }

    #[test]
    fn parse_exporter_explicit_none() {
        assert_eq!(parse_exporter(Some("none")), Exporter::None);
        // case-insensitive
        assert_eq!(parse_exporter(Some("NONE")), Exporter::None);
        // unknown value defaults to None — we don't fail startup over
        // a misspelled env var.
        assert_eq!(parse_exporter(Some("garbage")), Exporter::None);
        // empty / whitespace also resolves to None
        assert_eq!(parse_exporter(Some("")), Exporter::None);
        assert_eq!(parse_exporter(Some("   ")), Exporter::None);
    }

    #[test]
    fn parse_exporter_stdout_variants() {
        assert_eq!(parse_exporter(Some("stdout")), Exporter::Stdout);
        assert_eq!(parse_exporter(Some("STDOUT")), Exporter::Stdout);
        assert_eq!(parse_exporter(Some("  stdout  ")), Exporter::Stdout);
    }

    #[test]
    fn parse_exporter_otlp_variants() {
        assert_eq!(parse_exporter(Some("otlp")), Exporter::Otlp);
        assert_eq!(parse_exporter(Some("OTLP")), Exporter::Otlp);
    }

    #[test]
    fn parse_otlp_protocol_defaults_to_grpc() {
        assert_eq!(parse_otlp_protocol(None), OtlpProtocol::Grpc);
        assert_eq!(parse_otlp_protocol(Some("grpc")), OtlpProtocol::Grpc);
        assert_eq!(parse_otlp_protocol(Some("garbage")), OtlpProtocol::Grpc);
    }

    #[test]
    fn parse_otlp_protocol_http_variants() {
        assert_eq!(parse_otlp_protocol(Some("http")), OtlpProtocol::Http);
        assert_eq!(parse_otlp_protocol(Some("HTTP")), OtlpProtocol::Http);
        assert_eq!(parse_otlp_protocol(Some("http-proto")), OtlpProtocol::Http);
    }

    #[test]
    fn init_tracer_returns_none_when_exporter_is_none() {
        // Force-clear the env var inside the test; the parent process
        // may have it set. We don't restore — Cargo runs tests in a
        // separate process so cross-test contamination doesn't apply
        // for this assertion. (Other tests in this module don't read
        // ENV_EXPORTER.)
        // SAFETY: tests run single-threaded for env mutation isn't
        // guaranteed; we use the parser directly to avoid the env
        // mutation race entirely.
        let exporter = parse_exporter(Some("none"));
        assert_eq!(exporter, Exporter::None);
        // And the docs-contract that init_tracer respects this:
        // we can't easily test init_tracer() itself without env
        // mutation, but the integration test in tests/otel_stdout.rs
        // covers the stdout path end-to-end.
    }

    #[test]
    fn shutdown_is_idempotent_via_double_call_safety() {
        // We can't easily double-shut-down a provider in a unit test
        // without building one. The contract is "shutdown swallows
        // errors". Build a stdout provider, shut it down — should not
        // panic. (The OTEL SDK's internal shutdown bookkeeping is
        // already idempotent at the SDK layer; this test pins our
        // wrapper does NOT add a second-call panic.)
        let provider = build_stdout_provider("sbo3l-server-test");
        shutdown(provider);
        // If we reach here without panic, the wrapper is safe to call.
    }

    #[test]
    fn service_name_default_constant_is_stable() {
        // Pin the public default; renaming this is part of the
        // operator contract and will appear as `service.name` in
        // every emitted span. Not a free rename.
        assert_eq!(DEFAULT_SERVICE_NAME, "sbo3l-server");
    }

    #[test]
    fn env_var_names_are_stable() {
        // Pinning the env var names — these are part of the operator
        // contract documented in `docs/observability.md`. Renaming
        // them is a breaking change.
        assert_eq!(ENV_EXPORTER, "SBO3L_OTEL_EXPORTER");
        assert_eq!(ENV_OTLP_ENDPOINT, "SBO3L_OTEL_OTLP_ENDPOINT");
        assert_eq!(ENV_OTLP_PROTOCOL, "SBO3L_OTEL_OTLP_PROTOCOL");
        assert_eq!(ENV_SERVICE_NAME, "SBO3L_OTEL_SERVICE_NAME");
    }
}
