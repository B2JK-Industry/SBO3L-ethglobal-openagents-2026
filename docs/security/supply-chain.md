# SBO3L supply-chain attestation

> **What this proves:** every SBO3L crate published to crates.io is
> SLSA Build Provenance v1-attested at publish time, signed via
> GitHub OIDC + Sigstore Fulcio, and recorded on the public Rekor
> transparency log. A consumer can independently verify that a
> downloaded `.crate` file was built by the canonical
> [`B2JK-Industry/SBO3L-ethglobal-openagents-2026`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026)
> repo at the tagged version, not by an attacker who hijacked the
> registry credential.

## Why this matters

`crates.io` doesn't ship cryptographic provenance by default. A
consumer downloads `sbo3l-cli-1.2.0.crate` and trusts that:

1. The registry served the right bytes.
2. The registry credential wasn't compromised.
3. The publishing workflow ran on the canonical repo.

(1) is mitigated by the registry's hashing; (2) and (3) historically
required out-of-band trust. Sigstore + GitHub Artifact
Attestations close those gaps with verifiable signatures tied to
the specific GitHub Actions workflow run that produced the
artifact.

For agentic-platform consumers (KH, ENS, Uniswap eval teams,
enterprise auditors checking the supply chain before importing
the crates), this is **enterprise-grade trust signal** — present
in 100% of the publish flows, zero operator burden.

## How it works

`.github/workflows/crates-publish.yml` does this for every crate:

1. **Tag + version sanity check.** The workflow refuses to publish
   if the git tag doesn't match the workspace version in
   `Cargo.toml`.
2. **`cargo package`.** Builds the `.crate` file at
   `target/package/<name>-<version>.crate`.
3. **`actions/attest-build-provenance@v2`.** Generates a SLSA
   Build Provenance v1 statement covering the `.crate` file,
   signs it with the workflow's GitHub OIDC token (Fulcio), and
   pushes the bundle to the public Rekor transparency log.
4. **`cargo publish --no-verify`.** Uploads the same `.crate`
   file to crates.io. `--no-verify` skips re-running the build
   server-side (the local build already succeeded; this prevents
   non-determinism between the attested artifact and the
   published one).
5. **30s sleep.** Lets the crates.io index propagate before the
   next dependent crate publishes.

The attestation is bound to:
- **The workflow file path:** `.github/workflows/crates-publish.yml`
- **The git ref:** the tag (e.g. `refs/tags/v1.3.0`)
- **The repo:** `B2JK-Industry/SBO3L-ethglobal-openagents-2026`

A forge attempting to attach a fake attestation to a tampered
crate would need to either (a) compromise the GitHub Actions
OIDC issuer (nation-state level), or (b) have the repo's
maintainer push the tag (which means it's not a forge — it's the
maintainer).

## How to verify (consumer side)

### Option A — `gh attestation verify` (simplest)

Requires GitHub CLI 2.49+. Run:

```bash
# Download the .crate from crates.io
curl -L -o sbo3l-cli-1.3.0.crate \
  https://crates.io/api/v1/crates/sbo3l-cli/1.3.0/download

# Verify the attestation
gh attestation verify sbo3l-cli-1.3.0.crate \
  --owner B2JK-Industry
```

Expected output:

```
Loaded digest sha256:<hash> for file://sbo3l-cli-1.3.0.crate
Loaded 1 attestation from GitHub API
✓ Verification succeeded!

The following policy criteria will be enforced:
- Predicate type must match:................ https://slsa.dev/provenance/v1
- Source Repository Owner URI must match:... https://github.com/B2JK-Industry
- Predicate must be signed by GitHub.com.... ✓
```

### Option B — `cosign verify-blob` (no GitHub CLI)

Requires sigstore/cosign 2.x. Useful when the consumer doesn't
trust GitHub's CLI specifically and wants to verify against the
Sigstore root directly.

```bash
# Download the attestation bundle from the workflow run.
# (Path: workflow run → "Attestations" surface → download per-crate
# bundle. Or via the public Rekor log: rekor-cli search ...)

cosign verify-blob \
  --certificate-identity-regexp \
    "^https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/.github/workflows/crates-publish.yml@refs/tags/v[0-9].*" \
  --certificate-oidc-issuer \
    "https://token.actions.githubusercontent.com" \
  --bundle sbo3l-cli-1.3.0.crate.attestation.json \
  sbo3l-cli-1.3.0.crate
```

