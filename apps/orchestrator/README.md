# `@sbo3l/orchestrator` — Linear webhook → next-ticket auto-prompt

**Audience:** Daniel + the QA + Release agent operating the 4+1 nonstop loop.
**Outcome:** when an agent's ticket transitions to Done in Linear, this service auto-renders the next-ticket prompt and delivers it to that agent's Discord channel — no human in the loop.

This is the **Friction 3 fix** from
[`docs/win-backlog/11-nonstop-operation-guide.md`](../../docs/win-backlog/11-nonstop-operation-guide.md):
without it, agents idle between tickets waiting for Daniel to send the next prompt.

## How it works

```
Linear "Done" event ─► /api/linear-webhook (Vercel Function)
                       │
                       │ 1. verify HMAC-SHA256 signature
                       │ 2. parse webhook envelope
                       │ 3. resolve assignee.name → 4+1 slot ("Dev 1" etc.)
                       │ 4. query Linear for next unblocked ticket for that slot
                       │ 5. render prompt from docs/win-backlog/09-prompt-template.md
                       │ 6. POST prompt to slot's Discord webhook
                       │ 7. mark next ticket as In Progress in Linear
                       ▼
                Agent runtime (watching its Discord channel) picks up the prompt
                and starts the next ticket on branch agent/<slot>/<ticket-id>.
```

## Reaction matrix

| Webhook event | Action |
|---|---|
| Non-Issue (Comment, Project, ...) | ignored |
| Issue, action ≠ "update" | ignored |
| Issue, state.type ≠ "completed" | ignored |
| Issue, completed, no recognised slot assignee | ignored (Daniel handles manually) |
| Issue, completed, slot queue empty | post to coordination channel, no dispatch |
| Issue, completed, next ticket found | render prompt + deliver + mark in progress |

## File layout

```
apps/orchestrator/
├── api/
│   ├── linear-webhook.ts   # Vercel HTTP entry — verifies signature, calls handler
│   └── health.ts           # GET /api/health liveness probe
├── src/
│   ├── linear-webhook.ts   # core handler (pure, easy to unit-test)
│   ├── render-prompt.ts    # 09-prompt-template.md filler + phase inference
│   ├── agent-bridge.ts     # Discord webhook transport + status posts
│   ├── linear-client.ts    # GraphQL queries against api.linear.app
│   ├── signature.ts        # HMAC-SHA256 webhook verification
│   ├── slot-mapping.ts     # Linear assignee.name → 4+1 slot
│   ├── types.ts            # LinearWebhookEvent, Slot, etc.
│   └── local-server.ts     # `npm run dev` shim (not deployed)
├── test/                    # vitest suite
└── vercel.json              # function runtime + maxDuration config
```

## Setup (one-time)

1. **Create Linear webhook**
   - Linear → Settings → API → Webhooks → New
   - URL: `https://<vercel-deployment>/api/linear-webhook`
   - Resource types: **Issue**
   - Copy the signing secret → `LINEAR_WEBHOOK_SECRET`
2. **Get Linear API key**
   - Linear → Settings → Account → Security → Personal API key → New
   - Scope: `Read`, `Write` (write needed to mark next ticket In Progress)
   - → `LINEAR_API_KEY`
3. **Get Linear "In Progress" state UUID**
   - Run a one-off `workflowStates` GraphQL query (see Linear docs) and copy
     the UUID for the workspace's "In Progress" state → `LINEAR_STATE_IN_PROGRESS`
4. **Discord webhook per slot**
   - Discord → server settings → Integrations → Webhooks → New (one per channel)
   - One channel per slot (`#sbo3l-dev1-prompts`, `#sbo3l-dev2-prompts`, ...)
   - Plus one for `#sbo3l-coordination`
   - URLs go in `DISCORD_WEBHOOK_DEV{1..4}_URL`, `DISCORD_WEBHOOK_QA_URL`, `DISCORD_WEBHOOK_COORDINATION_URL`
5. **Linear assignee names**
   - In Linear, name the agent users **exactly** `Dev 1`, `Dev 2`, `Dev 3`, `Dev 4`, `QA + Release`
   - These strings are matched literally; misspellings cause the orchestrator to ignore the event.
