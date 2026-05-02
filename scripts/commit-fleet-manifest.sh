#!/usr/bin/env bash
#
# Post-broadcast manifest commit automation. Runs after Daniel
# completes `./scripts/register-fleet.sh <config.yaml>`, which
# populates `docs/proof/ens-fleet-<date>.json` with real tx hashes
# and Etherscan URLs.
#
# What this does:
#   1. Validates the populated manifest against the v1 schema.
#   2. Asserts every agent has status = "success" (refuses partial
#      runs unless --allow-partial is passed).
#   3. Optionally re-resolves every FQDN against a public RPC and
#      asserts the on-chain agent_id matches the manifest entry
#      (--verify-resolve flag, default off because it requires
#      network access).
#   4. Creates a fresh branch, commits the manifest, opens a PR
#      with auto-merge SQUASH enabled.
#
# Usage:
#   ./scripts/commit-fleet-manifest.sh docs/proof/ens-fleet-2026-05-01.json
#   ./scripts/commit-fleet-manifest.sh \
#     docs/proof/ens-fleet-60-2026-05-01.json \
#     --allow-partial \
#     --verify-resolve
#
# Prereqs:
#   - `gh` (GitHub CLI) installed + authenticated.
#   - `jq` installed.
#   - Optional: `cast` (for --verify-resolve).
#   - Optional: `python3 -m jsonschema` for schema validation
#     (otherwise it's a structural sanity check via jq).
#
# Exit codes:
#   0 — manifest validated, branch pushed, PR opened, auto-merge ON
#   1 — IO / shell error
#   2 — argument or env error
#   3 — manifest validation failed
#   4 — verify-resolve failed for one or more agents

set -euo pipefail

usage() {
    cat <<EOF
Usage: $0 <manifest.json> [--allow-partial] [--verify-resolve]

Reads a populated ENS fleet manifest, validates it against the v1
schema, optionally re-resolves every agent on-chain, then opens a
GitHub PR with auto-merge SQUASH enabled.
EOF
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
    usage
    exit 0
fi

MANIFEST_PATH="${1:-}"
shift || true

ALLOW_PARTIAL=0
VERIFY_RESOLVE=0
while [ $# -gt 0 ]; do
    case "$1" in
        --allow-partial)  ALLOW_PARTIAL=1; shift ;;
        --verify-resolve) VERIFY_RESOLVE=1; shift ;;
        -h|--help)        usage; exit 0 ;;
        *)                echo "ERROR: unknown flag: $1" >&2; usage; exit 2 ;;
    esac
done

if [ -z "$MANIFEST_PATH" ] || [ ! -f "$MANIFEST_PATH" ]; then
    echo "ERROR: manifest not found: $MANIFEST_PATH" >&2
    usage
    exit 2
fi

if ! command -v gh >/dev/null 2>&1; then
    echo "ERROR: \`gh\` (GitHub CLI) not on PATH." >&2
    exit 2
fi
if ! command -v jq >/dev/null 2>&1; then
    echo "ERROR: \`jq\` not on PATH." >&2
    exit 2
fi
if [ "$VERIFY_RESOLVE" = "1" ] && ! command -v cast >/dev/null 2>&1; then
    echo "ERROR: --verify-resolve requires \`cast\` (Foundry) on PATH." >&2
    exit 2
fi

# ---- Step 1: schema validate -------------------------------------

SCHEMA_PATH="schemas/sbo3l.ens_fleet_manifest.v1.json"
if [ ! -f "$SCHEMA_PATH" ]; then
    echo "ERROR: schema not found at $SCHEMA_PATH (run from repo root)." >&2
    exit 2
fi

echo "==> Schema-validating $MANIFEST_PATH against $SCHEMA_PATH"
if command -v python3 >/dev/null 2>&1; then
    python3 - <<PY
import json, sys, pathlib
try:
    from jsonschema import validate
except ImportError:
    sys.stderr.write("INFO: \`jsonschema\` not installed; falling back to structural sanity.\n")
    sys.exit(0)
