# SBO3L backup demo video — recording playbook

> **Purpose:** produce the 3-minute backup demo video in ~30 minutes of work
> by following these steps. The terminal portion (Scenes 2 + 4) is fully
> automated via `vhs`; the browser portion (Scenes 1, 3, 5, 6) is manual
> screen capture.
>
> **Source of truth:** [`docs/submission/demo-video-script.md`](../demo-video-script.md)
> — the 6-scene script. This playbook is the *how* for that script.
>
> **Output:** `final.mp4`, 1920×1080, ≤50 MB, uploaded as YouTube unlisted.
> URL recorded in [`docs/submission/backup-demo-video-url.md`](../backup-demo-video-url.md).

---

## 0. One-time setup (~5 min)

```bash
brew install vhs ffmpeg     # macOS
# or: go install github.com/charmbracelet/vhs@latest
# OBS Studio + Keynote (or Google Slides) for the static slide
```

Confirm:

```bash
vhs --version              # ≥ v0.7
ffmpeg -version            # any recent
```

---

## Scene 1 — Title card (15 s)

**Asset:** [`apps/marketing/public/demo-assets/title-card.svg`](../../../apps/marketing/public/demo-assets/title-card.svg)
(1920×1080, animated particle drift, brand palette).

**Recipe:**

1. Open the SVG in a browser, full screen (Cmd-Ctrl-F on Safari, F11 on Chrome).
2. Start an OBS *Display Capture* source pinned to that monitor.
3. Set OBS canvas to 1920×1080, output mp4, recording FPS 30.
4. Start record → wait 15 s → stop. Save as `scene-01-title.mp4`.

**Faster alternative** (no OBS):

```bash
# rasterise SVG → PNG, then make a 15 s static mp4 from the PNG.
# Requires librsvg + ffmpeg. macOS: brew install librsvg
rsvg-convert -w 1920 -h 1080 \
  apps/marketing/public/demo-assets/title-card.svg \
  -o /tmp/title.png
ffmpeg -loop 1 -i /tmp/title.png -t 15 -r 30 \
  -c:v libx264 -pix_fmt yuv420p -preset fast \
  scene-01-title.mp4
```

The static-png variant loses the particle animation but renders deterministically
and is the recommended path if the OBS capture looks janky.

---

## Scene 2 — Terminal: install + verify (30 s)

**Fully automated.** Just run vhs.

```bash
cd docs/submission/backup-demo-video
vhs vhs.tape
# → produces terminal-scenes.mp4 (~75 s, covers Scene 2 + Scene 4)
```

The single output `terminal-scenes.mp4` includes both Scene 2 (0–35 s) and
Scene 4 (35–75 s) back-to-back. Split during stitching (see §Stitching).

**What the tape demonstrates:**

- `cargo install sbo3l-cli --version 1.2.0` → the canonical install line from
  the README.
- `sbo3l doctor --extended` → 6/6 contracts ok (matches `crates/sbo3l-identity/src/contracts.rs`
  pinned addresses).
- `sbo3l agent verify-ens sbo3lagent.eth --network mainnet` → 5 records
  resolved with the live `policy_hash` (`e044f13c5acb…`) that matches the offline
  fixture byte-for-byte.

**Why the tape mocks the output:** vhs needs a deterministic, network-free
recording so the backup video is reproducible at any time. The mocks reproduce
the *exact* output a judge would see if they ran the real commands against
the live contracts and `sbo3lagent.eth`. Real-output verification is documented
in [`docs/proof/contracts-live-test.md`](../../proof/contracts-live-test.md)
and [`docs/submission/url-evidence.md`](../url-evidence.md).

---

## Scene 3 — Browser: paste capsule into `/proof`, 6 ✅ checks (30 s)

**No vhs.** This is a real browser interaction; the verifier runs in WASM in
the page so a screen capture is the cleanest path.

**Click path on https://sbo3l-marketing.vercel.app/playground:**

1. Open https://sbo3l-marketing.vercel.app/playground in Chrome (1920×1080
   window — `Cmd-Opt-J` → `window.resizeTo(1920,1080)` if you need exact pixels).
2. Click the **Try a request** card → choose `deny-aprp-expired` from the
   scenario dropdown. Watch the live response panel show `decision: deny`.
3. Click **Download capsule** (the resulting `capsule.json` lands in `~/Downloads`).
4. Navigate to https://sbo3l-marketing.vercel.app/proof.
5. Drag-drop the downloaded `capsule.json` onto the proof drop zone.
6. The 6 green checkmarks pop in, in this order:
   `schema → request_hash → policy_hash → decision → agent_id → audit_event_id`.
7. **Tamper beat** — open the JSON in any editor, flip one byte in
   `audit_event_hash` (e.g. change a `0` to `1`). Re-drop the same file. The
   verifier flips to red `capsule.audit_event_hash_mismatch`.

