# End-to-end user acceptance test — 2026-05-02

> **Filed by:** Heidi (QA + Release agent), final UAT round.
> **Date:** 2026-05-02 ~21:30 CEST.
> **Repo state:** main HEAD `5f9a199`.
> **Mode:** real CLI install + actual onchain calls + actual HTTP requests + binary inspection. **No browser** — interactive WASM page tests deferred to Daniel.

This is what happens when you actually USE the product end-to-end (not just curl smoke-tests of static URLs). I followed the documented install paths, ran every documented command, and called every live URL the way a wallet / SDK consumer would.

## Summary verdict

**🟢 The core product works.** CLI installs from crates.io, ENS resolution returns real onchain data, capsule verification runs, marketing pages render, WASM verifier serves a real binary, contracts are deployed and queryable.

**🚨 4 bugs found.** Two are critical (CCIP-Read flow has both a contract-side malformed URL AND a wrong-Vercel-project alias), two are documentation drift (CLI command names changed but docs reference old names).

## Test inventory

| Surface | Test performed | Result |
|---|---|---|
| `cargo install sbo3l-cli --version 1.2.0` | Real install from crates.io | ✅ 3m12s, binary at `~/.cargo/bin/sbo3l` |
| `sbo3l --version` | Verify binary works | ✅ "sbo3l 1.2.0" |
| `sbo3l agent verify-ens sbo3lagent.eth` | Live mainnet ENS lookup via PublicNode RPC | ✅ 5 records resolved |
| `sbo3l passport verify --path <golden>` | Structural verify on golden capsule | ✅ "structural verify: ok" |
| `sbo3l passport verify --path <tampered>` | Should reject 4 tampered fixtures | ⚠️ All "ok" — see Bug #6 |
| `sbo3l passport explain --path <golden>` | Capsule explanation output | ✅ Full readable summary |
| 11 marketing routes | HTTP fetch + title/h1 inspection | ✅ All 200 with proper titles |
| `/proof` page WASM verifier | Download `sbo3l_core_bg.wasm` | ✅ 2.4MB, valid WASM magic bytes `\0asm` |
| `/marketplace` 5 starter policies | h2 enumeration | ✅ All 5 policies render |
| `/playground` 8 scenarios | Scenario data-attr enumeration | ✅ All 8 scenarios present |
| `/try` 8-step walkthrough | Step h2 enumeration | ✅ All 8 steps + "Run it yourself" |
| 4 demo step pages | Per-page fetch + title verify | ✅ All 200, 11-20KB each |
| Mainnet `sbo3lagent.eth` namehash | RPC `urls(0)` on OffchainResolver | ✅ contract responds |
| 6 Sepolia contracts bytecode | `eth_getCode` per address | ✅ All 6 deployed |
| CCIP gateway short URL | `/api/<sender>/<data>.json` request | 🚨 **Returns marketing site, not gateway** (Bug #1) |
| CCIP gateway long preview URL | Same request | ✅ Returns proper JSON 400 with descriptive error |
| OffchainResolver `urls(0)` onchain value | `cast call ... 'urls(uint256)(string)' 0` | 🚨 **Malformed URL** (Bug #2) |
| Capsule mirror `b2jk-industry.github.io` | HTTP fetch + JSON parse | ✅ 3892 bytes, valid v1 capsule schema |
| `sbo3l:endpoint` ENS record | Live mainnet read | 🟡 **localhost** (Bug #3) |
| `sbo3l passport resolve` (per docs) | Run documented command | 🟢 **Command doesn't exist** (Bug #4) |

## Bugs found

### 🚨 Bug #1 — `sbo3l-ccip.vercel.app` serves marketing site, not CCIP gateway

**Repro:**
```bash
curl -s https://sbo3l-ccip.vercel.app/ | grep -oE '<title>[^<]+</title>'
# → <title>SBO3L — Don't give your agent a wallet. Give it a mandate.</title>

curl -s -w '%{http_code}' https://sbo3l-ccip.vercel.app/api/0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3/0x.json
# → 404 NOT_FOUND
```

The Vercel project alias `sbo3l-ccip.vercel.app` points at the marketing project. The actual CCIP gateway lives at the long preview URL `https://sbo3l-ccip-i05tmr4jc-babjak-daniel-5461s-projects.vercel.app/` and works correctly when called there.

**Severity:** Critical for the ENS Most Creative bounty narrative. The "live CCIP-Read flow" claim is technically broken at the canonical URL.

**Daniel-side fix:** in Vercel dashboard, re-link the project alias `sbo3l-ccip.vercel.app` to the `apps/ccip-gateway/` project (currently linked to marketing).

### 🚨 Bug #2 — OffchainResolver onchain has malformed gateway URL

**Repro:**
```bash
cast call 0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3 'urls(uint256)(string)' 0 \
  --rpc-url https://ethereum-sepolia-rpc.publicnode.com
# → "https://sbo3l-ccip.vercel.app/api/{sender/{data}.json}"
```

Expected (per ENSIP-10 + the project's own test fixtures): `https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json`

Bug: missing `}` after `sender`, extra `}` at end. Wallets following ENSIP-25 to do CCIP-Read against `*.sbo3lagent.eth` will:
1. Decode this URL template
2. Substitute `{sender}` and `{data}` literally
3. Either fail template substitution (depends on wallet impl) OR send a malformed request

**Severity:** Critical. The contract has NO `setUrls` setter — `urls` is `string[] public urls` populated only at construction. Fix requires:
- Redeploy OffchainResolver with corrected URL
- Update mainnet ENS record on `sbo3lagent.eth` to point at the new contract

**Combined impact of #1 and #2:** even if a wallet correctly resolved the literal URL, it would hit the marketing site (Bug #1) and 404. CCIP-Read is **non-functional in production**.

### 🟡 Bug #3 — `sbo3l:endpoint` ENS record points to localhost

**Repro:**
```bash
sbo3l agent verify-ens sbo3lagent.eth --rpc-url https://ethereum-rpc.publicnode.com
# → sbo3l:endpoint  actual="http://127.0.0.1:8730/v1"
```

The `sbo3l:endpoint` text record on mainnet points to `http://127.0.0.1:8730/v1`. This is the operator's local daemon and is unreachable from any non-local machine.

**Mitigation:** the trust assertion is `policy_hash` + `audit_root`, not endpoint reachability. The capsule is self-contained; you don't need to call the endpoint to verify a capsule. So this is a **🟡 not critical** — but it's a confusing data point for judges who try to call the endpoint.

**Daniel-side fix:** either update the ENS text record to a public-facing endpoint, OR document that `sbo3l:endpoint` is operator-side and not meant to be reached externally. Updated `HANDOFF-FOR-DANIEL.md` Demo 2 with a "Heads up on `sbo3l:endpoint`" note explaining the design intent.

### 🟢 Bug #4 — Stale CLI command in user-facing docs

**Repro:**
```bash
sbo3l passport resolve sbo3lagent.eth
# → error: unrecognized subcommand 'resolve'
```

7 user-facing docs reference `sbo3l passport resolve` which doesn't exist. Real command: `sbo3l agent verify-ens`.

**Affected:**
- `docs/submission/HANDOFF-FOR-DANIEL.md` (3 mentions — fixed)
- `docs/submission/bounty-ens-most-creative.md` (fixed)
- `docs/submission/bounty-ens-ai-agents.md` (fixed)
- `docs/submission/demo-video-script.md` (fixed)
- `docs/submission/rehearsal-walkthrough-r14-2026-05-02.md` (fixed)
- `docs/submission/live-url-inventory.md` (fixed)
- `docs/submission/rehearsal-walkthrough-r12-2026-05-02.md` (left as-is — historical record)

**Fixes shipped in this PR.** All future judge clicks will see the working command.

### 🟢 Bug #5 — CLI help text references obsolete phase numbers

The `sbo3l passport --help` text still says "P1.1 is structural-only", "P5.1/P6.1 work" — but the project is now at v1.2.0 with all of Phase 3 shipped. Cosmetic; not blocking.

**Not fixed in this PR** (would require touching the CLI source code which is out of self-review scope).

### 🟢 Bug #6 — `passport verify` accepts tampered capsules silently

The CLI `sbo3l passport verify --path <tampered>` returns "structural verify: ok" on all 4 tampered v2 fixtures. This is **technically correct** per the help text ("structural-only"), but a user reading the command name "verify" will reasonably expect a full crypto check.

The full crypto check happens in the WASM verifier on `/proof` — there's no CLI equivalent today.

**Not fixed in this PR** (CLI behavior change). **Recommended follow-up:** either:
- Rename CLI command to `passport verify-structural` to clarify scope
- Add a `passport verify --strict` flag that runs full crypto checks (would require porting the WASM logic to native, which the underlying `sbo3l-core` already supports)

## Confirmed-working surfaces (positive list)

These are the things that work end-to-end as a real user:

1. **CLI install path** — `cargo install sbo3l-cli --version 1.2.0` succeeds from crates.io in 3 minutes.
2. **CLI binary** — runs, version reports correctly, all subcommand `--help`s render.
3. **Live mainnet ENS lookup** — 5 records resolved correctly via PublicNode RPC.
4. **Capsule structural verify** — golden capsule passes, all relevant fields surfaced.
5. **Capsule explain** — readable JSON-ish summary suitable for judges.
6. **All 11 marketing routes** — proper titles, h1s, no template leakage, no placeholder text in user-visible content.
7. **WASM verifier asset** — 2.4MB binary serves with valid WASM magic bytes; loader script resolves correctly.
8. **Marketplace** — 5 starter policies enumerated and visible.
9. **Playground** — 8 scenarios enumerated.
10. **Try walkthrough** — 9 steps (8 scenarios + "Run it yourself" pointer).
11. **4 demo step pages** — all 200 with non-trivial content (11-20KB).
12. **Capsule mirror at GitHub Pages** — 3892 bytes, valid JSON, conforms to v1 schema.
13. **6 Sepolia contracts** — all deployed, bytecode > 0, queryable via `eth_getCode`.
14. **CCIP gateway long preview URL** — returns proper JSON error for malformed input (proves gateway code works; only the alias is wrong).

## What I couldn't test as a headless user

- **`/proof` interactive verification** — requires browser drag-drop into WASM verifier. WASM binary loads correctly (verified bytes); the actual verification flow needs browser hands-on by Daniel.
- **Trust DNS visualization** — `sbo3l-trust-dns-viz.vercel.app` returns 404 (separate Vercel project not deployed; documented gap).
- **Daemon end-to-end flow** — running `sbo3l-server` + POSTing real APRPs would require a running daemon + sponsor adapter env vars. Daemon is built into the CLI install but I didn't run a full daemon flow.
- **Python SDK + npm SDK install** — out of time budget; covered by previous CI smoke.
- **Hosted-app `/admin/*` routes** — Vercel deployment gated on Daniel.
- **Mobile app** — Expo skeleton; no live URL.

## Recommendations to Daniel

**Before submission:**
1. **Fix CCIP-Read alias (Bug #1):** Vercel dashboard → re-link `sbo3l-ccip.vercel.app` to the `apps/ccip-gateway/` project. ~5 min.
2. **Decide on Bug #2 (OffchainResolver):** either redeploy with corrected URL OR document the bug + workaround in the ENS bounty narrative. Redeploy is ~2h including ENS record update.
3. **Optionally: update `sbo3l:endpoint`** ENS text record (Bug #3) OR keep design-intent note in HANDOFF-FOR-DANIEL.md.

**After submission:**
4. Bug #5 (stale phase numbers in help): CLI source bump.
5. Bug #6 (`passport verify` doesn't run crypto): rename or add `--strict` flag.

## See also

- [`docs/submission/HANDOFF-FOR-DANIEL.md`](../submission/HANDOFF-FOR-DANIEL.md) — judge-facing handoff (updated with bug fixes)
- [`docs/submission/READY.md`](../submission/READY.md) — go/no-go signal
- [`docs/submission/live-url-inventory.md`](../submission/live-url-inventory.md) — every URL, smoke status
- [`docs/proof/competitive-benchmarks.md`](competitive-benchmarks.md) — perf comparison
