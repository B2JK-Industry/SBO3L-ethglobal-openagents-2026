# `sbo3l-verify` — GitHub Action

Verify a SBO3L Passport capsule in CI. Posts a markdown report to the workflow run summary + (optionally) a PR comment.

## Usage

```yaml
- uses: B2JK-Industry/SBO3L-ethglobal-openagents-2026/actions/sbo3l-verify@main
  with:
    capsule: ./artifacts/run-001-capsule.json
    fail-on-deny: true        # default; set 'false' to keep CI green on deny
    comment-on-pr: auto       # default 'auto' = on; explicit true/false also accepted
```

## Inputs

| name | required | default | meaning |
|---|---|---|---|
| `capsule` | yes | — | Path to capsule JSON, relative to `$GITHUB_WORKSPACE` |
| `fail-on-deny` | no | `true` | Fail the action if capsule decision is deny / requires_human |
| `comment-on-pr` | no | `auto` | Post the markdown report as a PR comment (`auto` = on for `pull_request` events) |

## Outputs

| name | shape | example |
|---|---|---|
| `decision` | `allow \| deny \| requires_human` | `allow` |
| `audit-event-id` | string (or empty) | `evt-01HTAWX5K3R8YV9NQB7C6P2DGM` |
| `checks-passed` | `n/total` | `6/6` |

## Verifier checks (6)

The action runs the same shape checks the SDK's `verify()` ships:

1. `capsule.is_object` — capsule parses as a JSON object
2. `capsule.type_recognised` — `capsule_type` (or legacy `receipt_type`) starts with `sbo3l.`
3. `capsule.decision_set` — decision ∈ {allow, deny, requires_human}
4. `capsule.audit_event_id_present` — id matches `evt-…`
5. `capsule.request_hash_present` — 64 hex chars
6. `capsule.policy_hash_present` — 64 hex chars

For full Ed25519 signature verification + ENS lookup, install `@sbo3l/sdk` in your workflow and call `verify()` directly — this action is the lightweight CI surface (zero install).

## Example workflow

```yaml
name: Verify capsule

on:
  pull_request:
    paths: ["artifacts/**.capsule.json"]

jobs:
  verify:
    runs-on: ubuntu-latest
    permissions:
      pull-requests: write   # required for the PR comment
      contents: read
    steps:
      - uses: actions/checkout@v4
      - uses: B2JK-Industry/SBO3L-ethglobal-openagents-2026/actions/sbo3l-verify@main
        with:
          capsule: artifacts/latest.capsule.json
```

## Tests

```bash
node test/verifier.test.mjs    # 8 inline tests, no install
```

The action ships with no `node_modules/`. Verifier is ~110 LoC of pure JS using only Node's stdlib + `fetch` (Node 20+).

## Marketplace listing

After this PR lands, the action is published to the GitHub Marketplace via the standard "Publish this Action" UI flow (no auto-publish from CI — needs Daniel's manual click in the repo's Releases tab on a tagged release).
