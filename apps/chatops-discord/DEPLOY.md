# `chatops-discord` — deployment

## What this is
A Discord bot exposing 3 slash commands via the Interactions API (HTTP, not gateway-websocket — works on Vercel without long-lived connections):
- `/sbo3l verify capsule:<json>`
- `/sbo3l audit agent:<id>`
- `/sbo3l decide aprp:<json>`

## 1. Create the Discord application (Daniel, one-shot)

1. Visit https://discord.com/developers/applications and click **New Application**.
2. Name: `SBO3L`.
3. **General Information** → copy **Public Key** (used for Ed25519 verification).
4. **Bot** → reset token (you don't need it for HTTP-only interactions but Discord requires the bot user to exist).
5. **Interactions Endpoint URL** → set to `https://<your-vercel-domain>/discord/interactions`. Discord pings this URL — won't save until your bot responds with PONG.

## 2. Register the slash command

```bash
APP_ID=<your-app-id>
BOT_TOKEN=<your-bot-token>
curl -X POST -H "Authorization: Bot $BOT_TOKEN" -H "Content-Type: application/json" \
  https://discord.com/api/v10/applications/$APP_ID/commands \
  -d @- <<'EOF'
{
  "name": "sbo3l",
  "description": "SBO3L capsule verify / audit / decide",
  "options": [
    {
      "type": 1, "name": "verify", "description": "Verify a capsule",
      "options": [{ "type": 3, "name": "capsule", "description": "Capsule JSON", "required": true }]
    },
    {
      "type": 1, "name": "audit", "description": "Audit chain prefix",
      "options": [{ "type": 3, "name": "agent", "description": "Agent ID", "required": true }]
    },
    {
      "type": 1, "name": "decide", "description": "Submit an APRP",
      "options": [{ "type": 3, "name": "aprp", "description": "APRP JSON", "required": true }]
    }
  ]
}
EOF
```

## 3. Deploy to Vercel

```bash
cd apps/chatops-discord
npm install
npm run build
vercel link
vercel env add DISCORD_PUBLIC_KEY production    # paste from Step 1.3
vercel env add SBO3L_DAEMON_URL production
vercel deploy --prod
```

Update the Interactions Endpoint URL in Discord to your Vercel production URL.

## 4. Install on a server

OAuth2 → URL Generator → scope `applications.commands` → copy URL → paste in browser → pick a server.

## Local dev

```bash
npm install
DISCORD_PUBLIC_KEY=<empty for unsigned> SBO3L_DAEMON_URL=http://localhost:8730 npm run dev
# → listens on :3000
```

For local interaction tests, Discord requires HTTPS — use ngrok:
```bash
ngrok http 3000
# update Interactions Endpoint URL to https://<id>.ngrok.io/discord/interactions
```

## Tests

```bash
npm test         # 18 vitest passing (handler logic, no Discord network)
npm run typecheck
```

## Same surface as Slack bot

The handler dispatcher accepts both Discord's `subcommand`/`option` shape AND the Slack-compat `text` shape so the same dispatcher could power both bots if they ever consolidate. Today they ship as separate apps with platform-native server.ts wrappers.
