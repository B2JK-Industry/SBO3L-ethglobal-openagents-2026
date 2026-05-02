# AWS KMS — secp256k1 EthSigner runbook

How to provision an AWS KMS key for SBO3L's EVM signing path
(`AwsEthKmsLiveSigner`, feature `eth_kms_aws`), wire credentials, and
run the gated live integration tests.

## Status

The Rust backend (`crates/sbo3l-core/src/signers/eth_kms_aws_live.rs`)
is **code-ready** as of R14 P3. No real KMS round-trip has been
verified yet — Daniel runs the live integration tests (`cargo test
--test aws_kms_live`) in R15 once the key + creds exist. Until then,
the unit tests cover all decoding paths against synthetic DER fixtures.

## 1 — Provision the KMS key

```bash
aws kms create-key \
  --region us-east-1 \
  --key-spec ECC_SECG_P256K1 \
  --key-usage SIGN_VERIFY \
  --description "SBO3L EVM signer (eth audit/receipt)"
```

Capture the `KeyId` (UUID) and `Arn` from the response. Optionally
attach an alias for stable referencing:

```bash
aws kms create-alias \
  --alias-name alias/sbo3l-eth-audit \
  --target-key-id <KeyId>
```

## 2 — IAM policy for the daemon's role

The role under which the daemon runs (EC2 instance profile / ECS task
role / lambda execution role) needs:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "Sbo3lKmsSign",
      "Effect": "Allow",
      "Action": ["kms:Sign", "kms:GetPublicKey"],
      "Resource": "arn:aws:kms:us-east-1:<account>:key/<key-id>"
    }
  ]
}
```

Both `kms:Sign` and `kms:GetPublicKey` are required — `GetPublicKey` is
called once at constructor time to derive the EIP-55 address; `Sign` is
called per signing operation. Omitting `GetPublicKey` causes
`AwsEthKmsLiveSigner::with_client` to error with `AccessDenied` at
startup.

## 3 — Daemon environment

```bash
# AWS SDK credentials chain — any of:
#  - AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY env vars
#  - shared credentials file (~/.aws/credentials)
#  - EC2 instance metadata service (IMDSv2)
#  - ECS task role
export AWS_REGION=us-east-1

# SBO3L wiring:
export SBO3L_ETH_SIGNER_BACKEND=aws_kms
export SBO3L_ETH_AWS_KMS_KEY_ID=arn:aws:kms:us-east-1:<account>:key/<key-id>
# OR (alias to the same value):
# export SBO3L_ETH_AWS_KMS_KEY_ARN=arn:aws:kms:us-east-1:<account>:key/<key-id>

# Build the daemon with the live backend feature:
cargo build --release -p sbo3l-server --features sbo3l-core/eth_kms_aws
```

The factory in `sbo3l_core::signers::eth_signer_from_env` routes to
`AwsEthKmsLiveSigner::from_env` when both the env var AND the feature
flag are set. Without the feature flag the factory returns
`SignerError::BackendNotCompiled("aws_kms")` even with the env var
present — by design, since the SDK isn't compiled in.

## 4 — Run the gated integration tests

```bash
export AWS_KMS_TEST_ENABLED=1
cargo test -p sbo3l-core --features eth_kms_aws --test aws_kms_live -- --nocapture
```

The tests:

1. Build a real `AwsEthKmsLiveSigner` from env.
2. Call `eth_address()` — must succeed (one `GetPublicKey` round-trip).
3. Sign a known 32-byte digest with `sign_digest_hex(&digest)`.
4. Run `ecrecover` over the resulting 65-byte signature + digest, and
   assert the recovered EIP-55 address matches `eth_address()`.

If step 4 fails, the bug is one of:

- Wrong key spec (`KeySpec::EccNistP256` instead of `EccSecgP256K1`)
- The cached pubkey + signed digest don't agree (clock-skew style
  staleness — extremely unlikely with KMS).

Without `AWS_KMS_TEST_ENABLED=1` the tests print `SKIP:` and return ok.
Default CI never touches AWS.

## Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| `KMSInvalidStateException` | Key is in `PendingDeletion` | `aws kms cancel-key-deletion --key-id <id>` |
| `AccessDeniedException` on `Sign` | IAM policy missing `kms:Sign` | Add `kms:Sign` to the role's policy. |
| `KeySpecMismatch { found_spec: "EccNistP256" }` | Key was created with the wrong spec | Provision a new key with `--key-spec ECC_SECG_P256K1`. |
| Constructor hangs ~30s then errors | Region mismatch (key in `us-west-2`, daemon in `us-east-1`) | Set `AWS_REGION` to the key's region or use a region-qualified ARN. |
| `SDK config: no credentials` | No creds in chain | Set `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY`, attach an instance profile, or run `aws configure`. |

## Why this design

- **Pubkey caching at constructor time** — every `eth_address()` call
  after the first is a memory read. KMS `GetPublicKey` is rate-limited
  and bills per call; caching saves both quota and latency.
- **Recovery byte derived locally** — KMS returns DER `(r, s)` only.
  We try both recovery ids 0 and 1 and pick the one that recovers the
  cached pubkey. One ECDSA verify is faster than a second KMS round-trip.
- **Low-S normalization** — EIP-2 / Bitcoin's malleability fix. KMS
  may return high-S signatures; the on-chain Solidity ecrecover
  rejects them. We normalize before returning.