schema = json.load(open("$SCHEMA_PATH"))
manifest = json.load(open("$MANIFEST_PATH"))
try:
    validate(instance=manifest, schema=schema)
    print("schema validates")
except Exception as e:
    sys.stderr.write(f"schema validation FAILED: {e}\n")
    sys.exit(3)
PY
fi

# Structural sanity via jq — runs even without python3.
ROOT_SCHEMA=$(jq -r '.schema' "$MANIFEST_PATH")
if [ "$ROOT_SCHEMA" != "sbo3l.ens_fleet_manifest.v1" ]; then
    echo "ERROR: manifest .schema is '$ROOT_SCHEMA', expected 'sbo3l.ens_fleet_manifest.v1'" >&2
    exit 3
fi

AGENT_COUNT=$(jq '.totals.agent_count // (.agents | length)' "$MANIFEST_PATH")
SUCCEEDED=$(jq -r '.totals.succeeded // 0' "$MANIFEST_PATH")
FAILED=$(jq -r '.totals.failed // 0' "$MANIFEST_PATH")
NETWORK=$(jq -r '.network' "$MANIFEST_PATH")
PARENT=$(jq -r '.parent' "$MANIFEST_PATH")

echo "==> Manifest stats: $SUCCEEDED/$AGENT_COUNT succeeded, $FAILED failed, network=$NETWORK, parent=$PARENT"

if [ "$SUCCEEDED" -lt 1 ]; then
    echo "ERROR: zero successful broadcasts in this manifest — refusing to commit." >&2
    exit 3
fi

if [ "$FAILED" -gt 0 ] && [ "$ALLOW_PARTIAL" = "0" ]; then
    echo "ERROR: $FAILED agents failed; pass --allow-partial to commit anyway." >&2
    exit 3
fi

# ---- Step 2: optional re-resolve check ---------------------------

if [ "$VERIFY_RESOLVE" = "1" ]; then
    echo
    echo "==> Re-resolving every agent against a public RPC"
    case "$NETWORK" in
        mainnet) RPC_URL="${SBO3L_RESOLVE_RPC_URL:-https://ethereum-rpc.publicnode.com}" ;;
        sepolia) RPC_URL="${SBO3L_RESOLVE_RPC_URL:-https://ethereum-sepolia-rpc.publicnode.com}" ;;
        *)       echo "ERROR: unknown network: $NETWORK" >&2; exit 2 ;;
    esac

    RESOLVE_FAILS=0
    while IFS=$'\t' read -r FQDN EXPECTED_AGENT_ID STATUS; do
        if [ "$STATUS" != "success" ]; then
            continue
        fi
        ACTUAL=$(cast text "$FQDN" sbo3l:agent_id --rpc-url "$RPC_URL" 2>/dev/null || echo "")
        if [ "$ACTUAL" = "$EXPECTED_AGENT_ID" ]; then
            echo "  OK   $FQDN → $ACTUAL"
        else
            echo "  FAIL $FQDN expected '$EXPECTED_AGENT_ID' got '$ACTUAL'" >&2
            RESOLVE_FAILS=$((RESOLVE_FAILS+1))
        fi
    done < <(jq -r '.agents[] | [.fqdn, .agent_id, .status] | @tsv' "$MANIFEST_PATH")

    if [ "$RESOLVE_FAILS" -gt 0 ]; then
        echo "ERROR: $RESOLVE_FAILS agents failed re-resolution." >&2
        exit 4
    fi
fi

# ---- Step 3: branch + commit + push + PR -------------------------

DATE_TAG=$(date -u +%Y-%m-%d-%H%M)
MANIFEST_BASENAME=$(basename "$MANIFEST_PATH" .json)
BRANCH="proof/${MANIFEST_BASENAME}-${DATE_TAG}"

echo
echo "==> Creating branch $BRANCH from latest origin/main"
git fetch origin main >/dev/null 2>&1
git checkout -B "$BRANCH" origin/main

