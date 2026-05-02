# GCP KMS — secp256k1 EthSigner runbook

How to provision a Google Cloud KMS key for SBO3L's EVM signing path
(`GcpEthKmsLiveSigner`, feature `eth_kms_gcp`), wire credentials, and
run the gated live integration tests.

## Status

The Rust backend (`crates/sbo3l-core/src/signers/eth_kms_gcp_live.rs`)
is **code-ready** as of R14 P3. No real KMS round-trip has been
verified yet — Daniel runs the live integration tests (`cargo test
--test gcp_kms_live`) in R15 once the key + service-account JSON
exist. Until then, the unit tests cover all decoding paths against
synthetic inputs (DER + PEM).

## 1 — Provision the KMS key

```bash
PROJECT=my-project
LOCATION=us-east1
KEYRING=sbo3l-keys
KEY=eth-audit

# Key ring (one-time per location):
gcloud kms keyrings create $KEYRING \
  --location $LOCATION \
  --project $PROJECT

# secp256k1 ECDSA-SHA256 key:
gcloud kms keys create $KEY \
  --keyring $KEYRING \
  --location $LOCATION \
  --purpose asymmetric-signing \
  --default-algorithm ec-sign-secp256k1-sha256 \
  --project $PROJECT
```

The full resource name is:

```
projects/$PROJECT/locations/$LOCATION/keyRings/$KEYRING/cryptoKeys/$KEY/cryptoKeyVersions/1
```

GCP auto-creates `cryptoKeyVersions/1` on key creation. List versions:

```bash
gcloud kms keys versions list \
  --keyring $KEYRING \
  --key $KEY \
  --location $LOCATION
```

## 2 — Service-account IAM role

Create a service account dedicated to the daemon:

```bash
gcloud iam service-accounts create sbo3l-daemon \
  --display-name "SBO3L daemon (KMS signing)"
```

Grant it the minimum role for signing:

```bash
gcloud kms keys add-iam-policy-binding $KEY \
  --keyring $KEYRING \
  --location $LOCATION \
  --member serviceAccount:sbo3l-daemon@$PROJECT.iam.gserviceaccount.com \
  --role roles/cloudkms.signerVerifier
```

`roles/cloudkms.signerVerifier` grants `cloudkms.cryptoKeyVersions.useToSign`
+ `cloudkms.cryptoKeyVersions.viewPublicKey` — exactly the two RPCs
the backend uses (`AsymmetricSign` + `GetPublicKey`).

Generate a service-account JSON key for non-GCE deployments:

```bash
gcloud iam service-accounts keys create /secure/sbo3l-daemon.json \
  --iam-account sbo3l-daemon@$PROJECT.iam.gserviceaccount.com
```

For GCE / GKE deployments use Workload Identity instead and skip the
JSON file.

## 3 — Daemon environment

```bash
# GCP credentials — either:
export GOOGLE_APPLICATION_CREDENTIALS=/secure/sbo3l-daemon.json
# OR rely on the metadata server (GCE / GKE Workload Identity).

# SBO3L wiring:
export SBO3L_ETH_SIGNER_BACKEND=gcp_kms
export SBO3L_ETH_GCP_KMS_KEY_NAME="projects/$PROJECT/locations/$LOCATION/keyRings/$KEYRING/cryptoKeys/$KEY/cryptoKeyVersions/1"

# Build the daemon with the live backend feature:
cargo build --release -p sbo3l-server --features sbo3l-core/eth_kms_gcp
```

## 4 — Run the gated integration tests

```bash
export GCP_KMS_TEST_ENABLED=1
cargo test -p sbo3l-core --features eth_kms_gcp --test gcp_kms_live -- --nocapture
```

Same shape as the AWS gated tests — sign a known digest, ecrecover,
assert the recovered address matches `eth_address()`. Without
`GCP_KMS_TEST_ENABLED=1` the tests skip cleanly.

## Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| `PermissionDenied` on `GetPublicKey` | IAM role missing | Add `roles/cloudkms.signerVerifier` (or at least `cloudkms.cryptoKeyVersions.viewPublicKey`). |
| `KeySpecMismatch { found_spec: "CryptoKeyVersionAlgorithm(8)" }` | Wrong algorithm (algorithm 8 = `EC_SIGN_P256_SHA256`) | Recreate the key with `--default-algorithm ec-sign-secp256k1-sha256`. |
| `gcp pem: missing BEGIN PUBLIC KEY header` | KMS returned an unexpected response shape | Verify the key version exists and is enabled. |
| `Authentication failed: invalid_grant` | Service-account JSON expired / clock skew | Regenerate the key, check NTP. |
| Constructor blocks on first call | DNS / firewall blocking `cloudkms.googleapis.com:443` | Confirm egress to GCP from the daemon's network. |

## Why this design

- **Direct gRPC to `cloudkms.googleapis.com`** via `google-cloud-kms`
  0.6 (the yoshidan family) — bypasses the official `google-cloud-rust`
  preview, which is still moving. The yoshidan crate is stable, has
  a clean async API, and avoids pulling `ethers-core` (which the 0.6
  `eth` feature would).
- **Algorithm enum cross-check** — we accept both
  `EC_SIGN_SECP256K1_SHA256` (= 31) and the legacy `UNSPECIFIED`
  (= 0); a mismatch (e.g. P-256, RSA) errors at constructor time so
  misconfiguration fails fast.
- **PEM → DER once, cached** — the public key arrives PEM-armored.
  We strip the envelope, base64-decode, parse SPKI, derive the
  EIP-55 address, and cache. Every subsequent `eth_address()` call
  is a memory read.
- **Shared decoding helpers with AWS path** — the SPKI + DER signature
  parser lives in `eth_kms_common.rs` and is exercised by both
  backends + an independent unit-test surface.
