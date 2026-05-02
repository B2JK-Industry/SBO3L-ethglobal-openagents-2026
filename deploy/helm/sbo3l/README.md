# sbo3l — Helm chart

> **EXPERIMENTAL — chart skeleton.** Lints clean, templates clean, **NOT
> validated on a live cluster.** This chart is a SCAFFOLD operators can
> fork. Every default in `values.yaml` should be reviewed before any
> production rollout. CRDs (`SBO3LPolicy`, `SBO3LCluster`) and the
> multi-replica Raft cluster mode are deliberately out of scope here —
> see "What's NOT in this chart" below.

Deploys a single SBO3L server instance (the `sbo3l-server` daemon —
cryptographically verifiable trust layer for autonomous AI agents) as
a Kubernetes `Deployment` + `Service`. The chart is intended for
operators who want to run one daemon behind their existing ingress +
monitoring stack.

## Versions

| chart `version` | chart `appVersion` | image tag default |
| ---             | ---                | ---               |
| `0.1.0`         | `1.2.0`            | `sbo3l/server:1.2.0` |

`appVersion` tracks the workspace package version; the chart `version`
is bumped independently on chart-shape changes.

## Prerequisites

- Kubernetes `>= 1.27.0` (matches `Chart.yaml#kubeVersion`).
- Helm `>= 3.10`.
- A container registry that mirrors `sbo3l/server` (or override
  `image.repository` to point at your own).
- (Optional) Prometheus Operator CRDs in-cluster if you enable
  `serviceMonitor.enabled=true`.
- (Optional) An IngressClass installed if you enable `ingress.enabled=true`.

## Quickstart — local smoke test

> **Danger:** the flags below enable the dev bypasses. NEVER use them
> in production.

```bash
helm install sbo3l ./deploy/helm/sbo3l \
  --set env.allowUnauthenticated=true \
  --set env.devOnlySigner=true

kubectl port-forward svc/sbo3l 8730:8730
curl http://127.0.0.1:8730/v1/healthz
```

The two `--set` flags above:

- `env.allowUnauthenticated=true` flips the F-1 auth bypass on. Without
  it, the server defaults to deny-all on `POST /v1/payment-requests`.
- `env.devOnlySigner=true` is the F-5 signer gate — the binary refuses
  to start with `signer.backend=dev` unless this flag is also set.

For any non-trivial deployment, leave both off and configure a
production signer backend (see "KMS backends" below).

## Common overrides

### Production-shaped install (single replica, KMS, ingress, monitoring)

```bash
helm install sbo3l ./deploy/helm/sbo3l \
  --set image.tag=1.2.0 \
  --set replicaCount=1 \
  --set persistence.enabled=true \
  --set persistence.size=10Gi \
  --set signer.backend=aws_kms \
  --set signer.awsKmsKeyArn=arn:aws:kms:us-east-1:111111111111:key/aaaa-bbbb-cccc \
  --set serviceAccount.annotations."eks\.amazonaws\.com/role-arn"=arn:aws:iam::111111111111:role/sbo3l-kms \
  --set existingSecret=sbo3l-prod-creds \
  --set serviceMonitor.enabled=true \
  --set ingress.enabled=true \
  --set ingress.className=nginx \
  --set 'ingress.hosts[0].host=sbo3l.example.com' \
  --set 'ingress.hosts[0].paths[0].path=/' \
  --set 'ingress.hosts[0].paths[0].pathType=Prefix' \
  --set 'ingress.tls[0].secretName=sbo3l-tls' \
  --set 'ingress.tls[0].hosts[0]=sbo3l.example.com'
```

> The current binary refuses to start with a non-`dev` signer backend
> until the F-5 AppState refactor lands (see
> `crates/sbo3l-server/src/main.rs`). The chart accepts the production
> values now so operator config can be staged ahead of the runtime
> change.

### KMS backends

The chart exposes three signer backend strings:

| `signer.backend` | extra env wired by chart           | auth supplied by operator                              |
| ---              | ---                                | ---                                                    |
| `dev`            | (no extra env)                     | none — uses public dev seeds                           |
| `aws_kms`        | `SBO3L_AWS_KMS_KEY_ARN`            | IRSA on the ServiceAccount, or `AWS_*` env vars        |
| `gcp_kms`        | `SBO3L_GCP_KMS_KEY_NAME`           | Workload Identity, or `GOOGLE_APPLICATION_CREDENTIALS` |

For `aws_kms` IRSA, annotate the ServiceAccount in your overrides:

```yaml
serviceAccount:
  create: true
  annotations:
    eks.amazonaws.com/role-arn: arn:aws:iam::111111111111:role/sbo3l-kms
```

For `gcp_kms` Workload Identity, annotate similarly:

```yaml
serviceAccount:
  create: true
  annotations:
    iam.gke.io/gcp-service-account: sbo3l-kms@my-project.iam.gserviceaccount.com
```

### Secrets

Two patterns are supported:

1. **`existingSecret` (recommended).** Manage the Secret out-of-band
   (External Secrets Operator, sealed-secrets, sops-flux, …) and tell
   the chart its name:

   ```yaml
   existingSecret: sbo3l-prod-creds
   ```

   The chart will `envFrom: secretRef: name: sbo3l-prod-creds` so any
   keys in that Secret become environment variables in the daemon.

