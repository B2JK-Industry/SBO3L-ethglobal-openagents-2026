# @sbo3l/mobile

Expo / React Native companion app for SBO3L. Tabs: Dashboard · Agents · Audit
· Approvals · Settings. Push notifications fire on `human_2fa` decisions and
deep-link into the approval detail screen; biometric re-auth gates the
approve/deny submission.

See [`DEPLOY.md`](./DEPLOY.md) for build + submission steps and
[`docs/dev3/production/03-mobile-apps-design.md`](../../docs/dev3/production/03-mobile-apps-design.md)
for the full design doc this skeleton implements.

## Quick start

```sh
pnpm --filter @sbo3l/mobile install
pnpm --filter @sbo3l/mobile start
```

Then press `i` (iOS simulator) or `a` (Android emulator) or scan the QR with
Expo Go.

## Status

Skeleton: navigation + auth + push registration + audit feed + approval
resolve flow + biometric gate + capsule QR scanner. Backend endpoints
listed in `src/lib/api.ts` need the hosted-app `/api/mobile/auth/start`
and `/api/t/<slug>/{audit,approvals,push-tokens}` proxies to land before
end-to-end testing.
