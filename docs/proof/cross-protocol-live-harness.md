# Cross-protocol LIVE harness — round 14 P7

`scripts/cross-protocol-live.sh` — wraps `examples/cross-protocol-killer/`'s `npm run demo` with the right `--daemon` + `--live-*` flags + env preflight.

## What it does

1. **Pre-flight (LIVE mode):** verifies the daemon is healthy at `$SBO3L_DAEMON_URL/v1/healthz`, prints the configured KH workflow id + Sepolia RPC URL, and aborts with a clear error if the daemon isn't reachable
2. **Run:** invokes `npm run demo` with `--daemon`, `--live-kh`, `--live-uniswap`, `--live-ens` (or none in `--mock` mode)
3. **Tee:** captures stdout to `transcript-pretty.txt` (human-readable) AND extracts the embedded `__TRANSCRIPT_JSON__=` line into `transcript.json` (machine-readable)
4. **Re-walk:** runs `npm run verify-output` against the saved log to confirm chain integrity offline
5. **Recording note:** drops `RECORDING.md` with OBS / iTerm settings + capture order + post-production checklist

## Mock-mode artifacts (committed as proof)

| File | Lines | What |
|---|---|---|
| `cross-protocol-live-mock-2026-05-02.json` | 1 | Full machine-readable transcript (10 step records as JSON array) |
| `cross-protocol-live-mock-2026-05-02.txt` | 77 | Human-readable demo log (the same lines a video would capture) |
| `cross-protocol-live-mock-2026-05-02-verify.txt` | 15 | Offline verifier re-walk — **7/7 ✅** |

## Live-mode invocation

```bash
# Daniel's one-shot (after starting the daemon + setting Sepolia RPC env):
SBO3L_DAEMON_URL=http://localhost:8730 \
SBO3L_KH_WORKFLOW_ID=m4t4cnpmhv8qquce3bv3c \
SBO3L_ETH_RPC_URL=https://eth-sepolia.g.alchemy.com/v2/<key> \
./scripts/cross-protocol-live.sh --output /tmp/live-run
```

End state: 4 artifacts in `/tmp/live-run/`. Commit `transcript.json` + `transcript-pretty.txt` + `verify-output.txt` to `docs/proof/`.

## Recording

`RECORDING.md` (auto-generated per run) walks the OBS setup at 1080p / 60fps, the capture order (start OBS → run script → wait for SUMMARY block → stop), and the post-production trim/caption checklist. Target: ~60-second clip, ~10 MB H.264 MP4.

## What this does NOT do

- The script does not run the demo against a daemon I have access to (no daemon in CI; no Sepolia env set up). Live recording is Daniel's one-shot, not CI's.
- The script does not start the daemon for you — it preflights that one is running and aborts cleanly otherwise.
- The script does not record video — that's the OBS step in `RECORDING.md`. Once Daniel uploads the recording, the file goes outside the repo (size).
