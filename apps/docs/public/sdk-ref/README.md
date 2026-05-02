# Auto-generated SDK reference output

This directory holds typedoc + sphinx HTML output from the published `@sbo3l/*` and `sbo3l-*` packages. Files here are **machine-generated** — do not hand-edit; changes are overwritten by the next CI run.

## Layout

```
apps/docs/public/sdk-ref/
├── typescript/
│   ├── sdk/          (typedoc HTML for @sbo3l/sdk)
│   ├── langchain/    (typedoc HTML for @sbo3l/langchain)
│   ├── autogen/      (typedoc HTML for @sbo3l/autogen)
│   └── vercel-ai/    (typedoc HTML for @sbo3l/vercel-ai)
└── python/
    ├── sbo3l-sdk/         (sphinx HTML)
    ├── sbo3l-langchain/   (sphinx HTML)
    ├── sbo3l-crewai/      (sphinx HTML)
    └── sbo3l-llamaindex/  (sphinx HTML)
```

Astro copies `public/` verbatim into `dist/` at build time. The reference pages are reachable at `https://sbo3l-docs.vercel.app/sdk-ref/<lang>/<package>/`.

## Regeneration

See [`apps/docs/scripts/README.md`](../../scripts/README.md). CI runs daily and on `repository_dispatch: sdk-published` events.

## First-run state

Empty until the first CI run completes. The wrapper Starlight pages at `/reference/sdk-typescript` and `/reference/sdk-python` link here; before first regeneration those links resolve to 404 — that's expected and the wrapper page documents it.
