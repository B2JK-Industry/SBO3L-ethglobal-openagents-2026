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

(One section appended per landed PR.)
