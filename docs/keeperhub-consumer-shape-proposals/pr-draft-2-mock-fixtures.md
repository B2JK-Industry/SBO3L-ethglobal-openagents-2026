# Consumer-side adapter shape — for KH issue #53

This file shows what the test suite + adapter wiring in `crates/sbo3l-keeperhub-adapter/` would look like once [KeeperHub/cli#53](https://github.com/KeeperHub/cli/issues/53) lands a public mock fixture suite (Option B) or a `keeperhub-mock` Docker image (Option A).

## Status: draft. Blocked on KeeperHub/cli#53.

## Option A — `keeperhub-mock` Docker image

Adapter test job in `.github/workflows/keeperhub-adapter-test.yml`:

```yaml
jobs:
  adapter-live-against-mock:
    runs-on: ubuntu-latest
    services:
      keeperhub-mock:
        image: ghcr.io/keeperhub/mock:latest
        ports: ['18080:8080']
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: cargo test --features live-against-mock
        env:
          SBO3L_KEEPERHUB_WEBHOOK_URL: http://localhost:18080/api/workflows/test/webhook
          SBO3L_KEEPERHUB_TOKEN: wfb_mocktoken
        run: |
          cargo test -p sbo3l-keeperhub-adapter --features live-against-mock
```

`Cargo.toml` adds:

```toml
[features]
live-against-mock = []  # gates tests that need the keeperhub-mock service
```

Tests under `crates/sbo3l-keeperhub-adapter/tests/live_against_mock.rs`:

```rust
#[cfg(feature = "live-against-mock")]
#[test]
fn live_submit_against_kh_mock() {
    let exec = KeeperHubExecutor::live();
    let receipt = make_signed_receipt(Decision::Allow);
    let request = make_request();

    let result = exec.execute(&request, &receipt).expect("kh-mock should accept");
    assert_eq!(result.sponsor, "keeperhub");
    assert!(!result.mock);
    assert!(result.execution_ref.starts_with("kh-"));
}
```

This deletes ~80 lines of in-process `mockito::Server` setup currently in the test file, replacing with one declarative service line in CI.

## Option B — public JSON fixture suite

`crates/sbo3l-keeperhub-adapter/Cargo.toml` adds:

```toml
[dev-dependencies]
keeperhub-fixtures = "2026.5.1"  # published by KH; pinned to schema date
```

Tests under `crates/sbo3l-keeperhub-adapter/tests/parser_fixtures.rs`:

```rust
use keeperhub_fixtures::responses::*;

#[test]
fn parser_handles_canonical_201_minimal() {
    let body = include_str!(WEBHOOK_SUBMIT_201_MINIMAL_PATH);
    let parsed = parse_response_body(body).unwrap();
    assert!(parsed.execution_id.starts_with("kh-"));
}

#[test]
fn parser_handles_201_with_metadata() {
    let body = include_str!(WEBHOOK_SUBMIT_201_WITH_METADATA_PATH);
    let parsed = parse_response_body(body).unwrap();
    assert!(parsed.execution_id.starts_with("kh-"));
    assert!(parsed.metadata.is_some());
}

#[test]
fn parser_rejects_malformed_envelope_400() {
    let body = include_str!(WEBHOOK_SUBMIT_400_ENVELOPE_MALFORMED_PATH);
    let err = parse_response_body(body).unwrap_err();
    assert_eq!(err.code, "kh.envelope.malformed");
}
```

This removes the `executionId` → `id` fallback (`crates/sbo3l-keeperhub-adapter/src/lib.rs:316-319`) — pinned schema means we know which key to read.

## Test count delta

Today: 18 tests in adapter, all using in-process `mockito::Server` with hand-rolled response bodies.

After this PR + #53 fixtures: 18 → ~30 tests (12 new fixture-driven). 18 existing stay; 0 deleted (the in-process mockito stays for the `local_mock` path).

## Why this is a draft today

We can't ship the test suite until either:
- A public `ghcr.io/keeperhub/mock` image exists (not today)
- A `keeperhub-fixtures` crate is published (not today)

Once either lands, this PR converts to a real test addition.