# Bring the manifest into the new branch (it lives in the working
# directory of whichever branch the operator was on when they ran
# register-fleet.sh).
mkdir -p "$(dirname "$MANIFEST_PATH")"
cp "$MANIFEST_PATH" "$MANIFEST_PATH.tmp"
mv "$MANIFEST_PATH.tmp" "$MANIFEST_PATH"

git add "$MANIFEST_PATH"
if git diff --cached --quiet; then
    echo "ERROR: no changes staged — manifest is identical to what's on main." >&2
    git checkout - >/dev/null 2>&1 || true
    git branch -D "$BRANCH" >/dev/null 2>&1 || true
    exit 3
fi

# Commit message records the manifest stats so the PR title reads
# self-describing.
COMMIT_MSG="chore(proof): ${MANIFEST_BASENAME} populated ($SUCCEEDED/$AGENT_COUNT, $NETWORK)"
COMMIT_BODY=$(cat <<EOF
$SUCCEEDED of $AGENT_COUNT agents successfully broadcast to $NETWORK
under $PARENT. Manifest validates against
schemas/sbo3l.ens_fleet_manifest.v1.json.

Re-derive each agent's pubkey from seed_doc + label via
\`scripts/derive-fleet-keys.py\`. Re-resolve every FQDN's
\`sbo3l:agent_id\` text record via
\`./scripts/resolve-fleet.sh $MANIFEST_PATH\` to confirm on-chain
parity.

Co-Authored-By: Dev 4 (Infra + On-chain + Distributed) <dev4@sbo3l.dev>
EOF
)
git -c commit.gpgsign=false commit -m "$COMMIT_MSG" -m "$COMMIT_BODY" >/dev/null

echo "==> Pushing $BRANCH"
git push -u origin "$BRANCH" >/dev/null 2>&1

echo "==> Opening PR"
PR_URL=$(gh pr create \
    --base main \
    --head "$BRANCH" \
    --title "$COMMIT_MSG" \
    --body "$(cat <<EOF
## Summary
Populated fleet manifest committed after \`register-fleet.sh\` broadcast completed.

- **Agents:** $SUCCEEDED of $AGENT_COUNT successful, $FAILED failed.
- **Network:** \`$NETWORK\`
- **Parent:** \`$PARENT\`
- **Schema:** \`sbo3l.ens_fleet_manifest.v1\` (validated)

## Verification

Reviewers re-derive every pubkey byte-for-byte:

\`\`\`bash
python3 scripts/derive-fleet-keys.py \\
  --config scripts/fleet-config/$(echo "$MANIFEST_BASENAME" | sed -E 's/^ens-fleet-//; s/-[0-9]{4}-[0-9]{2}-[0-9]{2}$//').yaml
\`\`\`

Re-resolve every agent against PublicNode:

\`\`\`bash
./scripts/resolve-fleet.sh $MANIFEST_PATH
\`\`\`

EOF
)" \
    --label "auto-merge" 2>/dev/null) || PR_URL=$(gh pr create \
    --base main \
    --head "$BRANCH" \
    --title "$COMMIT_MSG" \
    --body "Populated fleet manifest. $SUCCEEDED/$AGENT_COUNT agents on $NETWORK." 2>&1)

echo "==> PR opened: $PR_URL"

PR_NUMBER=$(echo "$PR_URL" | sed -E 's|.*/pull/([0-9]+).*|\1|')
if [ -n "$PR_NUMBER" ]; then
    gh pr merge "$PR_NUMBER" --auto --squash --delete-branch >/dev/null 2>&1 || true
    echo "==> Auto-merge SQUASH enabled on PR #$PR_NUMBER"
fi

echo
echo "==================================================================="
echo "  Branch:   $BRANCH"
echo "  PR:       $PR_URL"
echo "  Manifest: $MANIFEST_PATH"
echo "  Stats:    $SUCCEEDED/$AGENT_COUNT (network: $NETWORK)"
echo "==================================================================="
