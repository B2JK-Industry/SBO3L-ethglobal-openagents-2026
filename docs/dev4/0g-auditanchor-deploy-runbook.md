# 0G AuditAnchor deploy on Galileo — Daniel runbook (browser-friendly)

> **Audience:** Daniel.
> **Outcome:** `Sbo3lAuditAnchor` deployed live on 0G Galileo
> testnet. Captured address pinned in `contracts.rs` +
> bounty-narrative flip from "source ready" → "live."
> **Cost:** 0 (testnet) + browser-side faucet click.
> **Time:** <10 min total (5 min browser + 3 min terminal + 2 min
> me running the follow-up PR).
>
> **Companion to:** [`r19-task-c-0g-audit-anchor-deploy.md`](r19-task-c-0g-audit-anchor-deploy.md)
> (the original deploy-blocked runbook). This doc is the
> **screenshot-friendly browser-step variant** Daniel asked for in
> R20 Task C.

---

## What's already in main from R19

- Contract source: [`crates/sbo3l-identity/contracts/Sbo3lAuditAnchor.sol`](../../crates/sbo3l-identity/contracts/Sbo3lAuditAnchor.sol)
- Forge tests: 11/11 pass at `forge test --match-contract Sbo3lAuditAnchor`
- Deploy script: [`crates/sbo3l-identity/contracts/script/DeployAuditAnchor0G.s.sol`](../../crates/sbo3l-identity/contracts/script/DeployAuditAnchor0G.s.sol)

Everything is build-clean + tested. Only thing missing is the
browser-side faucet drip.

---

## STEP 1 — Faucet 10 OG to driver wallet (browser only)

The 0G hackathon faucet uses Cloudflare Turnstile (anti-bot).
You need a browser to solve the challenge. ~3 min.

### Option A — Faucet the existing driver wallet `0x50BA…7e9c`

1. **Open the faucet** in any browser:
   ```
   https://0g-faucet-hackathon.vercel.app/
   ```

2. **Clear localStorage** (the workaround — Kunal Shah,
   2026-05-02):
   - Open DevTools (Cmd+Opt+I on Mac, F12 on Win/Linux).
   - Go to the **Application** tab.
   - In the left sidebar: **Storage** → **Local Storage** →
     `https://0g-faucet-hackathon.vercel.app`.
   - Right-click → **Clear**. (Or click each entry → Delete.)
   - Close DevTools.

3. **Refresh the page** (Cmd+R / F5).

4. **Paste the driver wallet address**:
   ```
   0x50BA7BF5FDe124DB51777A2bF0eED733756B7e9c
   ```

