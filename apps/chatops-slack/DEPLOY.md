# `chatops-slack` — deployment

## What this is
A Slack bot (Express HTTP handler) that exposes 3 slash commands:
- `/sbo3l verify <capsule JSON>`
- `/sbo3l audit <agent_id>`
- `/sbo3l decide <APRP JSON>`

## 1. Create the Slack app (Daniel, one-shot)

1. Visit https://api.slack.com/apps and click **Create New App** → **From scratch**.
2. Name: `SBO3L`, pick the workspace.
3. **OAuth & Permissions** → add bot scope `commands`. Click **Install to Workspace**.
4. **Slash Commands** → **Create New Command**:
   - Command: `/sbo3l`
   - Request URL: `https://<your-vercel-domain>/slack/commands`
   - Short description: `SBO3L capsule verify / audit / decide`
   - Usage hint: `verify <capsule> | audit <agent> | decide <APRP>`
5. **Basic Information** → copy **Signing Secret** (used for HMAC verification).

## 2. Deploy to Vercel

```bash
cd apps/chatops-slack
npm install
npm run build
vercel link            # one-shot
vercel env add SLACK_SIGNING_SECRET production    # paste secret from Step 1.5
vercel env add SBO3L_DAEMON_URL production         # e.g. https://sbo3l-prod.example.com
vercel deploy --prod
```

Update the slash command's Request URL with the Vercel production URL.

## 3. Reinstall the app

After updating the URL, reinstall the app in the workspace (Slack caches the old URL).

## Local dev

```bash
npm install
SLACK_SIGNING_SECRET=test SBO3L_DAEMON_URL=http://localhost:8730 npm run dev
# → listens on :3000
```

For local Slack tests, expose via ngrok:
```bash
ngrok http 3000
# update the Slack command Request URL to the ngrok https://<id>.ngrok.io/slack/commands
```

## Tests

```bash
npm test         # 18 vitest passing (handler logic, no Slack network)
npm run typecheck
```

## Out of scope (future PRs)

- **Discord** + **Teams** counterparts — same 3-command surface; templates from this bot
- **Block Kit rich rendering** — current bot uses plain mrkdwn for portability
- **Workspace-level OAuth + multi-tenant** — single-workspace today; multi-tenant needs a token store
