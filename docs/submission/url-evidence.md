# Live URL evidence (HTTP-level)

> **Audience:** Heidi + Daniel + judges who want to verify before clicking through.
> **What this is:** HTTP response evidence — status, security headers, page title, content snippet — captured by Heidi at submission prep time. Not a Lighthouse report; see [Lighthouse runbook](#lighthouse-runbook-for-daniel) at the bottom for that.
>
> **Captured:** 2026-05-01 ~23:25 CEST. Re-run via the script at the bottom.

## ⚠️ Critical finding: marketing Vercel deploy is stale

**`https://sbo3l-marketing.vercel.app`** currently serves the **old static landing page (#86)**, not the Astro version (#100 + #103) that adds `/proof`, `/submission`, `/features`, `/trust-dns-story` routes. Evidence:

```
$ curl -sI https://sbo3l-marketing.vercel.app/proof
HTTP/2 404
x-vercel-error: NOT_FOUND
```

`HEAD /` returns 200 + `<title>SBO3L — Don't give your agent a wallet. Give it a mandate.</title>` (the static page from PR #86, last-modified `Thu, 30 Apr 2026 23:06:42 GMT`). The Astro routes were added in #100 (scaffold) + #103 (content) + #110 (WASM verifier) + #140 (PassportVerifier) + #160 (`/submission`) — none of those are reflected on this preview URL.

**Likely cause:** the Vercel project's `Root Directory` setting + branch deploy config. Possibilities:
1. Vercel Project root is `/` (repo root); root `vercel.json` points `outputDirectory: "apps/marketing"` but that's the static dir, not `apps/marketing/dist` (Astro build output)
2. Vercel project hasn't been re-pointed at the Astro app
3. A different preview URL exists for the Astro deploy that we haven't discovered

**Action item for Daniel before submission:**
- Open the Vercel dashboard → SBO3L marketing project → Settings → General → Root Directory
- If it's `apps/marketing` and the project type is auto-detected as "Other" (static), switch to the Astro framework preset OR set `outputDirectory: "dist"`.
- Trigger a fresh deploy from `main`.
- Verify `/proof`, `/submission`, `/features`, `/trust-dns-story` all return 200 with their respective Astro pages.

Until that's resolved, the Astro routes referenced in `live-url-inventory.md` and `ETHGlobal-form-content.md` should NOT be claimed as live. The submission walkthrough should fall back to the static `/` page + GitHub for the proof artifact paths.

## Per-URL HTTP evidence

### `https://sbo3l-marketing.vercel.app/`

| Field | Value |
|---|---|
| Status | **200** |
| Server | Vercel |
| Cache | `x-vercel-cache: HIT` (age 80252s) |
| HSTS | `max-age=63072000; includeSubDomains; preload` ✅ (2-year preload) |
| Title | `SBO3L — Don't give your agent a wallet. Give it a mandate.` |
| OG type | `website` |
| OG title | `SBO3L — Agent Trust Layer` |
| OG description | `Don't give your agent a wallet. Give it a mandate.` |
| Content-Type | `text/html; charset=utf-8` |
| Last-Modified | 2026-04-30 23:06:42 GMT (stale — see ⚠️ above) |

### `https://sbo3l-marketing.vercel.app/{proof,submission,features,trust-dns-story}`

| Field | Value |
|---|---|
| Status | **404** ❌ (Astro routes not yet on the preview deploy) |
| Server | Vercel |
| `x-vercel-error` | `NOT_FOUND` |

### `https://sbo3l-ccip.vercel.app/`

| Field | Value |
|---|---|
| Status | **200** ✅ |
| Server | Vercel (Next.js) |
| HSTS | `max-age=63072000; includeSubDomains; preload` ✅ |
| Title | `SBO3L CCIP-Read gateway` |
| Description | `ENSIP-25 / EIP-3668 gateway for off-chain SBO3L text records.` |
| Robots | `index, follow` (intentional — gateway URL is meant to be discoverable) |
| Status copy | "Pre-scaffold. Returns `501 Not Implemented` until the T-4-1 main PR ships the record source + signing logic." |

### `https://sbo3l-ccip.vercel.app/api/0xdeadbeef/0x12345678.json` (smoke fail mode)

| Field | Value |
|---|---|
| Status | **400** ✅ (correct — invalid sender + data rejected) |
| Server | Vercel |
| Content-Type | `application/json` |
| `access-control-allow-methods` | `GET, OPTIONS` ✅ (CORS scoped to read-only) |
| `access-control-allow-origin` | `*` (gateway is publicly readable per ENSIP-10 design) |
| `cache-control` | `public, max-age=10, stale-while-revalidate=30` ✅ (short TTL + SWR) |
| `referrer-policy` | `strict-origin-when-cross-origin` ✅ |
| `x-content-type-options` | `nosniff` ✅ |
| `x-frame-options` | `DENY` ✅ |
| `x-matched-path` | `/api/[sender]/[data]` (Next.js dynamic route handler, route exists) |

### `https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026`

| Field | Value |
|---|---|
| Status | **200** |
| Server | github.com |
| HSTS | `max-age=31536000; includeSubdomains; preload` ✅ (1-year preload) |
| `x-frame-options` | `deny` ✅ |
| `x-content-type-options` | `nosniff` ✅ |
| `referrer-policy` | `no-referrer-when-downgrade` |
| CSP | _full GitHub CSP_ — `default-src 'none'; …; frame-ancestors 'none'; …; script-src github.githubassets.com; style-src 'unsafe-inline' github.githubassets.com; upgrade-insecure-requests; …` (production-grade) |

### `https://app.ens.domains/sbo3lagent.eth`

| Field | Value |
|---|---|
| Status | **200** |
| Server | cloudflare |
| HSTS | `max-age=2592000; includeSubDomains` (30-day) |
| `cross-origin-opener-policy` | `same-origin-allow-popups` ✅ |
| `referrer-policy` | `strict-origin-when-cross-origin` ✅ |
| `x-content-type-options` | `nosniff` ✅ |
| CSP | `worker-src 'self' blob:; script-src 'self' plausible.io …; frame-ancestors 'self' https://app.safe.global` ✅ |

### Package registries

Web pages on `crates.io` and `npmjs.com` return 404/403 to `curl` (SPA + Cloudflare bot block). Verified live via machine APIs in [`live-url-inventory.md`](live-url-inventory.md):

- 9 crates @ `1.2.0` via `https://crates.io/api/v1/crates/<name>` JSON
- 6 npm packages @ `1.0.0` / `1.2.0` (mixed) (SDK + integrations) via `https://registry.npmjs.org/<scope>/<name>` JSON
- 5 PyPI packages @ `1.0.0` web HTTP 200 + JSON `https://pypi.org/pypi/<name>/json`

## Security headers summary

| Surface | HSTS preload | XFO/CSP | Status |
|---|---|---|---|
| Marketing `/` | ✅ 2y preload | _no XFO/CSP_ (static page) | 🟡 add CSP via `vercel.json` headers when Astro deploy lands |
| CCIP `/api/*` | ✅ 2y preload | XFO DENY + nosniff + ReferrerPolicy | ✅ |
| GitHub | ✅ 1y preload | XFO deny + full CSP + nosniff | ✅ |
| ENS app | 30d HSTS | nosniff + Referrer + CSP | ✅ |

Once the Astro deploy lands on the marketing project, `apps/marketing/vercel.json` already has the right `headers` block (CSP `default-src 'self'; …`, X-Frame-Options DENY, Permissions-Policy locked down, Referrer-Policy strict-origin) — it just needs the `Root Directory` config to actually apply them.

## Lighthouse runbook for Daniel

Heidi can't run a real Lighthouse audit here (no Chromium / `lighthouse-cli` in the QA environment). Daniel runs locally before the demo video record:

```bash
# Install once
npm install -g @lhci/cli@0.13.x lighthouse@12

# Per-URL (run after Astro deploy is live)
for u in \
  https://sbo3l-marketing.vercel.app \
  https://sbo3l-marketing.vercel.app/proof \
  https://sbo3l-marketing.vercel.app/submission \
  https://sbo3l-marketing.vercel.app/features \
  https://sbo3l-marketing.vercel.app/trust-dns-story \
  https://sbo3l-ccip.vercel.app ; do
  out=$(echo "$u" | sed 's|https://||;s|/|_|g').html
  lighthouse "$u" \
    --preset=desktop \
    --output=html \
    --output-path="docs/submission/lighthouse/desktop-$out" \
    --chrome-flags="--headless --no-sandbox" \
    --quiet
  lighthouse "$u" \
    --preset=mobile \
    --output=html \
    --output-path="docs/submission/lighthouse/mobile-$out" \
    --chrome-flags="--headless --no-sandbox" \
    --quiet
done
```

Targets per `02-standards.md` Frontend section:
- Lighthouse perf score > 90
- WCAG AA (contrast, aria, keyboard nav)
- JS bundle < 200 KB gzipped per page
- No external font / CSS / script CDNs

Commit the resulting `docs/submission/lighthouse/*.html` files (or a markdown summary) before the submission cut-off.

## Re-run the HTTP-level smoke (paste-ready)

```bash
URLS=(
  "https://sbo3l-marketing.vercel.app"
  "https://sbo3l-marketing.vercel.app/proof"
  "https://sbo3l-marketing.vercel.app/submission"
  "https://sbo3l-marketing.vercel.app/features"
  "https://sbo3l-marketing.vercel.app/trust-dns-story"
  "https://sbo3l-ccip.vercel.app/"
  "https://sbo3l-ccip.vercel.app/api/0xdeadbeef/0x12345678.json"
  "https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026"
  "https://app.ens.domains/sbo3lagent.eth"
)
for u in "${URLS[@]}"; do
  CODE=$(curl -sk -o /dev/null -w "%{http_code}" -m 10 -L "$u")
  HSTS=$(curl -sk -I -m 10 -L "$u" 2>/dev/null | awk -v IGNORECASE=1 '/^strict-transport-security:/ {sub(/^[^:]*: */, ""); sub(/\r$/, ""); print; exit}')
  printf "%-65s %s | HSTS: %s\n" "$u" "$CODE" "${HSTS:-(none)}"
done
```

Expected result at the time of writing: 5× `200`, 4× `404` for the marketing-routes-not-on-deploy issue, plus the CCIP `400` for the invalid-sender smoke. Once the Astro deploy lands, all four 404s flip to 200.
