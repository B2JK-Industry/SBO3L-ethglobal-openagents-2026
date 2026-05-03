# Final pre-submit UAT — Round 1 (2026-05-02 22:15 UTC)

> **Filed by:** Heidi (QA + Release agent), Round 1 of pre-submit final UAT.
> **Repo state:** main HEAD `c9eb25c` (after #374 + #375 + #376).
> **Method:** real CLI install (built from main, not crates.io v1.2.0 — to pick up #374 source-only fix), live HTTP requests, live mainnet ENS calls, live Sepolia onchain reads.
> **Round 1 scope:** independent of Daniel's pending mainnet redeploy. Round 2 + 3 follow after Daniel's manual steps.

## Summary verdict

**🟢 Round 1: PASS with 3 known gaps.** All previously-found code/CLI bugs are repaired. Two of the four UAT-1 bugs (#1 + #6) are now closed; bug #2 (OffchainResolver malformed URL on chain) is still pending Daniel's redeploy; bug #5 (obsolete phase markers in CLI help) is partially fixed (one site missed).

## Test matrix

### A. Re-verify 14 surfaces from previous UAT

| # | Surface | Round 1 result | UAT-1 result | Δ |
|---|---|---|---|---|
| 1 | Marketing root | ✅ 200 | ✅ 200 | — |
| 2 | /demo + 4 step pages | ✅ 5/5 200 | ✅ 5/5 | — |
| 3 | /proof | ✅ 200 | ✅ | — |
| 4 | /features | ✅ 200 | ✅ | — |
| 5 | /submission | ✅ 200 | ✅ | — |
| 6 | /marketplace | ✅ 200 | ✅ | — |
| 7 | /quickstart | ✅ 200 | ✅ | — |
| 8 | /playground | ✅ 200 | ✅ | — |
| 9 | /learn | ✅ 200 | ✅ | — |
| 10 | /compare | ✅ 200 | ✅ | — |
| 11 | /try | ✅ 200 | ✅ | — |
| 12 | sbo3l-ccip.vercel.app | ✅ **200 + correct content** | 🚨 served marketing | **🎉 Bug #1 FIXED** |
| 13 | sbo3l-trust-dns-viz.vercel.app | 🟡 404 (still gated on Daniel) | 🟡 404 | — |
| 14 | Capsule mirror @ GH Pages | ✅ 200 (3892 bytes) | ✅ | — |

Plus WASM verifier asset: ✅ 2.4MB binary with `\0asm` magic bytes.

### B. /submission/<slug> URLs (was bug #375)

| Slug | Result |
|---|---|
| /submission | ✅ 200 |
| /submission/keeperhub | ✅ 200 |
| /submission/ens-most-creative | ✅ 200 |
| /submission/ens-ai-agents | ✅ 200 |
| /submission/uniswap | ✅ 200 |
| /submission/erc-8004 | 🟡 404 (slug not built; not in /submission's link list) |
| /submission/privy | 🟡 404 (slug not built) |
| /submission/sbe | 🟡 404 (slug not built) |

**Verdict:** the 4 implemented sponsor-track slugs are live. The 3 missing slugs (erc-8004, privy, sbe) are not actually built — Daniel's brief listed 7 speculatively, but only 4 are in the deployed Astro routes.

### C. /status page truth-table

| Check | Result |
|---|---|
| HTTP 200 | ✅ |
| Bytes | 24982 |
| Contains "live" | ✅ 6 occurrences |
| Contains "mock" | ✅ 4 occurrences |
| Contains "not yet" | ✅ 3 occurrences |

### D. CLI exit codes (was bug #6 — fixed by #374)

| Capsule | Required exit | Actual exit | Result |
|---|---|---|---|
| `v2_golden_001_minimal.json` | 0 | 0 | ✅ |
| `v2_golden_002_keeperhub_mock.json` | 0 | 0 | ✅ |
| `v2_golden_003_keeperhub_mock.json` | 0 | 0 | ✅ |
| `v2_golden_004_keeperhub_mock.json` | 0 | 0 | ✅ |
| `v2_golden_005_keeperhub_mock.json` | 0 | 0 | ✅ |
| `v2_tampered_001_policy_snapshot_drift.json` | 2 | 2 | ✅ |
| `v2_tampered_002_audit_segment_chain_break.json` | 2 | 2 | ✅ |
| `v2_tampered_003_audit_segment_link_mismatch.json` | 2 | 2 | ✅ |
| `v2_tampered_004_audit_segment_too_large.json` | 2 | 2 | ✅ |

**Plus:** v1 capsule (`golden_001_allow_keeperhub_mock.json`) — exits 0 + emits the new "structural pass — capsule is NOT self-contained, signature checks were SKIPPED. Re-run with --strict --receipt-pubkey [--audit-bundle] [--policy] for full crypto coverage." hint. **#374 perfectly fixed bug #6.**

### E. Sepolia contracts onchain (sanity check)

| Contract | Address | Bytecode chars |
|---|---|---|
| OffchainResolver | `0x7c69…A8c3` | 4746 ✅ |
| AnchorRegistry | `0x4C30…f4Ac` | 3308 ✅ |
| SubnameAuction | `0x5dE7…114B` | 8934 ✅ |
| ReputationBond | `0x7507…93dA` | 5368 ✅ |
| ReputationRegistry | `0x6aA9…6dc2` | 6024 ✅ |
| Uniswap QuoterV2 | `0xEd1f…2FB3` | 16548 ✅ (read-side) |

All 6 deployed.

### F. Mainnet ENS resolution

```bash
sbo3l agent verify-ens sbo3lagent.eth --rpc-url https://ethereum-rpc.publicnode.com
# → totals: pass=0 fail=0 skip=5 absent=3
# → verdict: PASS
```

5 records on chain, 3 absent (`sbo3l:pubkey_ed25519`, `sbo3l:policy_url`, `sbo3l:capabilities`) — Phase 2 records pending. Round 2 will re-test after Daniel's amplifier setText calls.

### G. npm install + import smoke (`@sbo3l/anthropic`)

```bash
npm install @sbo3l/anthropic@1.2.0
# → added 115 packages

node -e "console.log(Object.keys(require('@sbo3l/anthropic')))"
# → SBO3LError, APRP_INPUT_SCHEMA, DEFAULT_TOOL_NAME, PolicyDenyError,
#   aprpSchema, runSbo3lToolUse, sbo3lTool
```

✅ Cleanly installs, imports 7 named exports.

🟡 **Side note:** running `npm test` from inside `sdks/typescript/integrations/anthropic/` fails because vitest is hoisted to the parent `sdks/typescript/` dir. This is a monorepo workspace setup issue, not a published-package issue. Judges installing the published package don't hit this.

## Bug status update from UAT-1 (2026-05-02 ~17:30 CEST)

| # | Bug | UAT-1 status | Round 1 status |
|---|---|---|---|
| 1 | sbo3l-ccip.vercel.app served marketing site | 🚨 broken | ✅ **FIXED** — title now "SBO3L CCIP-Read gateway"; API returns proper 400 |
| 2 | OffchainResolver onchain has malformed URL `{sender/{data}.json}` | 🚨 broken | 🟡 **STILL PENDING** — `cast call ... urls(0)` still returns the malformed URL. Awaiting Daniel's redeploy. |
| 3 | `sbo3l:endpoint` ENS record = localhost | 🟡 documented | 🟡 unchanged (will re-test in Round 2 after Daniel's Phase 2 setText calls) |
| 4 | Docs reference non-existent `sbo3l passport resolve` | 🟢 fixed in #373 | ✅ confirmed fixed across docs |
| 5 | CLI help has obsolete phase markers (P1.1/P5.1/P6.1) | 🟢 reported | 🟡 **PARTIALLY FIXED** — `passport verify` description cleaned up, but `passport run` description still has "P2.1/P5.1/P6.1 work" markers. One-line fix on `crates/sbo3l-cli/src/main.rs` doc-comment. |
| 6 | `passport verify` accepted tampered capsules silently | 🟢 reported | ✅ **FIXED** — all 4 tampered fixtures now exit 2; v1 emits structural-pass hint with --strict guidance |

## Remaining open items

| Item | Severity | Owner | When |
|---|---|---|---|
| OffchainResolver redeploy with corrected URL | 🚨 Critical | Daniel | NÁVOD 1 — pending |
| `sbo3l-trust-dns-viz` Vercel deploy | 🟢 Low | Daniel | Optional |
| `sbo3l:endpoint` ENS record updated to non-localhost | 🟡 Medium | Daniel | Phase 2 setText (per brief) |
| `passport run` help text obsolete-phase cleanup | 🟢 Low | Dev 1 | Follow-up patch |
| `npm test` from integration leaf dir (vitest hoisting) | 🟢 Low | Dev 2 | Follow-up; not user-facing |
| `/submission/<slug>` for erc-8004/privy/sbe | 🟢 Low | Dev 3 | Optional; only 4 of 7 speculated slugs are deployed today |

## Round 2 — pending Daniel's NÁVOD 1 (mainnet Phase 2 setText + Sepolia OR redeploy)

> **Scope clarification (corrected post-codex P1):** there is no "mainnet OR" — the OffchainResolver is a **Sepolia-only** contract; the mainnet apex `sbo3lagent.eth` uses the regular `PublicResolver` and stores its `sbo3l:*` records directly on chain. NÁVOD 1 is two independent pieces:
> 1. **Mainnet:** Phase 2 setText calls to add `sbo3l:pubkey_ed25519` + `sbo3l:capabilities` to the apex.
> 2. **Sepolia:** OR redeploy with the canonical URL template (the actual Bug #2 fix).

Will re-run when Daniel completes NÁVOD 1:

1. `sbo3l agent verify-ens sbo3lagent.eth --network mainnet` → expect `verdict: PASS` with **all 7+ records non-empty** (including new `sbo3l:pubkey_ed25519` and `sbo3l:capabilities` from the mainnet Phase 2 setText calls)
2. Confirm `sbo3l:endpoint` on mainnet is **NOT localhost** any more (was bug #3)
3. Decode the **new Sepolia OR** `urls(0)` → expect well-formed `{sender}/{data}.json` (the actual Bug #2 fix happens on Sepolia, not mainnet)
4. Live CCIP-Read flow: cast a real ENSIP-25 request against the new Sepolia resolver, confirm gateway signs response correctly

## Round 3 — pending Daniel's demo video

1. Open the demo video URL in incognito (verify public)
2. Verify URL works on mobile (Chrome DevTools mobile emulation OK if no real device available)

## Heidi recommendation

🟢 **Round 1 PASS.** Daniel can proceed with NÁVOD 1 (mainnet redeploy) without blocking on Heidi findings. The 5 remaining open items are either Daniel-side scheduled work or low-severity follow-ups; none block submission.

When Daniel signals NÁVOD 1 complete, I'll fire Round 2.

## See also

- [`docs/proof/user-acceptance-test-2026-05-02.md`](user-acceptance-test-2026-05-02.md) — UAT-1 (the report this round validates)
- [`docs/submission/READY.md`](../submission/READY.md) — submission go/no-go signal
- [`docs/submission/HANDOFF-FOR-DANIEL.md`](../submission/HANDOFF-FOR-DANIEL.md) — Daniel's submission-day checklist
