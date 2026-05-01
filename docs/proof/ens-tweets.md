# SBO3L × ENS — submission-day tweet thread

**For:** Daniel to post on submission day from his X account.
**Length:** 5 tweets, ≤ 280 chars each (counted incl. URLs).
**Tone:** technical, concrete, no hype words. Each tweet stands alone
*and* threads cleanly into the next.

---

## Tweet 1 — the hook

> ENS is usually treated as agent IDENTITY.
>
> SBO3L treats ENS as agent COMMITMENT.
>
> Two AI agents authenticate each other with ZERO out-of-band setup —
> ENS is the only thing they share.
>
> Code, manifests, live records: github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026
>
> 🧵👇

(257 chars — fits.)

---

## Tweet 2 — the cross-agent protocol

> How it works:
>
> Agent A signs a challenge containing its audit-chain head + nonce.
> Agent B reads A's sbo3l:pubkey_ed25519 via getEnsText, verifies the
> Ed25519 signature, emits a CrossAgentTrust receipt.
>
> No CA, no session, no enrolment.
>
> 13 tests, 0 mocks.

(256 chars — fits.)

---

## Tweet 3 — the policy commitment

> The "Most Creative" angle:
>
> sbo3l:policy_hash on ENS = JCS+SHA-256 of the agent's active policy.
>
> A judge holding the ENS record + the daemon's /v1/policy endpoint
> can re-hash independently. Policy as cryptographic commitment, not
> docs-page string.

(258 chars — fits.)

---

## Tweet 4 — the constellation

> The scale proof:
>
> 60-agent fleet under sbo3lagent.eth. 6 capability classes. Every
> keypair deterministically re-derivable from a public seed-doc via
> SHA-256.
>
> Trust-DNS viz at apps/trust-dns-viz animates the constellation in
> 3s on demo load.

(263 chars — fits.)

---

## Tweet 5 — the off-chain extension

> One more: ENSIP-25 / EIP-3668 CCIP-Read gateway at
> sbo3l-ccip.vercel.app. Reputation + capabilities update without
> per-agent setText gas.
>
> viem.getEnsText resolves transparently. No SBO3L-specific client
> code.
>
> Targeting both ENS tracks. End of thread.

(269 chars — fits.)

---

## Posting notes for Daniel

- **Best window:** ~10 min after the ETHGlobal "submissions open"
  tweet. Catches the ENS-judge attention spike.
- **Tag:** `@ensdomains` on Tweet 1; `@dhaiwat10` (Dhaiwat) and
  `@signalwerk` (sometimes Simon ses.eth) on Tweet 2.
- **Pin:** Tweet 1 to your profile until the bounty closes.
- **Reply with image:** drop a screenshot of `viem.getEnsText`
  resolving `research-agent.sbo3lagent.eth` → `sbo3l:reputation`
  under Tweet 5 once the Vercel deploy is live. Browser console
  screenshot is fine; no design polish needed.
- **Engagement bait that's also true:** if a reply asks "isn't this
  just ENSIP-25?", answer with: "ENSIP-25 is the *pointer* spec;
  this is what we *put under the pointer* — a JCS-canonical policy
  commitment + an Ed25519 pubkey + an audit-chain head. The pointer
  layer is ENSIP-25 compliant by design." Link the cross-agent doc
  for the deeper "why".

## Backup tweet (for if a thread loses traction)

> Quick demo:
>
> ```bash
> SBO3L_ENS_RPC_URL=https://ethereum-rpc.publicnode.com \
> sbo3l agent verify-ens sbo3lagent.eth --network mainnet
> ```
>
> Resolves all 5 sbo3l:* records on mainnet today. <5s. Run it
> yourself before believing me.

(259 chars — fits. Use as quote-RT of Tweet 1 if engagement is
flat 30 min in.)

## See also

- `docs/proof/ens-narrative.md` — full 1500-word judges narrative.
- `docs/proof/ens-pitch.md` — 300-word DM to Dhaiwat / Simon ses.eth.
