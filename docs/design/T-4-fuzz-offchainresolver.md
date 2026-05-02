# OffchainResolver fuzz + invariant suite

**Status:** Shipped (this PR).
**Track:** ENS — Phase 2 production hardening.
**Path:** `crates/sbo3l-identity/contracts/test/OffchainResolver.invariant.t.sol`.

## Why

The Sepolia OffchainResolver
(`0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3`) is in the verified
critical path for live ENS subname resolution via the SBO3L
CCIP-Read gateway. The unit suite that shipped with the contract
(6 tests) covered the named happy and unhappy paths — fuzzing
broadens that coverage to 11×10000 random inputs against the same
security claims, plus 3 structural-immutability checks.

## Suite layout

`OffchainResolver.invariant.t.sol` ships two test contracts:

### `OffchainResolverFuzzTest` — 11 fuzz tests (10K runs each)

| Test | Property under test |
|---|---|
| `testFuzz_validSignatureAlwaysVerifies` | gateway-signed responses are accepted for any `(value, data, ttl)` |
| `testFuzz_invalidSignatureRejects` | random `(r, s, v)` triples are rejected (catches accidental sig-skipping) |
| `testFuzz_tamperedValueRejects` | substituting `value` after signing → rejection |
| `testFuzz_tamperedDataRejects` | substituting `extraData` after signing → rejection |
| `testFuzz_expiredSignatureRejects` | `block.timestamp > expires` → `SignatureExpired` |
| `testFuzz_unauthorizedSignerRejects` | any non-gateway key → `UnauthorizedSigner` |
| `testFuzz_resolveAlwaysRevertsWithOffchainLookup` | `resolve()` never returns on-chain — always reverts via CCIP-Read |
| `testFuzz_constructorRejectsZeroSigner` | misconfigured deploy (signer=0) reverts |
| `testFuzz_constructorAcceptsAnyNonzeroSigner` | constructor preserves `signer` + `urls` exactly |
| `testFuzz_recoverSignerRejectsBadLength` | sigs not exactly 65 bytes → `InvalidSignerLength` |
| `testFuzz_supportsInterfaceTrueOnlyForKnownIds` | only `IExtendedResolver` + ERC-165 advertised |

### `OffchainResolverImmutabilityTest` — 3 structural checks

| Test | Property |
|---|---|
| `test_signerImmutableAfterAllCalls` | `gatewaySigner` cannot be mutated post-deploy |
| `test_urlsImmutableAfterAllCalls` | URL list cannot be mutated post-deploy |
| `test_interfaceAdvertisementStable` | ERC-165 answer is constant |

The contract has no setter for `gatewaySigner` and no `push`/`pop`
on `urls`, so the structural-immutability claim holds by
construction. These tests serve as a regression net: any future PR
that adds a setter or list-mutator will fail the suite.

## Why fuzz tests instead of `forge invariant`

Foundry's `forge invariant` runner needs a state-mutating function
to fuzz call sequences against. OffchainResolver is effectively
stateless after construction — all public functions are `view` /
`pure` or always revert. The invariant runner reports
"No contracts to fuzz" because it has nothing to call. The fuzz
tests above cover the same security claims with random inputs over
direct invocations, which is the right tool for this contract
shape.

## CI

`.github/workflows/foundry.yml` runs:

1. Unit + immutability suite (no fuzz).
2. Fuzz suite at **10000 runs per test**.
3. `forge coverage --report summary` (best-effort).

Triggered on PR + push to main + nightly cron. Nightly soak catches
counter-examples that don't surface in PR-time fuzz windows;
foundry's RNG is deterministic per seed, so a nightly miss = real
change in the suite.

## Running locally

```bash
cd crates/sbo3l-identity/contracts
forge install foundry-rs/forge-std --no-git
forge test --fuzz-runs 10000
```

20 tests pass (6 unit + 11 fuzz + 3 immutability). Wall-clock at
10K fuzz runs: ~13s on a M-series Mac.
