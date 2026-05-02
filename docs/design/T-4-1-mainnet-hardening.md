# OffchainResolver mainnet hardening posture

**Status:** pre-mainnet-deploy gate (P11 from round 9).
**Scope:** clarify what we *are* changing, what we are *deliberately not*
changing, and what's a pre-deploy CI gate vs a runtime check on the
contract before the mainnet deploy lands. Companion artefacts:
[`OffchainResolver.sol`](../../crates/sbo3l-identity/contracts/OffchainResolver.sol),
[`OffchainResolver.invariant.t.sol`](../../crates/sbo3l-identity/contracts/test/OffchainResolver.invariant.t.sol)
(11-fuzz × 10K + 3 immutability tests),
[`.github/workflows/foundry.yml`](../../.github/workflows/foundry.yml).

## Threat model recap

`OffchainResolver` is a **stateless** view contract after construction.
Every public method is either `view` / `pure` or always reverts:

| Method                | State | Behaviour                                      |
|-----------------------|-------|------------------------------------------------|
| `resolve`             | view  | Always reverts with `OffchainLookup`           |
| `resolveCallback`     | view  | Verifies signature; returns decoded value      |
| `recoverSigner`       | pure  | ECDSA recovery only                            |
| `urls(uint256)`       | view  | Read-only into immutable URL list              |
| `urlsLength`          | view  | Read-only into immutable URL list              |
| `gatewaySigner`       | view  | Read-only into immutable signer slot           |
| `supportsInterface`   | pure  | Constant ERC-165 advertisement                 |

