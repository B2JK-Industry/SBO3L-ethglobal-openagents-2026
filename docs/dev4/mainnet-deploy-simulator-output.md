# Mainnet deploy simulator — expected output

> **Audience:** Daniel (operator) + judges (truthfulness gate).
> **Outcome:** what `scripts/simulate-mainnet-deploy.sh` looks
> like end-to-end. Daniel reviews this BEFORE committing $190
> mainnet ETH on the real fleet deploy.
>
> **Companion to:** [`docs/dev4/mainnet-deploy-runbook.md`](mainnet-deploy-runbook.md).

## Why the simulator exists

The mainnet OR + 60-subname fleet runbook (R20 Task A) is
~$190 of irreversible mainnet gas. Daniel is reasonably hesitant
to drop that without seeing what would happen first.

The simulator forks mainnet at the current block via anvil, runs
ALL three deploy steps locally (zero real ETH), and reports
per-step gas + final on-chain state. If the simulator's
spot-checks all pass, Daniel has high confidence the real
broadcast will succeed.

## How to run

```bash
export MAINNET_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/<key>
./scripts/simulate-mainnet-deploy.sh
```

Runtime: ~3-5 minutes (anvil boot + 62 txes on the fork + state
read-back). Outputs to stdout + per-step logs in
`/tmp/sbo3l-simulator-output/`.

## Expected output (clean run)

```
===================================================================
  SBO3L mainnet deploy SIMULATOR — 2026-05-03T09:00:00Z
  upstream RPC:  https://eth-mainnet.g.alchemy.com/v2/<redacted>
  fork port:     8545
  output dir:    /tmp/sbo3l-simulator-output
===================================================================

[1/4] Booting anvil mainnet fork...

[2/4] Pre-flight: confirm sbo3lagent.eth is owned + reachable
  parent node:         0x2e3bac2fc8b574ba1db508588f06102b98554282722141f568960bb66ec12713
  parent owner:        0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231
  prior resolver:      0xF29100983E058B709F3D539b0c765937B804AC15

  impersonating 0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231 on the anvil fork...

[3/4] Simulating deploy steps

  STEP 1: forge create OffchainResolver
  ✅ deployed OR at 0x<NEW-OR-ADDRESS>
  STEP 1 gas estimate: 1500000 (~ $0.30 simulator-side; ~$5.00 real-mainnet @ 50 gwei)

  STEP 2: cast send setResolver(sbo3lagent.eth, 0x<NEW-OR-ADDRESS>)
  ✅ resolver flipped to 0x<NEW-OR-ADDRESS>
  STEP 2 gas estimate: 80000 (~ $0.02 simulator-side; ~$3.00 real-mainnet @ 50 gwei)

  STEP 3: forge script RegisterMainnetFleet (60 subnames)
  STEP 3 gas estimate: 3000000 (3M aggregate, 60×50K) (~ $0.60 simulator-side; ~$180.00 real-mainnet @ 50 gwei)

[4/4] Final state spot-check (5 subnames)
  ✅ research.sbo3lagent.eth          resolver = 0x<NEW-OR-ADDRESS>
  ✅ trader.sbo3lagent.eth            resolver = 0x<NEW-OR-ADDRESS>
  ✅ auditor.sbo3lagent.eth           resolver = 0x<NEW-OR-ADDRESS>
  ✅ agent-001.sbo3lagent.eth         resolver = 0x<NEW-OR-ADDRESS>
  ✅ agent-050.sbo3lagent.eth         resolver = 0x<NEW-OR-ADDRESS>

===================================================================
  SIMULATION RESULT
===================================================================
  STEP 1 OR deploy:           1500000 gas
  STEP 2 setResolver:          80000 gas
  STEP 3 60x setSubnodeRecord: 3000000 gas
  ----------------------------------------------------------------
  TOTAL:                       4580000 gas
  TOTAL +20% headroom:         5496000 gas

  At 50 gwei + ETH=$4000:
    Total: $1099.20

  Final on-chain state:
    sbo3lagent.eth resolver:     0x<NEW-OR-ADDRESS>
    research.sbo3lagent.eth:     0x<NEW-OR-ADDRESS>
    agent-001.sbo3lagent.eth:    0x<NEW-OR-ADDRESS>

  ✅ ALL SPOT-CHECKS PASSED — broadcast safely
===================================================================
```

