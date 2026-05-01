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

`vercel.json` includes `*.wasm` in the immutable-cache list. WASM is content-hashed by Astro, so cache invalidation is automatic.
