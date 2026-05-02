# Mobile apps — iOS + Android design

**Status:** design draft (R13 P78)
**Owner:** Dev 3 (frontend), pending mobile specialist hire
**Trigger:** post-hackathon roadmap, _not_ for the submission demo

## Why mobile

The hosted-app web surface covers desktop ops well, but tenant admins
need to **approve human-in-the-loop policy decisions on the go** — the
\"agent wants to send $50K, please confirm\" prompt is a mobile-first
flow. Email + SMS work today but feel hacky.

Targeting:
- Tenant admins who get pinged for `human_2fa: true` decisions
- Operators who want a glanceable health dashboard during ops
- Auditors reviewing capsules from the field (read-only)

## Stack — Expo / React Native

Reasons to pick Expo over native (Swift / Kotlin):
- **One codebase** — same TypeScript story as hosted-app
- **Component reuse** — design tokens already live in
  `packages/design-tokens`; React Native consumes them via
  `react-native-styled-components` or NativeWind
- **OTA updates** — Expo Updates ships patches without re-submitting
  to App Store / Play Store (minor JS changes only)
- **Push notifications** — Expo's push service abstracts APNs / FCM
- **No Apple Developer / Google Play account needed for the MVP** —
  Expo Go renders the app on a teammate's phone via QR code; full
  store submission is a later milestone

## Routes

```
/                           → Tenant picker (memberships from /v1/me)
/t/<slug>                   → Dashboard (mirrors hosted-app, simpler layout)
/t/<slug>/approvals         → Pending human_2fa decisions list
/t/<slug>/approvals/<id>    → Decision detail + Approve / Deny buttons
/t/<slug>/audit             → Read-only audit timeline (last 100 events)
/t/<slug>/agents            → Agent list (read-only)
/settings                   → Sign in / out, push toggles
```

## Authentication

Same NextAuth backend the hosted-app uses, with native auth flows:
- iOS — `expo-auth-session` opens GitHub OAuth in `SFAuthenticationSession`
- Android — same package opens Custom Tabs

Token stored in `expo-secure-store` (Keychain on iOS, Keystore on
Android). 30-day expiry, silent refresh on app open.

## Push notifications

The daemon's existing `policy.decision.pending_2fa` event hooks
into the push pipeline:

1. Tenant admin enables push in `/settings`
2. App registers with Expo Push, gets `ExponentPushToken[...]`,
   POSTs to `/api/me/push-tokens` with `{ tenant_uuid, token }`
3. Daemon emits `policy.decision.pending_2fa { tenant_uuid, decision_id }`
4. Webhook receiver fans out to all push tokens for that tenant's
   admins, payload `{ deeplink: \"sbo3l://t/<slug>/approvals/<id>\" }`
5. Tap → app opens to the approval detail screen
6. Admin taps Approve / Deny → POST `/v1/decisions/<id>/resolve`
7. Daemon's resolution unblocks the agent

## Offline + cache

- **Audit timeline** — last 100 events cached via React Query +
  `expo-sqlite` for offline read. Stale-while-revalidate on app
  open. _No mutation offline_ — approvals require live daemon
  contact (we can't sign a 2fa response without the current
  challenge).
- **Tenant memberships** — cached on sign-in. Stale OK; refresh
  silently every hour.

## Security

- Cert pinning via `expo-network-pin` against the daemon TLS cert
  (rotation procedure documented in
  `docs/dev3/production/04-cert-pinning-rotation.md`, future)
- Biometric gate on app open (Face ID / Touch ID / fingerprint)
  via `expo-local-authentication` — required for Pro tier, optional
  for Free
- Approval flows always re-authenticate via biometric before sending
  the resolve POST — defence against \"unlocked phone left on desk\"

## Release cadence

- **Internal TestFlight / Internal Track** — every merge to `main`
- **Public beta** — gated on first paying customer's request
- **App Store + Play Store** — gated on revenue milestone (don't
  burn $99/yr Apple Developer + $25 Google Play until a customer
  asks)

## What's NOT in scope

- Tablet / iPad layouts (use the web app)
- Full offline mode with conflict resolution (approvals are
  online-only; approval is a security boundary, not a sync surface)
- macOS Catalyst / Mac Mac App Store — desktop users have the web app
- Desktop notifications — out of scope; web app + SMTP cover it
- Wear OS / watchOS — too small a surface for the approval detail UI

## Rollback

Native apps can't be \"rolled back\" — once on the App Store, the
binary is permanent until the next submission. Mitigation: every
flow gated behind a feature flag served from `/v1/feature-flags`,
so a bad release is a server-side flag flip + force-reload.