2. **`secret.create: true` (chart-managed placeholder).** Useful for
   GitOps repos that prefer all manifests live in one place. Set
   `secret.data` to a map; values are b64-encoded automatically:

   ```yaml
   secret:
     create: true
     data:
       SBO3L_BEARER_TOKEN_HASH: "$2b$12$..."
       SBO3L_JWT_PUBKEY_HEX: "0x..."
   ```

   **Do not** check plaintext secrets into Git. Use sops, sealed-secrets,
   or any other transport encryption.

### Persistence

The daemon writes a SQLite database at `/var/lib/sbo3l/sbo3l.db` (the
container image's `VOLUME`). With `persistence.enabled=false` (default),
the chart mounts an `emptyDir` — fine for ephemeral demos but the audit
chain is lost on pod restart. For real use:

```yaml
persistence:
  enabled: true
  size: 10Gi
  storageClass: gp3
```

When persistence is on, the chart automatically forces
`spec.strategy.type=Recreate` because `ReadWriteOnce` PVCs cannot be
attached to two pods during a rolling update.

### Resources

The defaults (100m CPU request, 128Mi memory request, no limits) are
sized for a low-traffic single-tenant deployment. Profile under your
own workload before going to production.

### Pod security

The chart ships with a hardened pod by default:

- `runAsNonRoot: true`, `runAsUser: 65532` (matches the distroless image)
- `readOnlyRootFilesystem: true` (a writable `emptyDir` is mounted at
  `/tmp` for tracing-subscriber + tokio scratch)
- `allowPrivilegeEscalation: false`
- All Linux capabilities dropped
- `seccompProfile: RuntimeDefault`

Adjust via `podSecurityContext` and `securityContext` in your overrides
if your environment requires different settings (e.g. PSA-baseline
clusters can flip `seccompProfile.type` to `Unconfined`, but you
should not).

## Publishing to Artifact Hub

This chart is `experimental` while in `0.x.y`. To graduate:

1. Bump `Chart.yaml#version` to `1.0.0` and remove the EXPERIMENTAL
   banner from `Chart.yaml#description` and from `values.yaml`'s top
   comment.
2. Run `helm package deploy/helm/sbo3l`.
3. Sign the chart (`helm package --sign --key '<key>'`).
4. Push to your OCI registry (`helm push sbo3l-1.0.0.tgz oci://...`)
   or to a static repo index (`helm repo index --url <url> .`).
5. Submit to Artifact Hub via a `artifacthub-repo.yml` in the repo
   root pointing at the chart directory; see
   <https://artifacthub.io/docs/topics/repositories/helm-charts/>.

Until then, the chart is consumed via `helm install … ./deploy/helm/sbo3l`
from a checkout of the repo.

## Verification

This chart's release artifacts are validated with:

```bash
# Lint pass (must be clean):
helm lint deploy/helm/sbo3l
helm lint --strict deploy/helm/sbo3l

# Default render:
helm template test deploy/helm/sbo3l

# Full feature render (every optional template enabled):
helm template test deploy/helm/sbo3l \
  --set ingress.enabled=true \
  --set ingress.className=nginx \
  --set 'ingress.hosts[0].host=sbo3l.example.com' \
  --set 'ingress.hosts[0].paths[0].path=/' \
  --set 'ingress.hosts[0].paths[0].pathType=Prefix' \
  --set serviceMonitor.enabled=true \
  --set signer.backend=aws_kms \
  --set 'signer.awsKmsKeyArn=arn:aws:kms:us-east-1:111:key/abc' \
  --set persistence.enabled=true \
  --set secret.create=true \
  --set podDisruptionBudget.enabled=true \
  --set startupProbe.enabled=true
```

These were the only validations performed at chart authoring time. **No
live cluster apply was attempted.** The first operator to deploy this
chart should expect to debug at least one issue (image-pull permissions,
PVC storageClass mismatch, ingress class typo, KMS auth, …) — that's
the cost of "scaffold, not validated".

## What's NOT in this chart yet

This is a deliberate scope cut. The following are out of scope for the
0.x line and tracked separately:

- **Multi-replica with shared storage.** The audit chain is single-writer;
  multi-replica without coordination silently corrupts the chain. The
  experimental Raft cluster scaffold is in `docker-compose-cluster.yml`
  at repo root; the corresponding Helm chart will be a separate
  `sbo3l-cluster` chart that builds on this one.
- **CRDs (`SBO3LPolicy`, `SBO3LCluster`).** CRDs without an operator are
  inert. The operator + its CRDs will live in a separate `sbo3l-operator`
  chart project.
- **Live mTLS between sidecars.** Deferred to service-mesh integration
  (Linkerd / Istio / cilium-mesh take this off the chart's plate).
- **Backup / restore automation for the SQLite DB.** Use Velero or a
  scheduled `kubectl exec` cronjob — the chart does not opine.
- **Horizontal Pod Autoscaler.** Pointless until multi-replica works.
- **Live-cluster smoke test in CI.** The chart is render-tested only.

## Comparison

| Use case                                      | Use this chart? |
| ---                                           | ---             |
| Deploy a single SBO3L instance behind ingress | yes             |
| Run the experimental Raft cluster (3+ nodes)  | no — see `docker-compose-cluster.yml` or wait for `sbo3l-cluster` |
| Deploy CRDs + an operator that reconciles them | no — wait for `sbo3l-operator` |
| Local dev with docker-compose                 | no — use `docker-compose.yml` at repo root |
| Bare-metal install without Kubernetes         | no — use the binary directly per `docs/cli/docker.md` |

## Source + license

- Source: <https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026>
- License: Apache-2.0 (matches the workspace LICENSE)
