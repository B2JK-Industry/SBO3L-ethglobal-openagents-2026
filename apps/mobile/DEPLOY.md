# SBO3L mobile — deploy + submission runbook

## Local development

```sh
pnpm --filter @sbo3l/mobile install
pnpm --filter @sbo3l/mobile start            # opens Expo Dev Tools
# Press i for iOS simulator, a for Android emulator, w for web
```

The Expo Go app on a teammate's phone scans the QR — no Apple Developer / Play
Console account needed.

## Backend pointer

`EXPO_PUBLIC_API_BASE_URL` defaults to `https://sbo3l-app.vercel.app` and reads
through `expoConfig.extra.apiBaseUrl`. Override per-shell when targeting a
local hosted-app:

```sh
EXPO_PUBLIC_API_BASE_URL=http://192.168.0.42:3000 pnpm --filter @sbo3l/mobile start
```

## EAS — TestFlight + Internal Track

```sh
pnpm --filter @sbo3l/mobile dlx eas-cli login
pnpm --filter @sbo3l/mobile dlx eas-cli init    # prompts for projectId
pnpm --filter @sbo3l/mobile build:ios           # uses preview profile by default
pnpm --filter @sbo3l/mobile build:android
```

Set the `eas.projectId` placeholder in `app.json` to the value EAS assigns at
`init` time. The `eas.json` `submit.production` block needs `ascAppId` +
`appleTeamId` filled before iOS submission; Android needs the
`play-service-account.json` file in the working directory.

## Apple + Google accounts

- **Apple Developer** ($99/yr) — required for TestFlight + App Store. Hold
  off until first paying customer asks; Expo Go covers internal demos.
- **Google Play Developer** ($25 one-time) — required for Internal Track.
  Same gating rule.

## Push notifications

Configured against Expo Push (`expo-notifications`) — no APNs / FCM keys
needed for development. For production:

1. `eas credentials` configures APNs key + FCM server key on the EAS side
2. The hosted-app `/api/t/<slug>/push-tokens` endpoint stores the
   ExponentPushToken[...] tokens against the membership
3. Daemon webhook fans-out to all push tokens for a tenant when it emits
   `policy.decision.pending_2fa` (see daemon docs/events.md)

## Deep links

The `sbo3l://` scheme routes via expo-linking. Fan-out push notification
payload should contain:

```json
{ "deeplink": "sbo3l://approval/<decision-id>?tenant=<slug>" }
```

Tap → app opens to `app/approval/[id].tsx` directly.

## Out of scope for v0

- Tablet / iPad layouts (use the web app)
- Offline mode beyond read-only audit cache (approvals are online-only by
  design — see `docs/dev3/production/03-mobile-apps-design.md`)
- macOS Catalyst — desktop has the web app
