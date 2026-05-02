# SBO3L post-submission monitoring

> **Audience:** Daniel + QA + Release (Heidi).
> **Outcome:** every live URL judges interact with stays up post-submission, and any break auto-files a tracking issue.
> **Decision:** primary monitoring runs as a free GitHub Actions cron (no external account, lives in the repo); UptimeRobot config provided as a backup for higher-frequency checks.

## What gets monitored

Sourced from [`docs/submission/live-url-inventory.md`](../../docs/submission/live-url-inventory.md). The probe checks **only** the surfaces a judge would actually click on or curl:

| Surface | Expected | Why monitored |
|---|---|---|
| `https://sbo3l.dev/` (or fallback `sbo3l-marketing.vercel.app`) | HTTP 200 + non-empty `<title>` | judge entry point |
| `https://sbo3l.dev/proof` | HTTP 200 (once Astro deploy lands) | the verifier; if this 404s, the load-bearing demo claim is dead |
| `https://sbo3l.dev/submission` | HTTP 200 | submission landing page |
| `https://sbo3l-ccip.vercel.app/` | HTTP 200 | CCIP-Read gateway root |
| `https://sbo3l-ccip.vercel.app/api/0xdeadbeef/0x12345678.json` | HTTP 400 | smoke fail-mode — gateway must reject invalid input |
| `https://crates.io/api/v1/crates/sbo3l-cli` | JSON `max_version` ≥ `1.0.1` | the CLI install path the demo prescribes |
| `https://registry.npmjs.org/@sbo3l/sdk` | JSON `dist-tags.latest` non-empty | TS SDK install |
| `https://pypi.org/pypi/sbo3l-sdk/json` | JSON `info.version` non-empty | Py SDK install |
| `https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026` | HTTP 200 | repo |
| `https://app.ens.domains/sbo3lagent.eth` | HTTP 200 | mainnet apex page |

Each row is a single curl + content assertion.

## Primary: GitHub Actions cron

`.github/workflows/uptime-probe.yml` runs every 30 minutes (cron `*/30 * * * *`) plus on-demand via `workflow_dispatch`. The probe shells out to `scripts/monitoring/check-live-urls.sh`. On failure, the workflow opens an issue tagged `monitoring,uptime` (or comments on an existing open one to avoid spam).

- **Frequency:** 30-min cadence (GitHub Actions cron is best-effort; expect 5-15 min jitter)
- **Cost:** free for public repos
- **Alerting:** auto-opens a GitHub issue tagged `monitoring,uptime` with the failing row(s)
- **Dedup:** if an open issue with that label exists, the workflow comments on it instead of opening a new one

## Secondary: UptimeRobot (optional, no signup required to read this config)

`scripts/monitoring/uptime-robot-config.json` — drop-in importable config for [UptimeRobot](https://uptimerobot.com). 1-minute checks, free tier (50 monitors). Steps:

1. Sign up free at uptimerobot.com
2. Settings → API → generate a Main API Key
3. Use the API to import: `curl -X POST https://api.uptimerobot.com/v3/monitors/batch ...` (see config file for payload)
4. Add a Slack webhook (Settings → Alert Contacts) and link it to all monitors

**When to add UptimeRobot:** if the GitHub Actions cron's 30-min cadence isn't fast enough — e.g. if the marketing site goes down 5 minutes before a judge clicks through. For a hackathon submission window, the GA cron is sufficient.

## Tertiary: BetterStack (heavier, status-page-shaped)

If post-submission SBO3L gains real users, a hosted status page at `status.sbo3l.dev` is the right next step. BetterStack (formerly Better Uptime) provides this. **Defer until Phase 4.**

## Daily smoke (extends regression-on-main)

A separate workflow `.github/workflows/daily-live-smoke.yml` runs at 06:00 UTC and exercises the full demo gate against the live deploys (KH workflow webhook, ENS resolution, CCIP gateway). Failures here are SEV-2 — they indicate a sponsor surface broke, not just our deploy. Tracking issue auto-opened with `live-smoke,sev-2` labels.

(Implementation deferred to Phase 3 — needs Daniel-side review of which `live_from_env()` smokes are safe to run on a schedule against rate-limited sponsor APIs.)

## Files in this directory

- `README.md` — this file
- `check-live-urls.sh` — the probe; runnable locally + in CI
- `uptime-robot-config.json` — backup UptimeRobot import config

## Run the probe locally

```bash
bash scripts/monitoring/check-live-urls.sh
# exit 0 = all green; exit 1 = at least one row failed; exit 2 = config error
```

The probe respects `SBO3L_MONITORING_FAIL_FAST=1` (returns on first failure for fast iteration) and `SBO3L_MONITORING_VERBOSE=1` (prints request + response detail).
