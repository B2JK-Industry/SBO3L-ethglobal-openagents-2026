# UAT Round 1.5 — continuous per-R18-PR verification

> **Filed by:** Heidi (QA + Release agent).
> **Started:** 2026-05-03 (extends Round 1 from 2026-05-02; UAT-1 from earlier same evening).
> **Mode:** focused UAT against each R18 PR's new surface within 1 hour of merge. One commit per PR verified.

## Scope

For each R18 PR landing on main, run the targeted check from Daniel's R18 brief, append a row to the table below.

## Per-PR verification log

| PR | Title | Verified at | Result | Evidence |
|---|---|---|---|---|
| _(populated as R18 PRs land)_ | | | | |

## R18 expected PR map (from Daniel's brief)

| Owner | PR | Verification action |
|---|---|---|
| Dev 1 PR1 | Sepolia OR redeploy | `cast call <new-addr> 'urls(uint256)(string)' 0` — must return `https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json` exactly. `sbo3l doctor --extended` PASS on new addr. |
| Dev 1 PR2 | P-marker cleanup | `grep -E "P[0-9]+\.[0-9]+"` on `sbo3l --help` + `passport --help` + `passport run --help` + `passport verify --help` → 0 matches |
| Dev 1 PR3 | 0G Storage backend | `sbo3l audit export --backend 0g-storage --dry-run` produces valid envelope; live mode (`ZEROG_TESTNET_LIVE=1`) succeeds OR fails with browser-upload-fallback message |
| Dev 2 PR1 | LangChain KH demo | `pip install` + smoke exit 0 |
| Dev 2 PR2 | 5 KH issue URLs | All 5 GitHub issue pages HTTP 200 |
| Dev 2 PR3 | 0G uploader UI | `/demo/<route>` accepts drag-drop |
| Dev 3 PR1 | Trust DNS Manifesto | `wc -w` ≥ 4500 |
| Dev 3 PR2 | `/kh-fleet` | HTTP 200 + counter visible + ≥ 1 row |
| Dev 3 PR3 | 0G one-pager | HTTP 200 if mounted as `/submission/0g` |
| Dev 4 PR1 | ENSIP upstream | URL in `docs/proof/ensip-upstream-pr.md` reachable + shows the PR |
| Dev 4 PR2 | Universal Router PR | Same — URL in proof doc reachable |
| Dev 4 PR3 | 0G AuditAnchor | `cast getCode <addr>` on 0G Galileo RPC non-empty + `publishAnchor` with dummy hash emits event |
| Dev 4 PR4 | KH-A5 | PR URL reachable |

## Cascade snapshot at session start

Open R18 PRs at 2026-05-03 ~00:10 UTC:

- #383 [agent/dev4/sepolia-or-redeploy-task-a] — fix(deploy): Task A — pin canonical URL template + probe tests
- #384 [agent/dev1/doctor-extended] — feat(cli): sbo3l doctor --extended — Sepolia contract liveness probes

Both auto-merge armed. Will verify post-merge.

## Per-PR verification entries

### Batch 1 — verified 2026-05-03 ~00:50 UTC

Five R18 PRs landed in the same 80-second window:

#### #383 — Dev 4 PR1 / Task A — Sepolia OR redeploy prep ✅ PASS

**Verification:**
- ✅ `CANONICAL_URL_TEMPLATE` constant pinned in `crates/sbo3l-identity/contracts/script/DeployOffchainResolver.s.sol`
- ✅ Probe tests in `contracts/test/DeployOffchainResolver.t.sol` reference the constant (`new_or_url_template_is_canonical` regression guard)

This PR is Task A of a three-PR sequence (A/B/C). Task B = #390 (deploy + register).

#### #390 — Dev 4 Task B — Sepolia OR LIVE + sbo3lagent.eth Sepolia + research-agent ✅ PASS — **Bug #2 FIXED on chain**

