# SBO3L mobile — assets

This directory holds icon + splash + adaptive-icon assets for Expo
build. Currently empty so Expo uses its built-in defaults rather than
pointing `app.json` at files that don't exist (Codex review fix on
PR #309 — broken file references would have failed `eas build`).

## Required files (when branded assets are ready)

| File | Resolution | Used by |
|---|---|---|
| `icon.png` | 1024×1024 PNG | iOS app icon source (Expo derives all sizes) |
| `adaptive-icon.png` | 1024×1024 PNG, transparent BG | Android adaptive icon foreground |
| `splash.png` | 1242×2436 PNG, transparent or `#0a0e1a` BG | iOS + Android splash |

## How to wire them back into `app.json`

Once committed, restore the references in `app.json`:

```json
{
  "expo": {
    "icon": "./assets/icon.png",
    "splash": {
      "image": "./assets/splash.png",
      "resizeMode": "contain",
      "backgroundColor": "#0a0e1a"
    },
    "android": {
      "adaptiveIcon": {
        "foregroundImage": "./assets/adaptive-icon.png",
        "backgroundColor": "#0a0e1a"
      }
    }
  }
}
```

## Brand reference

The marketing site OG image at `apps/marketing/public/og-default.svg`
shows the visual brand (wallet→mandate tagline, accent green
`#4ade80`, dark bg `#0a0e1a`). A designer can derive the mobile
assets from it.
