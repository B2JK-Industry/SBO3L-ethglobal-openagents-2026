# Submit SBO3L mobile to TestFlight + Google Play Internal Track

Step-by-step Daniel runs once Apple Developer + Google Play accounts are
provisioned. The Expo skeleton in
[`apps/mobile/`](../../apps/mobile/) is build-ready; this doc covers the
external-account setup + first submission only.

## Pre-requisites

| Requirement | How to get | Cost |
|---|---|---|
| Apple Developer | https://developer.apple.com/programs/ | $99/yr |
| Google Play Console | https://play.google.com/console/signup | $25 one-time |
| Expo account | https://expo.dev/signup | Free for first projects |
| EAS CLI | `pnpm dlx eas-cli` (no install needed) | Free tier covers preview builds |

Sequence matters: pay Apple first (their queue is 24–48h to verify your
identity), then Google, then Expo.

## Phase 1 — One-time EAS init

```sh
# From repo root
pnpm --filter @sbo3l/mobile install
pnpm --filter @sbo3l/mobile dlx eas-cli login          # browser opens
pnpm --filter @sbo3l/mobile dlx eas-cli init           # prompts; assigns projectId
```

EAS replies with a project ID like `00112233-4455-6677-8899-aabbccddeeff`.
Drop it into `apps/mobile/app.json`:

```json
{ "expo": { "extra": { "eas": { "projectId": "<the-id>" } } } }
```

Commit + push.

## Phase 2 — iOS preview build (no Apple Dev account needed)

This validates the Expo skeleton compiles for iOS without burning the $99/yr
account yet.

```sh
pnpm --filter @sbo3l/mobile build:ios
```

EAS spins up a build server, signs with an EAS-managed simulator cert,
emails when done (~12 min). Download the `.app` bundle, drop into iOS
Simulator. **Sanity check:** sign-in screen renders, biometric gate prompts.

## Phase 3 — Android preview build (no Play Console account needed)

```sh
pnpm --filter @sbo3l/mobile build:android
```

Same flow, produces a `.apk` you can sideload onto any Android phone via
`adb install <file>.apk` for a friend's device test. **Sanity check:** push
permission prompt fires on first sign-in.

## Phase 4 — TestFlight submission (requires Apple Developer)

After Apple Developer account is active:

1. Log into App Store Connect, create a new app:
   - Bundle ID: `dev.sbo3l.mobile` (matches `app.json`)
   - SKU: `sbo3l-mobile-2026`
   - Primary Language: English (U.S.)

2. Get the App Store Connect App ID and Team ID. Edit
   `apps/mobile/eas.json`:

   ```json
   {
     "submit": {
       "production": {
         "ios": {
           "ascAppId": "1234567890",
           "appleTeamId": "ABCDEFGHIJ"
         }
       }
     }
   }
   ```

3. Run the production build + submit:

   ```sh
   pnpm --filter @sbo3l/mobile dlx eas-cli build --platform ios --profile production
   pnpm --filter @sbo3l/mobile submit:ios
   ```

   EAS builds, uploads to App Store Connect, registers the build with
   TestFlight. Apple's "Processing" stage takes ~30 min. Add internal
   testers (your email) under TestFlight → Internal Testing.

4. First submission triggers Apple's **App Review** for TestFlight Beta —
   typically 24–48h, sometimes immediate. They'll ping you about export
   compliance (no encryption beyond HTTPS; tick the EAR-exempt boxes).

## Phase 5 — Google Play Internal Track (requires Play Console)

After Play Console account is active + identity verified (1–7 days):

1. Create a new app in Play Console:
   - App name: SBO3L
   - Default language: English (United States)
   - App or game: App
   - Free or paid: Free

2. Generate a service account for upload automation:
   - Play Console → Setup → API access → Create service account in GCP
   - Grant **Service Account User** role
   - Download JSON key
   - Save as `apps/mobile/play-service-account.json` (it's already in
     `apps/mobile/.gitignore` — DO NOT COMMIT)

3. Run production build + submit:

   ```sh
   pnpm --filter @sbo3l/mobile dlx eas-cli build --platform android --profile production
   pnpm --filter @sbo3l/mobile submit:android
   ```

   Goes to **Internal Track** by default (per `eas.json`). Internal Track
   doesn't need Play Store review — testers (up to 100 emails you list)
   can install via a tester URL within 1h.

4. Promote to Production track later via Play Console UI when ready.

## Phase 6 — Push notifications (production credentials)

Expo Push handles token translation in dev. For production push:

```sh
pnpm --filter @sbo3l/mobile dlx eas-cli credentials
```

iOS path: configures APNs key (download .p8 from Apple Dev → Keys).
Android path: configures FCM server key from Firebase project linked to
the Play Console app.

EAS stores credentials securely; rotation is a re-run of the same command.

## Phase 7 — Versioning + OTA updates

Expo supports OTA updates for JS-only changes (no native binary rebuild):

```sh
pnpm --filter @sbo3l/mobile dlx eas-cli update --branch production
```

OTA bypasses Apple/Google review for pure JS/asset changes. Native code
changes (new Expo SDK, new permissions, new modules) require a full
binary rebuild + resubmit.

`app.json` `version` field bumps with every native rebuild;
`eas.json` `production.autoIncrement: true` handles build numbers
automatically.

## Phase 8 — Monitoring

Once live:
- **Sentry** — `npx @sentry/wizard -i reactNative` to wire crash reporting
- **Expo Insights** — built-in if you opt in during `eas init`
- **Push delivery rates** — Expo dashboard shows daily delivery success
  rate; alert on drop below 95%

## Common gotchas

| Symptom | Fix |
|---|---|
| `eas build` fails with "no projectId" | Re-run `eas init`, paste the new ID into app.json |
| iOS build green but won't install on device | Distribution profile mismatch — production build needs an App Store Distribution provisioning profile, not the simulator one |
| Android build green but Play Console rejects with "duplicate package" | `app.json` `android.package` collides with another listed app — change `dev.sbo3l.mobile` to a unique slug |
| Push works on iOS Simulator but not real iPhone | Simulators use Apple's Sandbox APNs; real devices need the production APNs key in EAS credentials |
| Biometric prompt never appears on iOS | `NSFaceIDUsageDescription` missing from `app.json` `ios.infoPlist` — already set in our config but verify after any app.json edit |

## What NOT to ship

- Live push token webhook from the daemon — needs `/api/t/<slug>/push-tokens`
  endpoint in hosted-app, currently a TODO. Mobile registers tokens but
  the daemon side is the missing fan-out target.
- Real-time approval polling — current implementation polls
  `/api/t/<slug>/approvals` on tab focus; production wants the daemon
  to push via Expo Push when a `human_2fa` decision lands.
- Tablet / iPad layout — explicit out-of-scope per the design doc.
- Apple Watch / Wear OS — too small for the approval-detail UI.

## Cost-aware staging

If hackathon doesn't justify the $124 Apple+Google sunk cost yet:

1. **Pre-store**: ship via Expo Go (`expo start` + QR scan on
   teammate's phone) — covers all internal testing without paying anyone.
2. **Soft launch**: TestFlight Internal Testing (no review needed beyond
   the first build) — pay Apple, skip Google for now.
3. **Public launch**: TestFlight External Testing (App Review required)
   + Play Internal Track — pay both.
4. **Production**: Promote both apps to production tracks once you have
   ≥10 active testers and zero blocking bugs in the past 7 days.