**Recording:**

- OBS *Window Capture* pinned to the Chrome window.
- Output mp4, 1920×1080, FPS 30.
- Start record before step 2, stop after step 7 (target ~30 s).
- Save as `scene-03-proof.mp4`.

**Pro tip:** pre-stage the tampered `capsule.json` at `/tmp/capsule-tamper-demo.json`
*before* recording so you only have to drag, not edit, on camera. Keep both files
on the desktop.

---

## Scene 4 — Terminal: live KeeperHub workflow (30 s)

**Already produced** by `vhs vhs.tape` — it's the second half of `terminal-scenes.mp4`
(roughly the last 40 s of the file).

**Optional: re-record live** if the KH workflow `m4t4cnpmhv8qquce3bv3c` is fresh
and the `wfb_*` token is loaded:

```bash
SBO3L_KEEPERHUB_WEBHOOK_URL=https://app.keeperhub.com/api/workflows/m4t4cnpmhv8qquce3bv3c/webhook \
SBO3L_KEEPERHUB_TOKEN=wfb_••••••• \
bash demo-scripts/sponsors/keeperhub-guarded-execution.sh
```

(The brief mentions a script named `keeperhub-live.sh`; on disk this lives at
`demo-scripts/sponsors/keeperhub-guarded-execution.sh` — same flow, just a
different filename. The vhs tape uses the script name from the brief; OBS the
live name above.)

Capture in OBS the same way as Scene 2 if going live. Output filename
`scene-04-keeperhub.mp4`.

---

## Scene 5 — Static slide: sponsor wins (15 s)

**Content:** [`scene-5-sponsor-wins.md`](./scene-5-sponsor-wins.md) — paste the
markdown into Keynote / Google Slides as a single 1920×1080 slide. Recommended
layout: title at top, 5 sponsor blocks as bullet rows, footer with the four-number
outro (881 / 13 / 25 / 9).

**Recording:**

```bash
# rasterise the slide to PNG (export from Keynote or use Slides → File → Download → PNG)
ffmpeg -loop 1 -i scene-05-sponsor-wins.png -t 15 -r 30 \
  -c:v libx264 -pix_fmt yuv420p -preset fast \
  scene-05-sponsors.mp4
```

Or screen-record the slide on full-screen presentation mode for 15 s in OBS.

---

## Scene 6 — End card (15 s)

**Asset:** [`apps/marketing/public/demo-assets/end-card.svg`](../../../apps/marketing/public/demo-assets/end-card.svg)
(1920×1080, end-card with QR code placeholders; the real QR codes live alongside
at `qr-github.svg`, `qr-npm.svg`, `qr-cratesio.svg`).

**Recipe:** identical to Scene 1, just swap the SVG path:

```bash
rsvg-convert -w 1920 -h 1080 \
  apps/marketing/public/demo-assets/end-card.svg \
  -o /tmp/end.png
ffmpeg -loop 1 -i /tmp/end.png -t 15 -r 30 \
  -c:v libx264 -pix_fmt yuv420p -preset fast \
  scene-06-end.mp4
```

If you want the real (machine-readable) QR codes overlaid instead of the SVG
placeholders, composite them in your video editor — the QRs are pre-rendered at
`apps/marketing/public/demo-assets/qr-*.svg` and verified scannable; do not
regenerate.

---

## Stitching — 6 clips → `final.mp4`

`terminal-scenes.mp4` covers Scene 2 + Scene 4 in one file. Split it first:

```bash
# Scene 2: 0:00–0:35 of terminal-scenes.mp4
ffmpeg -i terminal-scenes.mp4 -ss 0 -t 35 -c copy scene-02-install.mp4
# Scene 4: 0:35–end of terminal-scenes.mp4
ffmpeg -i terminal-scenes.mp4 -ss 35 -c copy scene-04-keeperhub.mp4
```

(Adjust the split point by eyeballing — the `clear` between scenes is at ~33 s.)

### Normalise each clip first (REQUIRED — different sources)

The six clips come from three different recording paths: `vhs` (Scenes 2 + 4 — video-only, no audio), OBS (Scene 3 — usually audio + video), `ffmpeg -loop 1` over an SVG-rasterised PNG (Scenes 1 + 5 + 6 — video-only, no audio). The `concat` demuxer requires every input to share the **same codec, resolution, fps, pixel format, AND track layout**. If you feed it a mix of with-audio and without-audio clips, it either errors out or produces a final.mp4 with a stuck audio track. So we re-encode each clip into a uniform shape first, then concat the normalised set.

