# SBO3L gRPC API

Status: scaffold (R14 P1). Three RPCs shipped — `Decide`, `Health`,
`AuditChainStream`. Lives alongside the existing REST surface; same
storage, same signers, same audit chain. Opt-in via the `grpc` cargo
feature.

## Wire definition

The canonical proto file is at [`proto/sbo3l.proto`](../../proto/sbo3l.proto)
(workspace root). Package: `sbo3l.v1`. Service: `Sbo3l`.

```proto
service Sbo3l {
  rpc Decide(DecideRequest) returns (DecideResponse);
  rpc Health(HealthRequest) returns (HealthResponse);
  rpc AuditChainStream(AuditChainRequest) returns (stream AuditChainEvent);
}
```

Three intentional choices encoded in the wire shape:

1. **`DecideRequest.aprp_json` is a string, not a structured message.**
   The APRP schema (`schemas/agent_payment_request_v1.json`) is the
   canonical source of truth and changes additively. Re-projecting
   every field into proto would force a round-trip-compatibility
   burden across two schemas; instead the caller serialises their
   APRP once and the server runs the existing
   `sbo3l_core::schema::validate_aprp` validator.

2. **`DecideResponse.receipt_json` is a string, not a structured
   message.** The `PolicyReceipt` is canonical-JSON-signed
   (Ed25519 over the JSON minus the signature field). A verifier must
   read the exact bytes the server signed; re-projecting into proto
   would either break verification or require shipping the JSON
   verbatim alongside the proto fields. We choose the latter by
   keeping the receipt as a JSON string.

3. **`AuditChainStream` is server-streaming, not unary + paginated.**
   The audit chain is a hash-linked sequence; streaming preserves
   ordering trivially. Pagination is handled by the
   `AuditChainRequest.since_seq` cursor — set to the last seq the
   client successfully consumed and resume.

## Build & run

```bash
# HTTP-only build (default; doesn't pull tonic).
cargo build -p sbo3l-server

# HTTP + gRPC build (additive; adds prost / tonic).
cargo build -p sbo3l-server --features grpc

# Run both REST and gRPC on different ports.
cargo run -p sbo3l-server --features grpc --bin sbo3l-server-grpc
```

Bind ports:

| Env var             | Default            | Surface |
| ------------------- | ------------------ | ------- |
| `SBO3L_LISTEN`      | `127.0.0.1:8730`   | REST    |
| `SBO3L_GRPC_LISTEN` | `127.0.0.1:8731`   | gRPC    |

Both surfaces share a single `AppState` — same in-memory `Storage`,
`AuditSigner`, `ReceiptSigner`, metrics registry. A request landing
on REST and a request landing on gRPC see identical state, including
the persistent nonce-replay store.

## Status code mapping

The REST surface returns RFC 7807 `Problem` responses; the gRPC
surface translates them to canonical `tonic::Status` codes. The
original Problem `code` (e.g. `protocol.nonce_replay`) is preserved
in `Status::message` so callers can branch on it without parsing
free-form text.

| HTTP                 | gRPC                  | Examples                                             |
| -------------------- | --------------------- | ---------------------------------------------------- |
| 400 Bad Request      | `INVALID_ARGUMENT`    | `schema.unknown_field`, `protocol.aprp_expired`      |
| 401 Unauthorized     | `UNAUTHENTICATED`     | (auth not yet wired in gRPC scaffold)                |
| 403 Forbidden        | `PERMISSION_DENIED`   |                                                      |
| 404 Not Found        | `NOT_FOUND`           |                                                      |
| 409 Conflict         | `ALREADY_EXISTS`      | `protocol.nonce_replay`, `protocol.idempotency_*`    |
| 429 Too Many         | `RESOURCE_EXHAUSTED`  |                                                      |
| 500 Internal Error   | `INTERNAL`            | `audit.write_failed`, `audit.signer_unavailable`     |
| 503 Service Unavail. | `UNAVAILABLE`         | storage mutex poisoned, `audit_count` failure        |

## Honest scope (what's NOT shipped here)

* **No interceptors, no auth, no rate limiting.** The auth middleware
  attached to the REST router is not yet ported to the gRPC pipeline.
  For now the gRPC surface trusts every caller. Run it on loopback
  only or behind a sidecar that handles auth.
* **No `Idempotency-Key` semantics.** gRPC's HTTP/2 trailers don't
  surface arbitrary headers cleanly through tonic, and the REST flow
  already covers safe-retry. Future revisions may add an
  `idempotency_key` field to `DecideRequest`.
* **No reflection / health proto integration.** Tonic's `tonic-reflection`
  + `tonic-health` are nice-to-have but optional — they'd need their
  own dependency stack and don't belong in the scaffold.
* **No bidirectional streaming.** Server-streaming on
  `AuditChainStream` is the only stream we ship; client-streaming and
  bidi RPCs aren't part of the scope.

## Clients

* **TypeScript:** [`sdks/grpc-ts`](../../sdks/grpc-ts/) — wraps
  `@grpc/grpc-js` + `@grpc/proto-loader`.
* **Example:** [`examples/grpc-ts`](../../examples/grpc-ts/) — connect,
  health, decide, walk audit chain.

A Rust client is generated alongside the server bindings via
`tonic-build` — see `crates/sbo3l-server/src/grpc.rs` `Sbo3lClient`. It's
exercised by `tests/grpc_e2e.rs` end-to-end over a real HTTP/2 channel.

## Tests

* `crates/sbo3l-server/src/grpc.rs::tests` — 11 unit tests covering
  Decide / Health / AuditChainStream / status mapping. Calls the
  service trait methods directly (no network).
* `crates/sbo3l-server/tests/grpc_e2e.rs` — 3 integration tests that
  spawn a real `tonic::transport::Server` on an ephemeral port and
  dial it back via the generated `Sbo3lClient`.

Run with:

```bash
cargo test -p sbo3l-server --features grpc
```

## Generated artefacts

The `tonic-build` invocation in [`crates/sbo3l-server/build.rs`](../../crates/sbo3l-server/build.rs)
emits `OUT_DIR/sbo3l.v1.rs` at compile time. The `protoc-bin-vendored`
crate ships a compiled `protoc` binary so builds work on hosts without
`protoc` on `PATH` — matching our CI + worktree pattern.