6. **Linear "unblocked" label**
   - Create a single workspace label called `unblocked`. Tickets only get it once their `Depends:` chain is merged.

See `.env.example` for the full env var list.

## Deploy on Vercel

```bash
# From repo root
cd apps/orchestrator
vercel link        # one-time, point at sbo3l-orchestrator project
vercel --prod
```

The root [`vercel.json`](../../vercel.json) currently points at `apps/marketing`, so deploy this orchestrator as a **separate Vercel project** rooted at `apps/orchestrator/`. Set `Root Directory: apps/orchestrator` in the Vercel dashboard.

Set the env vars (Vercel dashboard → Project → Settings → Environment Variables) per `.env.example`. Production deploys read them from there; local dev reads `.env`.

## Local development

```bash
cd apps/orchestrator
npm install
cp .env.example .env   # then fill values

npm run dev            # http://localhost:3000
npm test               # vitest
npm run typecheck      # tsc --noEmit
```

Hit it locally with a signed payload using `curl`:

```bash
SECRET="<value of LINEAR_WEBHOOK_SECRET>"
BODY='{"action":"update","type":"Issue","data":{...}}'
SIG=$(printf '%s' "$BODY" | openssl dgst -sha256 -hmac "$SECRET" -hex | awk '{print $2}')
curl -sS http://localhost:3000/api/linear-webhook \
  -H "Content-Type: application/json" \
  -H "linear-signature: $SIG" \
  -d "$BODY"
```

## Dry-run mode

Set `ORCHESTRATOR_DRY_RUN=1` to exercise the full pipeline without posting to Discord or mutating Linear state. The handler returns `{ kind: "dispatched", ... }` as if it had succeeded, but the prompt only goes to logs. Useful for replaying historical webhook payloads.

> **Caveat:** dry-run currently swaps only the Discord transport for an in-memory shim — Linear `markInProgress` calls still go through. To suppress those too, run against a Linear scratch workspace.

## QA test plan (Heidi runs literally before merge)

```bash
cd apps/orchestrator
npm install
npm run typecheck
npm test
```

Expect: typecheck clean (no `any`, no diagnostics), all vitest suites green.

Optional smoke (hits Discord — only run with throwaway webhook URLs):

```bash
ORCHESTRATOR_DRY_RUN=1 \
LINEAR_API_KEY=fake \
LINEAR_WEBHOOK_SECRET=test-secret \
LINEAR_STATE_IN_PROGRESS=fake-state \
DISCORD_WEBHOOK_DEV1_URL=https://example.invalid \
npm run dev &

sleep 1
SECRET=test-secret
BODY='{"action":"update","type":"Issue","data":{"id":"t","identifier":"F-1","title":"x","priority":2,"state":{"id":"s","name":"Done","type":"completed"},"assignee":{"id":"u","name":"Dev 1"}}}'
SIG=$(printf '%s' "$BODY" | openssl dgst -sha256 -hmac "$SECRET" -hex | awk '{print $2}')
curl -sf http://localhost:3000/api/linear-webhook \
  -H "Content-Type: application/json" \
  -H "linear-signature: $SIG" \
  -d "$BODY"

# Expect: 200 OK with body like {"kind":"queue_empty","slot":"Dev 1"} or {"kind":"dispatched",...}
pkill -f tsx
```

## Out of scope (deliberate)

- **No tmux/SSH paste-into-Claude-Code yet.** MVP transport is Discord webhook posting; agent runtimes either watch the channel or Daniel re-pastes manually. Real auto-injection is a follow-up.
- **No Linear "Depends:" graph evaluation.** We trust the workspace `unblocked` label as the gate. Daniel adds/removes the label when dependencies change.
- **No retry queue.** Linear retries failed webhook deliveries automatically; idempotency comes from the next-ticket selection being a query, not a state machine on our side.
- **No multi-instance coordination.** Single Vercel deployment; no leader election needed.

## Maintainers

QA + Release owns this. Bug reports → `#sbo3l-incidents` if blocking, otherwise GitHub issue.
