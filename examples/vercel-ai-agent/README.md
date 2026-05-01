# `examples/vercel-ai-agent`

Minimal Next.js + Vercel AI SDK + SBO3L example. Demonstrates `streamText` + `sbo3lTool` in a Route Handler.

> ⚠ **DRAFT (T-1-7):** depends on `@sbo3l/sdk` + `@sbo3l/vercel-ai` being published to npm.
> While unpublished, the example consumes both via `file:` paths (`../../sdks/typescript` and `../../sdks/typescript/integrations/vercel-ai`).

## Run (dev)

```bash
# 1. Start the SBO3L daemon
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &

# 2. Wire OpenAI
export OPENAI_API_KEY=sk-...

# 3. Boot Next
cd examples/vercel-ai-agent
npm install
npm run dev
# → http://localhost:3000
```

Try in the chat box:

> "Pay 0.05 USDC for an inference call to api.example.com."

The agent calls the `pay` tool with an APRP body; SBO3L's policy boundary decides; the agent reports the receipt or the deny code.

## Smoke (no OpenAI key)

```bash
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
npm install
npm run smoke   # runs scripts/smoke.ts directly via tsx
```

Output on success:

```
✓ allow — execution_ref: kh-01HTAWX5K3R8YV9NQB7C6P2DGS
  audit_event_id: evt-...
  signature.algorithm: ed25519
```

## What's where

- `app/api/chat/route.ts` — Vercel AI Route Handler with `streamText` + `sbo3lTool`.
- `app/page.tsx` — minimal client UI using `useChat`.
- `scripts/smoke.ts` — direct tool-execute smoke (no OpenAI needed).
- `next.config.mjs` — `transpilePackages` for the workspace-internal SBO3L packages.

## License

MIT