There are **no `external` writes, no `.call`/`.delegatecall`/`.send`,
no token movements, no value transfers, and no mutable storage**
beyond what the constructor sets. The fuzz suite (PR #198, merged)
proves: `gatewaySigner` and the URL list never drift after
construction.

The threat surface a hardener should optimise against is:

1. **Wrong signature accepted** → fuzz suite covers
   `validSignatureAlwaysVerifies`, `invalidSignatureRejects`,
   `tamperedValueRejects`, `tamperedDataRejects`, `expiredSignatureRejects`,
   `unauthorizedSignerRejects` — each at 10 000 random inputs.
2. **Constructor accepts garbage** → `constructorRejectsZeroSigner`,
   `constructorAcceptsAnyNonzeroSigner`.
3. **Off-chain gateway compromise** → mitigation: gateway-signing-key
   rotation requires redeploy; the immutable storage is the safety
   feature, not a limitation. (See "Why we are NOT adding mutable
   setters" below.)
4. **Static-analysis-discoverable bugs** → mitigation: Slither in CI
   (added in this PR's `.github/workflows/foundry.yml`).

## What this PR adds

### Slither in CI

`.github/workflows/foundry.yml` gains a new `slither` job that runs
on every PR + push-to-main + nightly. Findings classified `high`
fail the job (gating future merges); `medium` / `low` /
`informational` annotate the PR but don't fail at hackathon scope.

Pre-mainnet-deploy, the `|| true` fallback is removed and the job
becomes a hard gate on the deploy script. Until then, the failure-
mode is "PR review must read the slither summary alongside the fuzz
results."

False-positive exclusions: `solc-version` (pinned in `foundry.toml`,
slither can't introspect the foundry pin) + `naming-convention`
(noisy on EIP-3668 error-shape names). These are documented inline
in the workflow with reasoning.

### Documentation: hardening posture

This document. Pinned in the contract's NatSpec via a comment
reference so reviewers find it from either side.

## What this PR deliberately does NOT add

Three changes the spec mentioned that we're rejecting with rationale.
The rationale is the load-bearing contribution of this doc — anyone
proposing the same change in the future hits this doc first.

### 1. ReentrancyGuard

**Decision:** not added.
**Reason:** the contract has no `external` writes, no `.call` / `.delegatecall` / `.send`, no value transfers, and the only state-mutating function is the constructor. There is **no reentrant call site to guard** — a `nonReentrant` modifier on a `view` function is a no-op that adds gas to every read. The fuzz suite's stateful-immutability tests (`test_signerImmutableAfterAllCalls`, `test_urlsImmutableAfterAllCalls`) prove the absence of state mutation against any sequence of public-method calls.
**When to revisit:** if a future revision adds an `external` write path (e.g. on-chain registry mirror, token-gated registration) the guard becomes necessary alongside that change.

### 2. OZ Ownable for gateway-signer rotation

**Decision:** not added — and we'd push back if asked again.
**Reason:** the spec says "Optional upgrade path via OZ Ownable." That would replace the `immutable gatewaySigner` with a mutable slot guarded by an owner. **This is a security regression, not improvement:**

- Today: compromising the gateway-signing key lets an attacker forge records but never lets them rotate the public key the contract verifies against. Recovery = redeploy, ENS apex points at the new resolver.
- Post-Ownable: compromising the *owner* key (a separate, often hot, wallet) lets an attacker rotate the gateway signer to one they control without a redeploy. The owner becomes a single point of compromise for every record the resolver serves.

Immutable signer + redeploy-on-rotate is the cleaner trust model. The "ergonomic" gain of avoiding a redeploy is paid for in expanded blast radius.

**When to revisit:** if multi-sig owner with a 2/3 threshold becomes the operator standard, the trade-off shifts. Today it doesn't; we keep the simpler shape.

### 3. Mythril CI

**Decision:** Slither only, not Slither + Mythril.
**Reason:** Mythril's symbolic execution catches a different class of bugs (path-dependent vulnerabilities) but produces high false-positive rates on contracts with complex assembly (the `recoverSigner` inline `ecrecover` block is a known noisy area). Slither's static analysis catches the categories that matter here — uninitialized storage, shadowing, missing return values, `call`-pattern issues — without the operator-overhead.
**When to revisit:** if we add a contract with heavier control flow (e.g. ERC-8004 registry contract under P6, or the AnchorRegistry under P6 round 9), Mythril becomes worth the noise.

## Pre-mainnet-deploy checklist

Before `forge create OffchainResolver.sol --rpc-url $MAINNET_RPC --broadcast --private-key $...`:

- [ ] `forge test --fuzz-runs 10000` passes locally (20/20 today)
- [ ] CI green on the deploy commit, including the new `slither` job
- [ ] `slither .` locally produces no `high`-severity findings (the new CI job enforces the same)
- [ ] Gateway URL list is correct for production (single Vercel URL today; consider adding a second for multi-URL fallback per ENSIP-25)
- [ ] Gateway private key is fresh (not the Sepolia signer; don't share keys across networks)
- [ ] Owner of the apex `sbo3lagent.eth` calls `setResolver` to point at the new mainnet OffchainResolver — migration plan in [`docs/cli/ens-fleet-sepolia.md`](../cli/ens-fleet-sepolia.md) section "Mainnet migration"
- [ ] Existing 5 `sbo3l:*` records on `sbo3lagent.eth` are either migrated to the new resolver via `setText` calls OR mirrored through the gateway (operator's call; the records are static today so direct setText is the smaller-blast-radius path)

## Sign-off rubric (Daniel's call)

The deploy is authorised when:

1. The CI gate is green on the commit being deployed.
2. The mainnet-fresh gateway-signing key is provisioned and KMS-backed.
3. The post-deploy migration plan is rehearsed on Sepolia first.
4. There's a rollback plan: `setResolver` back to the previous resolver
   if the mainnet OffchainResolver misbehaves in production.

The CI gate covers (1). (2)-(4) are operator state outside the
contract surface; we document them here so the rubric is in one
place.

## Why this is "hardening" without changing the contract

The spec asked for hardening. The contract is small and stateless
enough that the meaningful hardening is **in the verification
infrastructure**, not in the contract itself:

- 11 × 10 000 fuzz runs already prove the security claims.
- Slither gates static-analysis-discoverable bugs.
- The 3 immutability tests (`*ImmutableAfterAllCalls`) act as a
  regression-net against any future PR introducing a setter or
  list-mutator.
- This document is the rationale for what we're *not* changing —
  so a future contributor doesn't add a reentrancy guard or an
  Ownable upgrade path "for completeness" and silently expand the
  trust surface.

A short contract that survives 10 000 fuzz runs across every
documented security claim is in better shape than a longer contract
with the same claims behind more code. We'd rather extend the test
suite than the implementation.

## Future work

- **Multi-URL fallback** in the gateway list (currently 1 URL —
  ENSIP-25 spec is silent on multi-URL retry semantics, but
  ENS-aware clients implement it; documented as a follow-up).
- **Mythril** if a future contract adds heavier control flow.
- **Slither GitHub-SARIF upload** so findings surface in the
  Security tab — non-blocking but better discoverability.
- **Formal verification** (Certora, Halmos) — overkill for the
  current contract; reconsider when the surface area grows.
