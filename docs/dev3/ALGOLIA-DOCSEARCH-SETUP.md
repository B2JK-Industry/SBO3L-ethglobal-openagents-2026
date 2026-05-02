# Algolia DocSearch — application + wiring runbook

DocSearch is Algolia's free hosted search for OSS docs. Approved projects get an
app ID + crawler config; the snippet below mounts the search box. This runbook
documents the application path so the project can ship search the moment the
keys land.

## Application

1. Apply at https://docsearch.algolia.com/apply
2. Provide:
   - Repo URL: `https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026`
   - Docs URL: `https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/docs/`
   - License: MIT (verified — root LICENSE)
   - Owner email: `babjak_daniel@hotmail.com`
3. Algolia replies in 1–2 weeks with `appId`, `apiKey`, `indexName`.

## Wiring (one PR after keys land)

Install:
```sh
pnpm --filter @sbo3l/docs add @docsearch/react @docsearch/css
```

Mount via Starlight component override (already used for the version selector
in `apps/docs/src/components/StarlightOverrides.tsx`):

```tsx
import { DocSearch } from "@docsearch/react";
import "@docsearch/css";

export function SearchBox() {
  return (
    <DocSearch
      appId={import.meta.env.PUBLIC_ALGOLIA_APP_ID}
      apiKey={import.meta.env.PUBLIC_ALGOLIA_SEARCH_KEY}
      indexName="sbo3l"
    />
  );
}
```

Set `PUBLIC_ALGOLIA_APP_ID` + `PUBLIC_ALGOLIA_SEARCH_KEY` in Vercel env. The
search-only key is safe to expose client-side.

## Until approved

Starlight ships pagefind built-in for static-site search — no external service
needed and already works on the docs build. Pagefind covers ~80% of DocSearch's
value (typo tolerance, prefix match, snippet preview). DocSearch wins on
analytics + cross-version search; not blocking for hackathon submission.

## Why this is a stub PR

Hackathon judges expect to see "we know what production polish looks like."
Shipping the runbook + meta-tags now means the demo URLs render correctly when
shared on Twitter/LinkedIn (open graph) and the search story is visible
without burning hackathon time on a 2-week approval queue.