(The `$1099.20` headline is at 50 gwei + ETH=$4000; current
realistic mainnet gas is closer to 5-15 gwei, putting actual
cost at ~$110-330 not $1099. Simulator is intentionally
conservative on the price assumption — better to budget
high.)

## What the simulator proves

| Check | Pass criterion |
|---|---|
| anvil fork at current block reachable | `cast chain-id` returns `1` after fork boot |
| sbo3lagent.eth owner = `0xdc7EFA…D231` | Pre-flight read confirms before impersonating |
| `forge create OffchainResolver` succeeds | Bytecode at returned address is non-empty |
| `setResolver` lands | `ENS.resolver(node)` returns the new OR address |
| All 60 setSubnodeRecord calls land | 5/5 spot-checked subnames resolve to the new OR |

If any of these fail, the simulator exits non-zero and Daniel
knows to investigate `/tmp/sbo3l-simulator-output/` logs before
touching real mainnet.

## What the simulator does NOT prove

- **Daniel's actual wallet has gas headroom.** The simulator
  uses anvil's pre-funded test accounts, not Daniel's
  `0xdc7EFA…D231`. Wallet balance check lives in
  [`mainnet-deploy-runbook.md`](mainnet-deploy-runbook.md)
  STEP 0.
- **Cloudflare WAF / Alchemy rate limits during real broadcast.**
  Real mainnet has different operational constraints than an
  anvil fork.
- **Gateway records.json is updated.** The simulator focuses on
  on-chain side; the gateway-side pre-population happens via
  `apps/ccip-gateway/scripts/seed-fleet-records.mjs` (R20 Task A)
  + Vercel deploy.
- **Mempool front-running risk.** A malicious mempool actor
  could front-run the `setSubnodeRecord` calls if they noticed
  the parent owner posting them. The runbook's mitigation: post
  the 60 in a single batch + use a higher-priority gas price.
- **Reproducibility against future blocks.** The simulator forks
  *current* mainnet state; running it tomorrow against a new
  block could produce different gas estimates if the underlying
  ENS contracts change (rare but possible during ENS protocol
  upgrades).

## When to re-run the simulator

- Right before broadcasting the real deploy (within 24h is ideal).
- After any change to `RegisterMainnetFleet.s.sol` or the
  60-label list.
- After any change to the OR contract source (so the deploy
  bytecode matches what the simulator built).
- If the gas-price environment shifts dramatically (the cost
  estimates need re-validation).

## Failure modes + recovery

| Symptom in simulator | Real-mainnet impact | Recovery |
|---|---|---|
| `anvil` won't fork (network error) | None (simulator can't run) | Check `MAINNET_RPC_URL` is reachable + has fork capability |
| Pre-flight: parent owner ≠ Daniel's wallet | Real broadcast would revert at STEP 2 setResolver | Daniel doesn't own the parent — wrong wallet OR ownership transferred away |
| STEP 1 deploy fails | Real broadcast might revert too | Inspect `/tmp/sbo3l-simulator-output/step1-deploy-or.log`; common cause: gateway signer env mismatch |
| STEP 2 setResolver fails (parent owner mismatch) | Real broadcast would revert | Same as pre-flight failure above |
| STEP 3 some subnames fail | Real broadcast partial-state | Check `step3-fleet.log` for the specific failing label; may be a duplicate / already-existing subname |
| Spot-check resolver mismatch | Subname won't resolve to OR via CCIP-Read | Re-run the failing subname's setSubnodeRecord with correct args |

## Cross-reference

- [`scripts/simulate-mainnet-deploy.sh`](../../scripts/simulate-mainnet-deploy.sh) — the simulator script
- [`docs/dev4/mainnet-deploy-runbook.md`](mainnet-deploy-runbook.md) — the real-mainnet runbook
- [`crates/sbo3l-identity/contracts/script/DeployOffchainResolver.s.sol`](../../crates/sbo3l-identity/contracts/script/DeployOffchainResolver.s.sol) — STEP 1 underlying script
- [`crates/sbo3l-identity/contracts/script/RegisterMainnetFleet.s.sol`](../../crates/sbo3l-identity/contracts/script/RegisterMainnetFleet.s.sol) — STEP 3 underlying script
