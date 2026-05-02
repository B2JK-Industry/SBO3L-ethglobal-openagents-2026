# Submission rehearsal runbook (for Daniel — recording day)

> **Audience:** Daniel, ~30 min before recording the demo video.
> **Outcome:** the screencast shows judges *exactly* the verify-everything path with deterministic timing, no surprises, no broken links.
> **Why Heidi can't record this herself:** the QA environment has no Chromium / no screen recorder. The mechanical substitute is `scripts/submission/rehearsal-audit.sh`, which exits 0 when the package is record-ready. Run that BEFORE recording.

## Pre-record checklist (10 min)

```bash
# 1. Confirm submission package is record-ready
bash scripts/submission/rehearsal-audit.sh
# expect: 36 PASS / 15 WARN (SPA-bot-blocked, expected) / 0 FAIL

# 2. Confirm chaos suite latest run is current
cat scripts/chaos/artifacts/summary.txt
# expect: 3/5 PASS minimum (02 + 03 + 04). 01 + 05 known-failing per
# the documented findings — flag verbally in the video if relevant.

# 3. Confirm v1.0.1 install path is clean on a fresh shell
mktemp -d /tmp/sbo3l-rehearsal-XXX | xargs -I{} bash -c 'cd "{}" && cargo install sbo3l-cli --version 1.0.1 --root . && bin/sbo3l --version'
# expect: sbo3l 1.0.1

# 4. Confirm key live URLs (see also: scripts/monitoring/check-live-urls.sh)
bash scripts/judges/verify-everything.sh
# expect: all PASS, total elapsed < 10 min

# 5. Pre-warm the demos' deterministic fixtures
ls test-corpus/passport/v2-capsule.json   # exists, byte-identical to live
ls demo-fixtures/ens-records.json          # offline ENS fixture
```

If anything in steps 1-4 fails, **don't record yet** — fix forward. The point of the audit is that it's noisy when something's drifted from the documented state.

## Recording setup (5 min)

Per [`docs/submission/demo-video-script.md`](demo-video-script.md):

- Resolution: 1080p minimum (1440p preferred for terminal legibility)
- Terminal font: ≥ 18pt (e.g. Iosevka, Berkeley Mono, JetBrains Mono)
- Theme: dark background — judges' projection rooms are dark
- Browser tab order: pre-load these in this order, in a single window:
  1. `https://sbo3l-marketing.vercel.app` (or `https://sbo3l.dev` if DNS pointed)
  2. `https://sbo3l-marketing.vercel.app/proof` (after Astro deploy lands)
  3. `https://app.sbo3l.dev/trust-dns` (or fallback Vercel preview)
  4. `https://app.ens.domains/sbo3lagent.eth`
  5. `https://etherscan.io/address/0x231b0Ee14048e9dCcD1d247744d114a4EB5E8E63` (PublicResolver, mainnet)
  6. https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/v1.0.1
- Terminal panes: 2 splits — left pane runs the live demo, right pane shows the audit chain rendered via `tail -f` on `~/.sbo3l/audit.log` (or the operator console)
- Capsule for tamper demo: pre-generated at `/tmp/capsule-tamper-demo.json` via `bash demo-scripts/run-openagents-final.sh && cp demo-scripts/artifacts/passport-allow.json /tmp/capsule-tamper-demo.json`. Don't tamper before recording — show the green-check pass first, then byte-flip live on camera.

## Storyboard execution

Time-boxed pacing per `demo-video-script.md`:
- 0:00–0:15 — cold open + tagline (15s, deliberate pacing — let it land)
- 0:15–0:45 — live KH workflow (30s — most depends on KH webhook latency; pre-warm by running once)
- 0:45–1:15 — ENS mainnet + Sepolia fleet (30s)
- 1:15–1:45 — trust-DNS viz (30s — the visual hero)
- 1:45–2:15 — `/proof` verifier with tamper (30s — load-bearing scene; rehearse 2-3 times)
- 2:15–2:45 — multi-framework crossover (30s — the most ambitious; if pre-recorded transcript runs cleanly use it, else cut)
- 2:45–3:00 — outro (15s — "9 crates, 8 frameworks, 60 agents, 1 mandate")

## Re-record triggers (any one of these = redo)

- A `/proof` verifier check fails on the green-pass demo (something drifted in the bundled WASM)
- The KH workflow webhook returns anything other than 200 with a `kh-…` executionId
- The Sepolia fleet visualization shows < 5 agents (T-3-3 manifest didn't apply)
- Terminal font is < 18pt on playback
- Any tab shows a 404 in the URL bar
- Total runtime is > 3:00

## Post-record checklist

```bash
# 1. Lighthouse on the recorded URLs
#    (assumes lighthouse + Chrome are installed locally)
for u in \
  https://sbo3l-marketing.vercel.app \
  https://sbo3l-marketing.vercel.app/proof \
  https://app.sbo3l.dev/trust-dns ; do
  out=$(echo "$u" | sed 's|https://||;s|/|_|g')
  lighthouse "$u" --preset=desktop --output=json --output=html \
    --output-path="docs/submission/lighthouse/desktop-$out" \
    --chrome-flags="--headless --no-sandbox" --quiet
done

# 2. Validate the recording
ls -la docs/submission/rehearsal-2026-MM-DD.{mp4,gif,mov}

# 3. Commit + push
git add docs/submission/rehearsal-* docs/submission/lighthouse/
git commit -m "docs(submission): demo video rehearsal recording + Lighthouse reports"
git push
```

## Fallbacks

- If `sbo3l.dev` DNS isn't pointed, narrate the fallback: "sbo3l-marketing.vercel.app is the current preview; the custom domain points after CTI-3-1 lands. Same content."
- If the trust-DNS viz at `app.sbo3l.dev/trust-dns` is 404, fall back to a screenshot you captured during a successful local run; narrate as "viz preview pre-deploy."
- If KH workflow webhook returns 500/timeout (their side), fall back to the **mock** demo path with `KeeperHubExecutor::local_mock()` — narrate honestly: "The live arm is exercised by `bash demo-scripts/sponsors/keeperhub-real-execution.sh` which captured `kh-172o77rxov7mhwvpssc3x` at submission day; the mock here is byte-shape-identical."

## What makes this an honest demo (Frank rule)

Every claim in the video has a code reference, a live URL, or a runnable command — listed in [`docs/submission/ETHGlobal-form-content.md`](ETHGlobal-form-content.md). If the video says it, the doc cites it. If the doc cites it, you can verify it from the public repo. **No marketing fluff, no silent claims.**