The `certificate-identity-regexp` pin ensures the attestation
came from the canonical workflow file at a release tag (not from
a feature branch or a non-maintainer fork).

### Option C — Inspect the Rekor entry directly

Every attestation is on the public Rekor transparency log:

```bash
# Find the entry by .crate digest
rekor-cli search --sha sha256:<crate-file-digest>

# Inspect the certificate + statement
rekor-cli get --uuid <returned-uuid>
```

Useful for auditors who want to confirm the entry was committed
to the log (proves the attestation existed at a point in time
even if the workflow run is later deleted).

## What's covered + what's not

| Covered | Not covered |
|---|---|
| `.crate` file integrity (the bytes you download match the bytes built) | The `Cargo.toml` `[dependencies]` of the crate. Transitive deps are NOT attested — that's the consumer's job (cargo audit, deny.toml). |
| Identity of the publisher (B2JK-Industry GitHub Actions OIDC) | The intent / business logic of the publisher. Attestation says "this was built by the canonical workflow"; it does NOT say "this code is bug-free." |
| Tag-binding (an attestation for v1.3.0 only verifies v1.3.0) | Reproducibility. We don't (yet) ship a deterministic-build manifest; rebuilding from source on a different machine may produce a different `.crate` digest. |
| Rekor log inclusion (entry persists publicly even if the workflow run is deleted) | Pre-publish supply-chain (build cache poisoning, RustSec advisories on deps). Run `cargo audit` separately. |

## Backfill posture

Existing 1.2.0 publishes (shipped pre-2026-05-03 without
attestation) **are not retroactively attested.** Backfilling
would require re-firing the publish workflow on the existing
v1.2.0 tag — but the tag's commit is already mainline, and
cargo refuses to re-publish the same version (immutability).

The honest posture: 1.2.0 is unattested; 1.3.0+ will be attested
from publish onward. The workflow change is forward-only.

If a consumer needs attestation for 1.2.0 specifically, the
options are:
- Wait for 1.3.0 (when this workflow fires for the first time
  end-to-end).
- Manually re-run cargo from the v1.2.0 tag locally + verify
  against the public crates.io 1.2.0 hash. This is reproducibility
  by hand, not crypto-attested.

## Operational notes for Daniel

- **No new credentials required.** Sigstore uses GitHub OIDC,
  which is already provided to the workflow via
  `permissions: id-token: write`.
- **No new secrets in repo.** The Fulcio CA + Rekor log are
  public Sigstore infrastructure.
- **No breaking change to the publish flow.** A consumer who
  doesn't verify attestation gets the same `.crate` file as
  before; verification is opt-in.
- **Workflow run time impact:** ~5-10s per crate for the
  packaging + attestation step. Total publish time goes from
  ~5 min (existing 9 crates × 30s sleep) to ~6-7 min. Acceptable.

## Cross-track context

Pairs with the upstream community PRs:
- [ENSIP-26](https://github.com/ensdomains/ensips/pull/71) gives
  agents a verifiable identity at the ENS layer.
- [Universal Router policy-guarded swap](https://github.com/Uniswap/universal-router/pull/477)
  gives agents per-command policy gating with signed receipts.
- [KeeperHub policy-receipt envelope](https://github.com/KeeperHub/cli/pull/57)
  gives agents a workflow-submit envelope shape carrying the
  signed receipt.

This supply-chain attestation closes the loop: the **adapter
crates that consumers use** to interact with the above protocols
are themselves verifiable. End-to-end trust chain:

```
agent identity (ENSIP-26)
  → policy decision (per-command UR pattern)
    → workflow submit (KH IP-1 envelope)
      → adapter crate (this attestation)
```

Every link is signed.

## References

- [SLSA Provenance v1](https://slsa.dev/spec/v1.0/provenance) — the
  attestation predicate format.
- [Sigstore Fulcio](https://github.com/sigstore/fulcio) — short-lived
  certificate issuer used for the OIDC-bound signature.
- [Sigstore Rekor](https://github.com/sigstore/rekor) — the public
  transparency log every attestation lands on.
- [actions/attest-build-provenance](https://github.com/actions/attest-build-provenance)
  — the GitHub Action this workflow uses.
- [GitHub Artifact Attestations docs](https://docs.github.com/en/actions/security-guides/using-artifact-attestations-to-establish-provenance-for-builds)
