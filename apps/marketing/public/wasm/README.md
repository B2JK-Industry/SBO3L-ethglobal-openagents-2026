# WASM verifier artefacts

This directory will hold the WebAssembly artefacts emitted by Dev 1's slice of issue #101:

- `sbo3l_core_bg.wasm` — compiled Rust verifier (`crates/sbo3l-core` + `wasm-bindgen`).
- `sbo3l_core.js` — wasm-bindgen JS glue.

Until Dev 1's PR lands, this directory contains only this README and a `.gitkeep`. The frontend at `/proof` detects the missing module and shows a graceful fallback ("verifier not yet available, link out to GitHub Pages capsule").

## How Dev 1 produces these files

```bash
cd crates/sbo3l-core
wasm-pack build --target web --out-dir ../../apps/marketing/public/wasm --release
```

That command lands two files here. Dev 3 (this PR) does the JS glue + Astro embed; Dev 1 (separate PR) does the cargo build + bindings.

## Why kept under public/

Astro copies `public/` verbatim into `dist/` at build time. The WASM module is then served from the same origin under `/wasm/sbo3l_core_bg.wasm`, which clears CSP `default-src 'self'`. Dynamic `import()` of the JS glue is also same-origin.

## Cache headers

`*.wasm` is intentionally **excluded** from the immutable-cache rule in `vercel.json`. The wasm-pack output is served at stable filenames (`sbo3l_core_bg.wasm`), not fingerprinted URLs, so a long-lived `immutable` cache would let browsers run stale verifier logic after a deploy. Default short cache is correct here until we content-hash the filename.

Follow-up: fingerprint the wasm output (e.g. `sbo3l_core_bg.<contenthash>.wasm`) and emit an import-map JSON the loader reads. Once that lands, the wasm extension can re-join the immutable list. Tracked alongside #101.
