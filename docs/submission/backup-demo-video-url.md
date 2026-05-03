# Backup demo video — URL + integrity

> **Status:** placeholder. Daniel fills in after recording per
> [`docs/submission/backup-demo-video/record-playbook.md`](backup-demo-video/record-playbook.md).
>
> The "primary" demo video is the live judge walk-through. This *backup*
> exists so judges who can't catch the live demo (or whose live demo URL
> has rotted) still have a deterministic 3-minute cut they can play.

---

## Unlisted YouTube URL

```
TBD — fill in after recording (YouTube Studio → Visibility = Unlisted, NOT Private)
```

**Important:** the URL must be **Unlisted**, not Private — judges should be
able to play it without a YouTube account. If the YouTube upload fails for
any reason, fallback URL:

```
https://sbo3l-marketing.vercel.app/backup-demo.mp4
```

(served from `apps/marketing/public/backup-demo.mp4` after the next Vercel
deploy)

---

## File integrity

- **Filename:** `final.mp4`
- **Resolution:** 1920×1080
- **Container:** mp4 (H.264 + AAC)
- **Target size:** ≤ 50 MB
- **SHA-256:** `TBD — paste output of \`shasum -a 256 final.mp4\` after stitch`

A judge who downloads the file from the YouTube fallback (or the Vercel
fallback) can verify it's the file we recorded by re-running `shasum -a 256`
and matching against the hash above.

---

## Recording metadata

- **Recording date:** TBD — fill in on record day
- **Source commit:** [`45168cc`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/commit/45168cc) (current `main` at PR open time)
- **Recorder:** Daniel Babjak
- **Source script:** [`docs/submission/demo-video-script.md`](demo-video-script.md)
- **Recording playbook:** [`docs/submission/backup-demo-video/record-playbook.md`](backup-demo-video/record-playbook.md)
- **vhs tape (terminal scenes):** [`docs/submission/backup-demo-video/vhs.tape`](backup-demo-video/vhs.tape)
- **Slide content (Scene 5):** [`docs/submission/backup-demo-video/scene-5-sponsor-wins.md`](backup-demo-video/scene-5-sponsor-wins.md)

---

## Verification ritual

Once Daniel uploads:

1. Open the unlisted YouTube URL in an Incognito window (no Google account)
   to confirm it plays without auth.
2. Download `final.mp4` from the YouTube "..." menu (creator-only download)
   or from the Vercel fallback URL.
3. `shasum -a 256 final.mp4` → matches the hash above.
4. `ffprobe final.mp4` → duration ≈ 3:00 (±10 s slack), dimensions 1920×1080.

If any step fails, re-record per the playbook (ETA ~30 min).