5. **Solve the Cloudflare Turnstile challenge** (the "I'm not a
   robot" widget). Usually 1-2 clicks; sometimes asks for an
   image puzzle.

6. **Enter promo code**:
   ```
   OPEN-AGENT
   ```

7. **Click Submit / Claim** → drip lands at **10 OG**.

8. Verify the drip on chain:
   ```bash
   cast balance 0x50BA7BF5FDe124DB51777A2bF0eED733756B7e9c \
     --rpc-url https://evmrpc-testnet.0g.ai
   # expect: 10000000000000000000  (10 OG, 18 decimals)
   ```

If the faucet rejects with "wallet already claimed":
- Ensure localStorage was actually cleared (some browsers cache).
- Try Option B (fresh wallet) below.

### Option B — Fresh wallet (cleaner)

```bash
cast wallet new
```

Save the address + PK. In the faucet UI, paste the new address
instead of the driver wallet. Clear localStorage step is the
same. After drip lands:

```bash
export PRIVATE_KEY=0x<fresh-wallet-PK-from-cast-wallet-new>
```

(Use this in STEP 2 instead of the driver wallet PK.)

---

## STEP 2 — Deploy Sbo3lAuditAnchor (terminal, single command)

```bash
# Driver-wallet PK is in your local
# `~/.claude/projects/.../memory/critical_credentials.md`
# under "Driver-spawned deploy wallet". Do NOT paste it into the
# repo — gitleaks will flag.
export PRIVATE_KEY=0x<driver-wallet-PK-from-memory>
# OR the fresh-wallet PK from STEP 1 Option B

cd /Users/danielbabjak/Desktop/MandateETHGlobal/mandate-ethglobal-openagents-2026/crates/sbo3l-identity/contracts

forge script script/DeployAuditAnchor0G.s.sol \
  --rpc-url https://evmrpc-testnet.0g.ai \
  --broadcast
```

Expected output (last 5 lines):

```
Sbo3lAuditAnchor deployed to: 0x...
chain id: 16602

ONCHAIN EXECUTION COMPLETE & SUCCESSFUL.

Transactions saved to: .../broadcast/DeployAuditAnchor0G.s.sol/16602/run-latest.json
```

**Save the deployed address** into shell:

```bash
export DEPLOYED_AUDIT_ANCHOR=0x...   # from script output
```

---

## STEP 3 — Verify on chain

```bash
RPC=https://evmrpc-testnet.0g.ai

# Bytecode landed
cast code "$DEPLOYED_AUDIT_ANCHOR" --rpc-url "$RPC" | wc -c
# expect: > 1000 (non-trivial bytecode)

# Sample state read — anchorTimestamp(zero) returns 0 for never-anchored.
cast call "$DEPLOYED_AUDIT_ANCHOR" "anchorTimestamp(bytes32)(uint256)" \
  0x0000000000000000000000000000000000000000000000000000000000000000 \
  --rpc-url "$RPC"
# expect: 0

# Optional: publishAnchor a test hash + verify timestamp pins
cast send "$DEPLOYED_AUDIT_ANCHOR" "publishAnchor(bytes32)" \
  0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef \
  --rpc-url "$RPC" --private-key "$PRIVATE_KEY"

cast call "$DEPLOYED_AUDIT_ANCHOR" "getAnchor(bytes32)(uint256)" \
  0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef \
  --rpc-url "$RPC"
# expect: a Unix timestamp (block.timestamp at the publishAnchor tx)
```

Open `https://chainscan-galileo.0g.ai/address/$DEPLOYED_AUDIT_ANCHOR`
in a browser to see the contract on the 0G explorer.

---

## STEP 4 — Hand off to me (Daniel-side done)

Paste the deployed address into our chat. I'll run a 5-min
follow-up PR doing all of:

1. **Pin** in `crates/sbo3l-identity/src/contracts.rs`:
   ```rust
   pub const ZEROG_AUDIT_ANCHOR_GALILEO: ContractPin = ContractPin {
       address: "0x...",
       network: Network::ZeroGGalileo,  // adds new variant if not present
       label: "SBO3L AuditAnchor (0G Galileo testnet)",
       canonical_source: "https://chainscan-galileo.0g.ai/address/0x...",
   };
   ```
2. **Add to `all_pins()`** — covered by existing
   `every_pin_is_canonical_form` test.
3. **Update memory** `zerog_bounty_intel.md` with deployed
   address + tx hash + chainscan URL.
4. **Update bounty narrative** — flip the 0G AuditAnchor row
   from "source ready, deploy gated" to "LIVE on Galileo at
   `0x…`."
5. **Update `/status` truth-table** — flip the same row.
6. **Update `docs/proof/etherscan-link-pack.md`** — add a "0G
   Galileo" section with the address.

Total < 5 min Dev 4 work, single bundled PR, judge-clickable
evidence flips from "structurally complete, deploy gated" to
"live on Galileo."

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Faucet returns `{"error":"invalid wallet address"}` | Cloudflare Turnstile token failed | Solve the challenge again; ensure JS is enabled |
| Faucet says "already claimed" but balance is 0 | localStorage not cleared properly | Hard-refresh + clear ALL site data (Application → Clear storage); or use fresh wallet (Option B) |
| `forge script` reverts with `chainid mismatch` | Wrong RPC URL | Verify `cast chain-id --rpc-url <url>` returns `16602` |
| `forge script` reverts with `insufficient funds` | Faucet drip didn't land | Re-check `cast balance`; Galileo block times can be 30-60s |
| `cast code` returns `0x` post-deploy | Tx mined but reverted | Look at `broadcast/DeployAuditAnchor0G.s.sol/16602/run-latest.json` for the actual tx hash + status |

---

## Why this matters (judge-grade impact)

- **0G Track A**: from "source ready" → "live on Galileo." 5th →
  3rd probability +40% per the prompt's grade-impact mapping.
- **Cross-track**: the live AuditAnchor address gives the
  Sbo3lAuditAnchor a real probe surface — Dev 1 Task C (doctor
  probe) gets a real address to validate against in CI.
- **Truthfulness**: every "we deployed an audit anchor on 0G"
  claim becomes resolvable from `chainscan-galileo.0g.ai`.

---

## What this PR DOESN'T do

- ❌ Bypass the Turnstile challenge programmatically. The faucet
  workaround is a *browser* trick (clear localStorage); the
  actual challenge still needs a human (or a paid Turnstile-
  bypass service we're not paying for).
- ❌ Pin `ZEROG_AUDIT_ANCHOR_GALILEO` in `contracts.rs`. That's
  STEP 4 — happens after you paste the address into chat.
- ❌ Update memory or bounty narrative. Same — STEP 4 follow-up.

The R19 source PR (#447) already shipped the contract + tests +
deploy script. This PR is purely the **runbook + screenshot
guide** the prompt asked for — paste-runnable wrapper around
existing infrastructure.