**Verification (live Sepolia reads):**
- ✅ New OR deployed at `0x87e99508c222c6e419734cacbb6781b8d282b1f6` (4746 bytecode chars onchain)
- ✅ `cast call ... 'urls(uint256)(string)' 0` returns **`https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json`** — canonical form, not the old `{sender/{data}.json}` malformed shape
- ✅ Sepolia apex registration tx `0x655f2b78…1238783` confirmed (status=0x1, block 10777914, to=ETH Registrar Controller)
- ✅ Subname `setSubnodeRecord` tx `0x71c7fd7b…95db1` confirmed (status=0x1, block 10777918, to=ENS Registry)

**Note:** the PR body has a typo in the new OR address — it shows `0x87e99508Ad7DdaBcdf67C50ad5cFC18906bDb1f6` (capital letters in middle) but the actual deploy script + onchain reality is `0x87e99508c222c6e419734cacbb6781b8d282b1f6`. The deploy + onchain state are correct; only the PR body display is wrong. Filing minor doc-fix follow-up.

**Bug status:** Heidi's UAT-1 Bug #2 (malformed gateway URL onchain) is now **FIXED on Sepolia**. The mainnet apex `sbo3lagent.eth` still points at the OLD OR (`0x7c69…A8c3`); pinning the new addr in `contracts.rs` + updating mainnet ENS record is Task C (pending).

#### #386 — Dev 1 PR2 — P-marker cleanup ✅ PASS — **Bug #5 FIXED**

**Verification (CLI built from main HEAD with #386 + cargo build --release):**

| Help text | P-marker count | Required |
|---|---|---|
| `sbo3l --help` | 0 | 0 |
| `sbo3l passport --help` | 0 | 0 |
| `sbo3l passport run --help` | 0 | 0 |
| `sbo3l passport verify --help` | 0 | 0 |

All four help texts free of `P[0-9]+\.[0-9]+` markers. Heidi's UAT-1 Bug #5 fully closed.

#### #384 — Dev 1 — `sbo3l doctor --extended` ✅ PASS

**Verification:**
- ✅ Subcommand exists and runs (`sbo3l doctor --extended`)
- ✅ Default-config probes the OLD OR (`0x7c69…A8c3`) and **CORRECTLY flags it as FAIL** with the explicit message: `URL template missing {sender} or {data} — Heidi's Bug #2 shape`
- ✅ Other Sepolia contract probes (AnchorRegistry, etc.) report `ok` with bytecode + read-result
- ✅ Truthfulness note: `skip means the feature is not yet implemented in this build, NOT that it silently passed`

**Note:** Once the contracts.rs pin updates to the new OR (Task C), `doctor --extended` will flip to PASS overall.

#### #392 — Dev 3 PR2 — `/kh-fleet` ✅ PASS

**Verification:**
- ✅ HTTP 200 (20006 bytes)
- ✅ Counter visible: `<h1>Live KeeperHub executions through SBO3L</h1>` + headline "20 executions"
- ✅ Row signal: `<h2>Recent 20 executions</h2>` confirms ≥1 row in list

All three brief requirements met.

### Batch 1 verdict — all 5 PASS, 2 UAT-1 bugs closed (#2, #5)

R18 batch 1 lands a major chunk of Heidi's UAT-1 follow-ups:
- Bug #2 fixed on Sepolia (#390 Task B; mainnet update pending Task C)
- Bug #5 fixed (#386)
- Plus operational upgrades (#384 doctor extension; #392 live KH dashboard) and deploy-script regression guard (#383)

Task C (contracts.rs pin + mainnet ENS update + doc memory updates) is the remaining piece for full Bug #2 closure.

### Open R18 PRs at batch 1 close

- #393 [agent/dev3/zerog-onepager] — 0G partner one-pager (Dev 3 PR3)
- #394 [agent/dev1/uniswap-swap-cli] — sbo3l uniswap swap CLI (Dev 1 extra)
- #395 [agent/dev2/langchain-kh-demo] — LangChain KH demo (Dev 2 PR1)

Batch 2 verification fires when these land.

