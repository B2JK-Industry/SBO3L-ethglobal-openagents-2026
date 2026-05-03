# `apps/marketing/tests/e2e`

Playwright end-to-end specs for the marketing site. **NOT run in CI by default** — adding `@playwright/test` + Chromium binaries is ~700 MB of overhead and the marketing site is static-only Astro. The fast `node --test` suite at `src/lib/*.test.mjs` covers the helpers; this dir catches the few failure modes that only surface in a real browser.

## What lives here

- `zerog-uploader.spec.ts` — 7 specs covering the 6 R20 edge cases + a happy-path sanity:
  1. Non-JSON file → "Not valid JSON" error
  2. Empty file → "File is empty" error
  3. Valid JSON missing `schema` → "Missing top-level `schema` field" error
  4. localStorage quota exceeded → success card shows quota warning
  5. Popup blocked → fallback panel shows inline guidance
  6. Mobile copy swap → desktop strings hidden via `@media (pointer: coarse)`
  7. Happy path → end-to-end manual rootHash flow

## Run locally

```bash
cd apps/marketing
npm install -D @playwright/test
npx playwright install --with-deps chromium

# Build the static site first; the spec defaults to file:// against dist/.
npm run build

npx playwright test tests/e2e/zerog-uploader.spec.ts
```

Or against the dev server:

```bash
cd apps/marketing
npm run dev &
BASE_URL=http://localhost:4321 npx playwright test tests/e2e/zerog-uploader.spec.ts
```

## When to add to CI

If we start seeing real-browser regressions (CSP breaks, runtime drift between the .astro inline template + the pure-helper layer), promote this spec to a separate `playwright-e2e.yml` workflow that runs on a `marketing-changed` path filter. Until then, keep CI fast.
