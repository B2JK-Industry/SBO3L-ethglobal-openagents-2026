# SBO3L demo video — record checklist

> **Goal:** ship a 3-minute demo video to YouTube in ~45 min total work.
> **Output:** `final.mp4` at YouTube public URL → paste into ETHGlobal submission form.

---

## Pre-flight (5 min, one-time)

```bash
# macOS
brew install ffmpeg vhs

# verify
ffmpeg -version | head -1   # ffmpeg version 7.x
vhs --version               # vhs version v0.x
```

The repo already has playwright installed in `apps/marketing/` from the screenshot work earlier today.

---

## Step 1 — Auto-generate terminal scenes (3 min)

```bash
cd docs/submission/backup-demo-video
vhs vhs.tape
# → outputs terminal-scenes.mp4 (~75 seconds, 1920x1080)
```

This covers Scene 2 (install + verify, 0:30-1:00) and Scene 4 (live KH workflow, 1:30-2:00).

---

## Step 2 — Auto-record browser scenes (5 min)

```bash
cd ../../../apps/marketing
node ../../docs/submission/video-final-record-pack/record-browser-scenes.mjs
# → outputs scenes/scene-1-home.mp4, scenes/scene-3-proof.mp4,
#   scenes/scene-5-uniswap.mp4, scenes/scene-6-outro.mp4
```

These cover Scene 1 (homepage hero), Scene 3 (WASM verifier drag-drop), Scene 5 (UNI-A1 Etherscan), Scene 6 (outro slide).

---

## Step 3 — Record voiceover (15 min)

1. Open QuickTime → File → New Audio Recording
2. Open `voiceover-script.md` on a second screen / iPad / printed page
3. Read the script naturally. Total ~3 minutes at conversational pace.
4. **Stop after each scene** (use the timecodes in the script as your section breaks) — easier to re-record one scene than the whole thing.
5. Save as `voiceover.m4a`.

**Tip:** AirPods Pro mic is fine. If you want studio quality, a Yeti USB mic is the easy upgrade.

---

## Step 4 — Stitch everything (5 min)

```bash
cd docs/submission/video-final-record-pack
mkdir -p output
# Copy your generated clips into ./scenes/
cp ../../backup-demo-video/terminal-scenes.mp4 scenes/
cp ../../../apps/marketing/scenes/*.mp4 scenes/
# Copy your voiceover
cp ~/path/to/voiceover.m4a ./

# Run stitch
bash stitch.sh
# → outputs output/final.mp4 (~3 min, 1080p, ≤50MB)
```

---

## Step 5 — Upload (5 min)

1. Open YouTube → Upload
2. Title: `SBO3L — Don't give your agent a wallet. Give it a mandate. (ETHGlobal Open Agents 2026)`
3. Description: paste from `submission-form-video-description.md` (TODO)
4. Visibility: **Public** (NOT unlisted — ETHGlobal judges need to find it)
5. Copy URL → paste into ETHGlobal form Video field

---

## Total time budget

| Step | Time |
|---|---|
| Pre-flight install | 5 min |
| Step 1 — terminal scenes | 3 min |
| Step 2 — browser scenes | 5 min |
| Step 3 — voiceover | 15 min |
| Step 4 — stitch | 5 min |
| Step 5 — upload | 5 min |
| Buffer + retakes | 7 min |
| **Total** | **~45 min** |

---

## Troubleshooting

**vhs.tape fails:** verify font installed (Dracula theme uses default monospace; should work out-of-box on macOS).

**Playwright video has black bars:** check `scenes/` exists in `apps/marketing/` and ffmpeg is installed (playwright video uses ffmpeg internally).

**Audio out of sync:** use `ffmpeg -itsoffset 0.3 -i voiceover.m4a ...` in stitch.sh to add delay (positive = audio later, negative = audio earlier).

**Final.mp4 > 50MB:** add `-crf 28` (was `-crf 23`) to ffmpeg in stitch.sh — slightly lower quality, much smaller file.
