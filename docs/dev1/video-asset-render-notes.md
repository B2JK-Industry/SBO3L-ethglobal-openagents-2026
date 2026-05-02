# Video asset render verification notes

> **Date:** 2026-05-02
> **Scope:** Wave 2 demo-asset extraction (PR after #419)
> **Author:** Dev 1 (Daniel + Claude pair)
> **Assets under test:** `apps/marketing/public/demo-assets/*.svg`

## Files verified

| File | Dimensions | Animated | Notes |
|---|---|---|---|
| `title-card.svg` | 1920×1080 | yes (particle drift, 5s loop) | honors `prefers-reduced-motion` via inline `@media` |
| `lower-third-template.svg` | 600×100 | no | text strings hand-editable |
| `sponsor-insert-keeperhub.svg` | 200×200 | no | shield motif |
| `sponsor-insert-ens.svg` | 200×200 | no | namespace-tree motif |
| `sponsor-insert-uniswap.svg` | 200×200 | no | swap-arrows motif |
| `sponsor-insert-anthropic.svg` | 200×200 | no | tool-use chat motif |
| `end-card.svg` | 1920×1080 | no | placeholder QRs (composite real ones over it) |
| `qr-github.svg` | 39×39 modules | no | real QR, EC level M, margin 1 |
| `qr-npm.svg` | ~33 modules | no | real QR, EC level M, margin 1 |
| `qr-cratesio.svg` | ~33 modules | no | real QR, EC level M, margin 1 |

## Validation steps run

1. **`xmllint --noout`** on every SVG — all 10 parse as well-formed XML, no warnings.
2. **macOS Quick Look (`qlmanage -t`)** — uses WebKit, so this is a Safari render check. Sampled `title-card.svg`, `end-card.svg`, `sponsor-insert-keeperhub.svg`, `lower-third-template.svg`, `qr-github.svg`. All produced visually correct PNG thumbnails at 1080px.
3. **QR round-trip** — re-ran the generator with the same target URL and byte-compared against the on-disk SVG (`expected === onDisk` → `true`). Confirms the SVG encodes the intended URL bit-for-bit.
4. **Astro static build** — `apps/marketing/public/` is served as-is; no Astro processing applied to these files. Anything that opens directly in a browser opens identically when served by Vercel.

## Browser matrix

| Engine | How tested | Result |
|---|---|---|
| WebKit (Safari) | macOS Quick Look thumbnail (uses WebKit's SVG renderer) | ✅ all 5 sampled assets render correctly |
| Chromium (Chrome) | Not directly verified in this PR; Astro dev server can be used post-merge to confirm. SVG features used (basic shapes, `<text>`, inline `<style>` `@keyframes`, `@media (prefers-reduced-motion)`) are all in the SVG 1.1 + CSS-Animations Level 1 baseline that Chromium has supported since 2014. | ✅ expected pass |
| Firefox (Gecko) | Not directly verified. Same reasoning as Chrome. | ✅ expected pass |

## Known limitations / followups

- **End-card QR placeholders are stylized markers, not real QRs.** This is by design — `end-card.svg` ships as a self-contained recording-ready asset, and `npm run build:qr` produces standalone real QRs that Daniel composites in the editor. Rationale: keeping the static asset minimal lets Daniel re-use it across cuts without re-running the script.
- **No webfont fetch.** All text uses `ui-monospace, monospace`. On macOS Daniel's recording machine this resolves to SF Mono; on the recording rig's preview path nothing has to load over network. Trade-off: the exact glyph shape varies by OS — acceptable for a 3-min demo, not acceptable for a brand-style guide.
- **Particle animation is decorative.** If Daniel records with `prefers-reduced-motion: reduce` on (likely false on the recording machine but worth flagging), the title card will be a static layout — still on-brand, just no drift.
- **Sponsor inserts use a generic "SPONSOR TRACK" tag line.** Hand-edit the `<text>` inside each file if the script calls for "TRACK PARTNER" or "SPONSORED BY" instead.

## How to regenerate

```bash
cd apps/marketing
npm install                 # picks up qrcode devDep if not already
npm run build:qr            # writes 3 qr-*.svg files
```

If a target URL changes (repo rename, npm org rename), edit `scripts/build-qr.mjs` `targets` array and re-run.

## Out of scope for this PR

- Real-time end-card with embedded QRs as a single SVG (would couple end-card.svg to script execution). Daniel composites in the editor instead.
- MP4/MOV pre-renders. The recording rig handles rasterization.
- Subtitle/caption tracks. Separate asset family.
