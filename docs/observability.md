# Observability — SBO3L server

This runbook covers the daemon-side observability surfaces shipped today
and how to point them at your collector / dashboard stack.

## Two surfaces, two purposes

| Surface         | Wire format         | Endpoint                  | Use                                        |
|-----------------|---------------------|---------------------------|--------------------------------------------|
| Prometheus      | Text exposition     | `GET /v1/metrics`         | Counters, histograms (RPS, latency, decisions). |
| OpenTelemetry   | OTLP gRPC / HTTP / stdout | (push, exporter-driven) | Distributed traces (per-request spans).    |
| Admin JSON      | JSON                | `GET /v1/admin/metrics`   | Bundled snapshot for the `apps/observability` dashboard. |
| Healthz         | JSON                | `GET /v1/healthz`         | Liveness probe with audit-chain head + uptime. |

The Prometheus surface (PR #303) handles counters / histograms. The OTEL
surface (PR for R14 P5) handles distributed traces. **They are
complementary**, not redundant — counters tell you *what's slow*, traces
tell you *which request is slow and which downstream call inside the
pipeline took the time*.

## Enabling OpenTelemetry traces

OTEL is gated behind a Cargo feature so the default daemon build pulls
zero OTEL transitive dependencies. Build with the feature and select an
exporter at runtime via env vars.

### Build with the feature

```bash
cargo build -p sbo3l-server --features otel --release
```

### Env-var matrix

| Var                          | Default                 | Effect                                                         |
|------------------------------|-------------------------|----------------------------------------------------------------|
| `SBO3L_OTEL_EXPORTER`        | `none`                  | `none` \| `stdout` \| `otlp`                                   |
| `SBO3L_OTEL_OTLP_ENDPOINT`   | `http://localhost:4317` | Collector endpoint. gRPC port 4317 by default; for HTTP use `:4318/v1/traces`. |
| `SBO3L_OTEL_OTLP_PROTOCOL`   | `grpc`                  | `grpc` (port 4317) \| `http` (port 4318)                       |
| `SBO3L_OTEL_SERVICE_NAME`    | `sbo3l-server`          | `service.name` resource attribute attached to every span       |
| `OTEL_RESOURCE_ATTRIBUTES`   | (unset)                 | Standard OTEL var; honoured by `opentelemetry_sdk` directly. Use this for `service.version`, `deployment.environment`, etc. |

### Exporter modes

#### `none` (default, fast no-op)

```bash
sbo3l-server  # no OTEL emission, no overhead
```

`init_tracer()` returns `None` and the binary's existing
`tracing-subscriber::fmt` path runs unchanged. Zero per-request OTEL
overhead.

#### `stdout` (validated end-to-end)

```bash
SBO3L_OTEL_EXPORTER=stdout sbo3l-server
```

Spans pretty-print to stdout. Useful for quick sanity checks; not
intended for production. The integration test in
`crates/sbo3l-server/tests/otel_stdout.rs` pins this path: spawn the
binary with this env var, fire a request, assert a `Spans` block lands
on stdout.

#### `otlp` (compile-checked; bring your own collector)

```bash
SBO3L_OTEL_EXPORTER=otlp \
  SBO3L_OTEL_OTLP_ENDPOINT=http://localhost:4317 \
  sbo3l-server
```

The exporter is wired and compile-tested but **not** live-tested
against a real Tempo/Jaeger because we don't ship a docker-compose
collector bring-up — that's the operator's choice. We've validated
that:

- The OTLP gRPC and OTLP HTTP exporters both build cleanly under
  `--features otel`.
- The `tracing-opentelemetry` Layer attaches to the same `EnvFilter`-
  driven subscriber as the existing fmt layer.
- Per-request spans named `http.request` carry
  `http.method` / `http.target` / `http.status_code` plus an
  `sbo3l.audit_event_id` attribute populated by the pipeline handler.

To run a local collector:

```bash
docker run --rm -p 4317:4317 -p 4318:4318 \
  otel/opentelemetry-collector-contrib:latest
```

Or point at any OTLP-compatible endpoint: Tempo, Jaeger (via OTLP
receiver), Grafana Cloud Tempo, Honeycomb, Lightstep, Datadog.

#### Honest scope

We ship the OTEL **emitter**. The collector is the operator's choice.
We deliberately do not ship a Tempo/Jaeger/Loki bring-up in
`docker-compose.yml` — that mixes two concerns (daemon + collector) in a
way that makes it harder to swap collectors later. The runbook above is
the contract.

## Per-request span enrichment

When `--features otel` is on, every incoming HTTP request opens a
`tracing` span called `http.request` carrying:

- `http.method` — e.g. `POST`
- `http.target` — path + query, e.g. `/v1/payment-requests`
- `http.status_code` — recorded after the handler returns
- `sbo3l.tenant_id` — placeholder; populated when tenant headers land
- `sbo3l.audit_event_id` — populated by the `create_payment_request`
  handler at the moment the audit event is appended to the chain. This
  means a trace inspector can join an OTEL trace ID to an SBO3L audit
  chain entry by `audit_event_id`.

The middleware is mounted via `axum::middleware::from_fn` and **only
attached when `feature = "otel"` is on**, so the no-features build
pays zero per-request middleware overhead.

## Crate-version pinning

The `opentelemetry`/`opentelemetry_sdk`/`opentelemetry-otlp`/
`opentelemetry-stdout` crates churn their inter-crate API on every
minor bump. We pin them all to `0.31.x` and `tracing-opentelemetry` to
`0.32.x` (the version that depends on the 0.31 OTEL series). Bumping
any one of these requires bumping all five together. Future-Daniel: if
you see `error[E0432]: unresolved import` after a `cargo update`, you
forgot to coordinate the version bump across the five crates.

## Grafana dashboard

The 12-panel Grafana dashboard at
`apps/observability/grafana/dashboard.json` is backed mostly by the
Prometheus metrics exposed at `/v1/metrics` (PR #303). One panel is
reserved for OTEL/Tempo traces; that panel is empty until an operator
wires up Tempo and updates the datasource UID.

Import:

1. Grafana → Dashboards → New → Import.
2. Upload `apps/observability/grafana/dashboard.json`.
3. Select your Prometheus datasource.
4. (Optional) Select your Tempo / OTLP datasource for the trace panel.

The dashboard renders cleanly with placeholder data — judges can see
the full observability surface without a live deployment. Most panels
back metrics that already exist; the trace panel is documentation more
than running code at submission time.

## Troubleshooting

### Daemon starts with `SBO3L_OTEL_EXPORTER=otlp` but no spans land at the collector

- Confirm the collector is reachable: `nc -zv localhost 4317`.
- Confirm the protocol matches the port: 4317 = gRPC, 4318 = HTTP.
- The daemon never panics on a broken endpoint — it logs a warning to
  stderr (`OTEL OTLP exporter failed to build (...)`) and continues
  without OTEL emission. Check stderr.

### Daemon shutdown takes >1s

This is by design — the OTEL batch span processor drains in-flight
spans on shutdown so they aren't lost. SIGTERM triggers
`otel::shutdown()` which calls the SDK's `shutdown()`; the SDK waits
for the in-flight batch to flush before returning. A 1-3s drain is
expected.

### `cargo build -p sbo3l-server` (no features) pulls OTEL crates

It shouldn't. Verify with:

```bash
cargo tree -p sbo3l-server -e normal | grep opentelemetry
```

If anything matches, the feature gate has regressed; file a bug.
