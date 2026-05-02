# Lighthouse + a11y CI

Two scripts wired to a single GitHub Actions workflow keep the marketing + docs surfaces above 90 on Lighthouse and free of WCAG 2.1 AA violations.

## Run locally

```bash
# Lighthouse (requires Chrome / Chromium installed)
npm install --no-save lighthouse@^12 chrome-launcher@^1
node scripts/lighthouse-ci.mjs
# → docs/submission/lighthouse-reports/<id>.<preset>.json
# Exits 1 if any score < 0.9

# axe-core a11y audit
npm install --no-save playwright@^1.48 @axe-core/playwright@^4.10
npx playwright install --with-deps chromium
node scripts/a11y-audit.mjs
# → docs/submission/a11y-reports/<host>.<id>.json
# Exits 1 if any URL has WCAG 2.1 AA violations
```

## CI integration

`.github/workflows/lighthouse.yml` runs both jobs on every PR touching `apps/marketing/**`, `apps/docs/**`, `apps/hosted-app/**`, the scripts themselves, or the workflow file. Reports upload as artifacts on each run. Both jobs use `continue-on-error: true` so artifact upload happens even when scores fail — fix the score, re-run.

## Targets

The script targets all reachable surfaces — see the `TARGETS` array in each script. Add a new URL there + commit; CI picks it up on next run.

## Score floor

`MIN_SCORE = 0.9` in `lighthouse-ci.mjs`. Adjust if the target shifts. Categories: performance, accessibility, best-practices, seo.

## A11y baseline (code-visible fixes already applied)

- BaseLayout: `<a href="#main" class="skip-link">Skip to main content</a>` first; `<main id="main" tabindex="-1">` lands keyboard focus correctly.
- Globally-injected `:focus-visible` outline using the brand accent.
- `lang` prop on BaseLayout (defaults `en`); Slovak pages pass `lang="sk"`.
- `<header role="banner">` + `<footer role="contentinfo">` + `<nav aria-label="Primary">` landmarks.
- External links carry `rel="noopener"` for tabnabbing protection.
- Brand link gets `aria-label="SBO3L home"` so screen readers don't read the bare wordmark in isolation.

Further fixes when CI flags violations — they go in here as documented diffs.
