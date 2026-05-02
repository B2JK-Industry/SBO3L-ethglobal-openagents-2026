# `chatops-teams` — deployment

## What this is
A Microsoft Teams bot exposing 3 slash commands via the Bot Framework Activity v3 webhook:
- `@SBO3L verify <capsule JSON>`
- `@SBO3L audit <agent_id>`
- `@SBO3L decide <APRP JSON>`

(Teams uses `@bot mention` rather than `/`-style — the handler strips the leading mention and dispatches the same as Slack/Discord.)

## 1. Register the Azure Bot resource (Daniel, one-shot)

1. Visit https://portal.azure.com → search **Azure Bot** → **Create**.
2. Bot handle: `sbo3l`. App type: **Multi Tenant**.
3. **Configuration** → set **Messaging endpoint** to `https://<your-vercel-domain>/api/messages`.
4. **Configuration** → copy **Microsoft App ID** + **Microsoft App Password** (the latter from "Manage Password" if needed — Azure rotates these).
5. **Channels** → add **Microsoft Teams**.

## 2. Create the Teams app manifest

Minimal `manifest.json` for sideloading:

```json
{
  "$schema": "https://developer.microsoft.com/en-us/json-schemas/teams/v1.16/MicrosoftTeams.schema.json",
  "manifestVersion": "1.16",
  "version": "1.2.0",
  "id": "<your Bot's App ID>",
  "packageName": "dev.sbo3l.chatops",
  "developer": {
    "name": "SBO3L",
    "websiteUrl": "https://sbo3l.dev",
    "privacyUrl": "https://sbo3l.dev/privacy",
    "termsOfUseUrl": "https://sbo3l.dev/terms"
  },
  "name": { "short": "SBO3L", "full": "SBO3L ChatOps" },
  "description": {
    "short": "Verify capsules, audit agents, decide APRPs.",
    "full": "Bring SBO3L's policy boundary into Teams: /sbo3l verify, /sbo3l audit, /sbo3l decide."
  },
  "icons": { "outline": "outline.png", "color": "color.png" },
  "accentColor": "#5eb3ff",
  "bots": [
    {
      "botId": "<your Bot's App ID>",
      "scopes": ["personal", "team", "groupchat"],
      "supportsFiles": false,
      "isNotificationOnly": false
    }
  ],
  "permissions": ["identity", "messageTeamMembers"],
  "validDomains": ["<your-vercel-domain>"]
}
```

Zip `manifest.json` + `outline.png` + `color.png` → upload via **Teams Admin Center** → **Manage apps** → **Upload**.

## 3. Deploy to Vercel

```bash
cd apps/chatops-teams
npm install
npm run build
vercel link
vercel env add MICROSOFT_APP_ID production         # paste from Step 1.4
vercel env add MICROSOFT_APP_PASSWORD production   # paste from Step 1.4
vercel env add SBO3L_DAEMON_URL production
vercel deploy --prod
```

Update Azure Bot **Messaging endpoint** to the Vercel production URL.

## Local dev

```bash
npm install
SBO3L_DAEMON_URL=http://localhost:8730 npm run dev
# → listens on :3000
```

For local Teams tests, use the **Bot Framework Emulator** — it skips JWT and lets you hit `http://localhost:3000/api/messages` directly with @bot-mentioned messages.

## Tests

```bash
npm test         # 18 vitest passing (handler logic, no Teams network)
npm run typecheck
```

## Production hardening (out of scope for this PR)

- Full Bot Framework JWT validation via `@azure/msal-node` + `BotFrameworkAuthentication`. The current `server.ts` accepts any POST to `/api/messages` (DEPLOY.md says insists on app-id + secret env vars, but doesn't enforce signature verification yet).
- Adaptive Card responses for richer rendering — current bot uses plain markdown for portability.
- Channel-level OAuth — currently all responses are sync replies in the same channel.
