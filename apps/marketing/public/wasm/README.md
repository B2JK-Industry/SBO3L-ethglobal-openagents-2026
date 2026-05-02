# SBO3L Passport WASM verifier

This directory holds the WebAssembly build of `sbo3l-core`'s capsule
verifier, served from the marketing site at `/wasm/sbo3l_core.js` and
loaded by the `/proof` page (#110, Dev 3).

## What's here

| File | Purpose |
|---|---|
| `sbo3l_core.js` | wasm-bindgen JS glue — `import` this from a browser script |
| `sbo3l_core_bg.wasm` | the compiled Rust verifier (~2.3 MiB unstripped) |
| `sbo3l_core.d.ts` | TypeScript declarations for the JS bindings |
| `sbo3l_core_bg.wasm.d.ts` | TypeScript declarations for the wasm exports |
| `package.json` | `wasm-pack`-emitted package manifest (not published; bundlers can resolve `import * from "./wasm/sbo3l_core.js"`) |

## JS surface

```ts
import init, {
  verify_capsule_json,
  verify_capsule_strict_json,
  sbo3l_core_version,
} from "/wasm/sbo3l_core.js";

await init(); // load the .wasm module

// Structural verify — null on success, throws on failure.
verify_capsule_json(JSON.stringify(capsule));

// Strict cryptographic verify — returns a 6-check report. v2
// self-contained capsules pass without auxiliary inputs; v1 capsules
// see SKIPPED on the aux-dependent checks.
const report = verify_capsule_strict_json(JSON.stringify(capsule));
// {
//   ok: boolean,
//   any_failed: boolean,
//   any_skipped: boolean,
//   checks: [
//     { label: "structural", outcome: "PASSED" | "SKIPPED" | "FAILED", detail?: string },
//     ...
//   ]
// }

console.log("verifier built from sbo3l-core", sbo3l_core_version());
```

## Build cadence

Re-run the build whenever `crates/sbo3l-core/src/wasm.rs`, the F-6
verifier (`crates/sbo3l-core/src/passport.rs`), or any transitive
schema/AuditBundle code changes.

```bash
bash scripts/build-wasm-verifier.sh
```

CI runs the same command on every PR (`.github/workflows/ci.yml ::
wasm-verifier-build`) and asserts the four artefacts exist; a stale
checked-in build from a different `sbo3l-core` revision will surface
as a CI diff.

## Source

- `crates/sbo3l-core/src/wasm.rs` — the wasm-bindgen surface (50 lines).
- `crates/sbo3l-core/src/passport.rs` — the verifier itself (F-6).
- `crates/sbo3l-core/Cargo.toml` — `[lib] crate-type = ["cdylib", "rlib"]`
  + `[target.'cfg(target_arch = "wasm32")'.dependencies]`
  pinning `getrandom/wasm_js` and `wasm-bindgen`.

## Why this is offline-verifiable

The verifier carries every JSON Schema it needs (the F-11 vendoring lifted
`schemas/*.json` into `crates/sbo3l-core/schemas/` and `include_str!`s
them at compile time). The browser bundle hashes JCS-canonical bytes
itself and verifies Ed25519 signatures locally — no network call, no
trusted daemon, just the capsule and the `.wasm`. v2 capsules are fully
self-contained per F-6; v1 capsules pass structural + request_hash and
the report honestly reports SKIPPED on the four aux-dependent checks.
