---
title: "Mobile real-device UAT checklist"
audience: "Daniel — running this on his actual iPhone + Android phone before submission day"
runtime: "~40 minutes (5 min × 8 page groups)"
prereq: "Tested deploy is live at https://sbo3l-marketing.vercel.app/"
---

# Mobile real-device UAT checklist

R20 and R21 ran Lighthouse + Chrome DevTools mobile emulator. Real-device coverage gaps:

- **iOS Safari** has subtle rendering differences from Chrome DevTools (esp. Flexbox / Grid edge cases, `:focus-visible`, `100vh` quirks under bottom-bar)
- **Android Chrome** real-device WebView ≠ desktop Chromium emulation (touch events, file picker behaviour, WASM thread limits)
- **Touch interaction** doesn't show up in Chrome's mobile emulator (44×44 tap targets, double-tap zoom, accidental swipes on horizontal-scroll regions)
- **WASM verifier** under iOS Safari memory constraints (2.4 MB module — should fit, but Safari kills tabs that hit memory ceilings)
- **0G uploader drag-drop** has no real drag-drop on touch; falls back to file picker — needs validation

Daniel runs this end-to-end on the device(s) listed in the table at the bottom. Report findings into `docs/dev3/mobile-uat-findings-$(date +%Y%m%d).md`.

## Hardware target

Pass on at least:

- iPhone with iOS 16+ (any model, Safari + one alt browser if available)
- Android with Chrome 120+ (any model)

If Daniel only has one phone, run the iOS path; Android coverage becomes follow-up.

## Per-page checklist (5 min each)

For every page below: open the URL on phone, then run the steps. Mark ✅ pass / ❌ fail / ⚠️ caveat per row. If any row fails, capture a screenshot before navigating away.

### 1. `/` (homepage)

- [ ] Hero tagline ("Don't give your agent a wallet. Give it a mandate.") fits viewport, no horizontal scroll
- [ ] HeroIllustration SVG renders (animated agent → gate → executor split)
- [ ] CinematicDecision auto-plays after FCP; 6 scenes cycle without freezing
- [ ] Primary CTA button "Start in 5 minutes" is comfortably tappable (≥ 44×44 px), visibly underlined or button-shaped
- [ ] Tier cards (Tier 1 / Tier 2 / Tier 3) tap-navigate to /demo, /playground, /playground/live
- [ ] Sponsor strip (KH, ENS, Uniswap) shows 3 cards in a grid; cards have brand motifs
- [ ] KeyFlowDiagram renders (agent → daemon → executor with audit-log box below)
- [ ] Footer links work; nothing overflows

### 2. `/proof` — WASM verifier

This is the heaviest page. Real-device exercise is critical.