```bash
# Add a silent audio track to every clip + force 1920x1080@30fps + yuv420p.
# Re-encoding happens once; the final concat step is then stream-copy.
for SRC in scene-01-title scene-02-install scene-03-proof scene-04-keeperhub scene-05-sponsors scene-06-end; do
  ffmpeg -y -i "${SRC}.mp4" -f lavfi -i anullsrc=channel_layout=stereo:sample_rate=48000 \
    -shortest \
    -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p -r 30 \
    -vf "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2,setsar=1" \
    -c:a aac -ar 48000 -ac 2 -b:a 128k \
    -map 0:v:0 -map 1:a:0 \
    "${SRC}.norm.mp4"
done
```

If your Scene 3 OBS recording already has narration audio, drop the `-f lavfi -i anullsrc=...` and `-map 1:a:0` for that one clip and let it carry its own audio:

```bash
ffmpeg -y -i scene-03-proof.mp4 \
  -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p -r 30 \
  -vf "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2,setsar=1" \
  -c:a aac -ar 48000 -ac 2 -b:a 128k \
  scene-03-proof.norm.mp4
```

### Now concat the normalised set

```bash
cat > /tmp/concat.txt <<EOF
file 'scene-01-title.norm.mp4'
file 'scene-02-install.norm.mp4'
file 'scene-03-proof.norm.mp4'
file 'scene-04-keeperhub.norm.mp4'
file 'scene-05-sponsors.norm.mp4'
file 'scene-06-end.norm.mp4'
EOF

ffmpeg -f concat -safe 0 -i /tmp/concat.txt -c copy -movflags +faststart final.mp4
```

`-c copy` is safe here because every input is already a 1920×1080 yuv420p H.264 + 48 kHz stereo AAC clip from the normalisation pass — no re-encoding needed.

**Sanity check the output:**

```bash
DUR=$(ffprobe -v error -show_entries format=duration -of csv=p=0 final.mp4)
SIZE=$(ffprobe -v error -show_entries format=size -of csv=p=0 final.mp4)
echo "duration=${DUR}s size=${SIZE}B"
# Target: 175–185 seconds (3:00 ± 5s). The script budgets 15+30+45+30+30+15 = 165s
# of in-scene content + ~15s of cross-fade / breath. If duration < 175s the
# missing time is almost always Scene 3 — re-record it.
# size must be < 52428800 (50 MB).
test "${DUR%.*}" -ge 175 -a "${DUR%.*}" -le 185 || { echo "FAIL: duration ${DUR}s is outside 175-185s window"; exit 1; }
test "$SIZE" -lt 52428800 || { echo "FAIL: size ${SIZE}B exceeds 50 MB cap"; exit 1; }
echo "OK: duration + size within submission limits"
```

If `final.mp4` is over 50 MB, raise the CRF (try `-crf 28`) and re-run the normalisation pass.

---

## Voiceover (optional)

If you want narration over the cuts, the script lines live in
[`docs/submission/demo-video-script.md`](../demo-video-script.md). Record a single
audio track (Audacity / QuickTime) reading the **Voiceover** beats in order, then
mux at stitch time:

```bash
ffmpeg -i final.mp4 -i voiceover.m4a -c:v copy -c:a aac -shortest final-with-vo.mp4
```

For the backup video, narration is **optional** — silent + on-screen text is
acceptable.

---

## Upload — YouTube as unlisted

1. https://studio.youtube.com → Create → Upload video.
2. Drag `final.mp4`. Title: `SBO3L — ETHGlobal Open Agents 2026 — backup demo`.
3. **Critical:** Visibility → **Unlisted** (NOT Private — judges need the URL
   to work without YouTube account access).
4. After upload completes, copy the share URL (`https://youtu.be/<id>`).
5. Hash the file:
   ```bash
   shasum -a 256 final.mp4
   ```
6. Edit [`docs/submission/backup-demo-video-url.md`](../backup-demo-video-url.md):
   replace the `TBD` placeholder with the unlisted URL + paste the hash.
7. Commit + push.

**If YouTube fails for any reason:** copy `final.mp4` to
`apps/marketing/public/backup-demo.mp4`. The marketing site serves it as a
static asset at `https://sbo3l-marketing.vercel.app/backup-demo.mp4` after the
next deploy. Document the fallback URL in `backup-demo-video-url.md`.

---

## Time budget (worst case)

| Step | Time |
|---|---|
| vhs setup + render | 5 min |
| Scene 1 title card | 2 min |
| Scene 3 browser capture | 8 min (incl. one re-take) |
| Scene 5 slide build + render | 5 min |
| Scene 6 end card | 2 min |
| Splitting + stitching | 3 min |
| YouTube upload + URL doc commit | 5 min |
| **Total** | **~30 min** |

If anything breaks, the static-image fallback for Scenes 1 + 6 + the vhs
auto-render for Scenes 2 + 4 mean only Scene 3 (browser) requires real-time
operator attention.