- [ ] PassportCapsule open spread renders (cover left, decision stamp right) — fits within viewport, doesn't horizontal-scroll
- [ ] PassportVerifier section visible alongside (stacked single-column on phones <1024 px)
- [ ] Tap "Verify" button without any input → red "Paste a capsule JSON first" error
- [ ] Paste a known-good v2 capsule (sample at `crates/sbo3l-core/tests/fixtures/passport-v2-known-good.json`) → all 6 ✅ green
- [ ] WASM module loads without crashing the tab (memory headroom check on iOS — 2.4 MB module in addition to page memory)
- [ ] Verify completes in ≤ 5 s on the phone's CPU (it's measured at < 1s on desktop)
- [ ] Tamper one byte in the pasted JSON, re-verify → strict-mode rejects with specific deny code, no fake-OK
- [ ] Try `/proof?capsule=<base64url-encoded-real-capsule>` — verifier textarea pre-fills (the runtime parser landed in #452)

### 3. `/status` — truth table

- [ ] Truth tables don't horizontally-scroll the **page** — only scroll within the table-wrap container (R20 #439 fix)
- [ ] Right-edge gradient hint visible on tables that have content past viewport
- [ ] Tap a `code` element → no double-tap-zoom regression
- [ ] Counter line at top renders (`X surfaces live · Y mock · Z not-yet`)
- [ ] All 4 sections (Sponsor / Storage + audit / Identity + signing / Brand surface) collapsed/expanded readably

### 4. `/roadmap`

- [ ] 3-column grid stacks to 1 column on phone (desktop 1100 px breakpoint)
- [ ] Each item readable; "What's blocking" + "Owner" meta lines visible on production-prereqs items
- [ ] Footer "no-overclaim rule" note + cross-links to /status + docs/dev3 work

### 5. `/kh-fleet`

- [ ] Big counter (247 cumulative executions) renders with tabular-nums
- [ ] Sub-counters wrap nicely (allow/deny, unique agents, workflow id)
- [ ] Recent-20 timeline rows stack to single column on phone (each row is a vertical list with field labels)
- [ ] Long execution IDs (kh-01HZRG_…) wrap (`word-break: break-all` should kick in)
- [ ] "verify capsule format →" link tap-navigates to /proof
- [ ] CTA card bottom — `cargo install sbo3l-cli` code block scrolls horizontally if too long, doesn't overflow page

### 6. `/playground` — mock playground

- [ ] Mock badge banner persistent at top (red-tinted, "MOCK PLAYGROUND" header)
- [ ] APRP + Policy textareas resize on phone; tap to focus brings up keyboard without page-jumping
- [ ] Scenario tabs scroll horizontally if too many
- [ ] "Decide (mock)" button produces output below
- [ ] "Run on real daemon →" link goes to /playground/live

### 7. `/playground/live` — Tier-3

- [ ] Provision banner shows correct state (yellow if API in skeleton mode; green/hidden if provisioned)
- [ ] Form submit → either real response or "skeleton mode" placeholder JSON (never just hangs)
- [ ] "Re-run in Tier 2" cross-link visible

### 8. `/try` — sticky-scroll walkthrough

- [ ] At <900 px breakpoint, sticky left column collapses to single-column flow
- [ ] All 8 sections render readably
- [ ] PassportCapsule closed thumbnail in CTA section visible (~200 px width)
- [ ] CTA buttons tappable (≥ 44×44)

### 9. ZeroGUploader (mounted on `/try`)

- [ ] "Drag a capsule JSON" prompt visible
- [ ] Tapping the drop zone opens the iOS/Android file picker (not drag-drop on touch)
- [ ] Pick a small JSON file — file name + bytes count appear
- [ ] "Push to 0G Storage" button activates after file pick
- [ ] If 0G upload fails → manual fallback section appears with link to storage tool

### 10. `/learn` + per-article pages

- [ ] Card grid → list at narrow widths
- [ ] Each article (e.g. `/learn/trust-dns-manifesto`) renders prose + tables + code blocks
- [ ] Code blocks horizontal-scroll within their container (don't push page horizontal-scroll)
- [ ] In-paragraph links underlined (R21 #462 fix)

### 11. Navigation

- [ ] Tap each nav link (`Proof`, `Status`, `KH-fleet`, `Roadmap`, `Quickstart`) — every one navigates
- [ ] Current-page link visually highlighted (R20 #442 added `aria-current="page"`)
- [ ] **Cmd+K hint hidden under 640 px** — should NOT see the `⌘K` pill on phone
- [ ] GitHub external link opens new tab without breaking back-nav
- [ ] Skip-to-content link on Tab — N/A on phone, but verify keyboard nav from a connected hardware keyboard if available

### 12. /demo step pages (4 sub-pages)

- [ ] `/demo/1-meet-the-agents` — "Open the live viz" hero card tap-navigates to the trust-DNS viz (Vercel preview); fallback PNG renders if preview unavailable
- [ ] `/demo/2-watch-a-decision` — embedded decision feed renders
- [ ] `/demo/3-verify-yourself` — verifier embedded copy works (same as /proof checks)
- [ ] `/demo/4-explore-the-trust-graph` — full force-directed graph renders (canvas backend, may struggle on 100+ nodes on low-end Android)

## Cross-cutting checks (do once, not per page)

- [ ] **Tap targets** — pick 5 random buttons across the site, measure with finger that hit area is comfortable. R20 added `min-height: 44px` globally to `.btn`; verify it landed on real screens.
- [ ] **Page weight** — open Safari Develop → Show Web Inspector → Network. Visit /, /proof, /status. Total transfer < 5 MB on each (most of /proof's weight is the WASM bundle).
- [ ] **Console errors** — should be zero. Any CSP violation surfaces here. R21 #462 added the inline-script hashes to vercel.json; verify no violations remaining.
- [ ] **Mobile-specific gestures** — long-press a `code` element should bring up "Copy" without breaking layout. Pinch-zoom on /status truth table should work without bouncing.
- [ ] **i18n locale switch** — tap LocaleSwitcher (if present), confirm `/sk/` `/ko/` `/de/` paths load (they fall back to EN content via the i18n redirect mapping but should not 404).

## Failure modes to watch for

These are the bugs we expect — if any of them show up, it's not a surprise, but it does need a follow-up issue.

| Symptom | Likely cause | Fix path |
|---|---|---|
| /status table horizontal-scrolls the page | `.table-wrap` overflow rule didn't take effect | Re-check viewport meta + `overflow-x: auto` on the wrapper |
| WASM verifier crashes Safari tab | iOS memory ceiling hit | Lazy-defer + add page-warning ("This page loads ~2.4 MB"); follow-up to compress WASM |
| Cmd+K pill visible on phone | `@media (max-width: 640px) { .search-hint { display: none } }` not applied | Verify Nav.astro CSS specificity |
| 0G uploader drag-drop tries to fire on touch | `dragover` listener fires on touch in some Android Chrome builds | Detect touch + bypass drag handlers entirely |
| Buttons feel unresponsive | tap targets < 44×44 | `min-height: 44px` should cover; one-off offenders fix per-component |
| In-paragraph links not underlined | R21 selector miss | Check `p a, li a, td a, dd a, .lede a` — extend selector if a new context appears |

## Reporting findings

Create `docs/dev3/mobile-uat-findings-YYYYMMDD.md` with:

```md
# Mobile UAT findings — YYYY-MM-DD

Device: <phone model + OS version>
Browser: <Safari N.M / Chrome N.M>

## Pass

- All 12 page groups passed.

## Fail

(list per-page failures with screenshots + repro steps)

## Caveats

(things that worked but felt off — slow load, unusual layout shift, etc.)
```

Open one issue per failure. Tag `mobile-uat`. R21 ships pre-emptive fixes; anything new should be its own follow-up round.

## Hardware coverage matrix (for the report)

| Device class | OS | Browser | Status |
|---|---|---|---|
| iPhone SE 2nd gen | iOS 16 | Safari | TBD |
| iPhone 14/15 | iOS 17 | Safari | TBD |
| Android low-end | 11 | Chrome | TBD |
| Android flagship | 14 | Chrome | TBD |
| iPad | iOS 17 | Safari | TBD |

Daniel: pick whichever combination you actually have. Note the row + status when reporting.
