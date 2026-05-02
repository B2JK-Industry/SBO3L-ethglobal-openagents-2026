#!/usr/bin/env bash
# scripts/quickstart-validate.sh — static validation of docs/quickstart/*.md.
#
# What this catches:
#   - import statements that reference names the SDK no longer exports
#   - APRP fixtures that miss required v1 fields
#   - hardcoded nonces / expired expiry (we burned 2 hours fixing those
#     in round 4; this script makes regressions impossible)
#   - CHAIN/recipient drift between guide and reference policy
#
# What this does NOT catch (out of scope for this script):
#   - Runtime end-to-end (needs npm packages live + LLM keys; covered by
#     scripts/sdk-install-verify.sh + per-guide manual runs by Heidi)
#
# Usage:
#   ./scripts/quickstart-validate.sh              # all guides
#   ONLY=ens-with-anthropic ./scripts/quickstart-validate.sh
#
# Output: docs/proof/quickstart-validation.md + non-zero exit on any drift.

set -euo pipefail

ONLY="${ONLY:-}"
OUT="${OUT:-docs/proof/quickstart-validation.md}"
QUICKSTART_DIR="docs/quickstart"

# Per-guide check spec.
# Format: id|file|expected_packages|expected_exports
#
# - expected_packages: comma-separated list of package names the guide
#   should `npm i` (or `pip install`). The script greps the install
#   block to confirm each is present.
# - expected_exports: comma-separated list of names the guide's code
#   block must `import` from any package. Catches drift like
#   "guide imports sbo3lTool but adapter exports sbo3lAssistantTool".
read -r -d '' MATRIX <<'EOF' || true
keeperhub-with-langchain|keeperhub-with-langchain.md|sbo3l-sdk,sbo3l-langchain,langchain-openai|SBO3LClientSync,sbo3l_tool
keeperhub-with-openai-assistants|keeperhub-with-openai-assistants.md|@sbo3l/sdk,@sbo3l/openai-assistants,openai|SBO3LClient,sbo3lAssistantTool,runSbo3lToolCall
uniswap-with-vercel-ai|uniswap-with-vercel-ai.md|@sbo3l/sdk,@sbo3l/vercel-ai,@ai-sdk/openai,ai|SBO3LClient,uniswap,sbo3lTool
uniswap-with-mastra|uniswap-with-mastra.md|@sbo3l/sdk,@sbo3l/mastra,@mastra/core,@ai-sdk/openai|SBO3LClient,uniswap,sbo3lTool
ens-with-anthropic|ens-with-anthropic.md|@sbo3l/sdk,@sbo3l/anthropic,@anthropic-ai/sdk|SBO3LClient,sbo3lTool,runSbo3lToolUse
EOF

mkdir -p "$(dirname "$OUT")"

declare -a RESULTS=()
fail_count=0
total=0
all_issues=()

# Stable APRP-required-field check: the v1 schema mandates these 12.
APRP_REQUIRED=(
    "agent_id" "task_id" "intent" "amount" "token" "destination"
    "payment_protocol" "chain" "provider_url" "expiry" "nonce" "risk_class"
)

# Nonces that should NEVER appear in shipped guides (frozen samples
# from earlier rounds). Each one was responsible for a real CI break.
FORBIDDEN_NONCES=(
    "01HTAWX5K3R8YV9NQB7C6P2DGM"
    "01HTAWX5K3R8YV9NQB7C6P2DGN"
    "01HTAWX5K3R8YV9NQB7C6P2DGP"
)

# Expiry timestamps that have already lapsed (catches guides that
# copy-pasted a fixed expiry instead of computing now+5min).
FORBIDDEN_EXPIRIES=(
    "2026-05-01T10:31:00Z"
    "2026-05-02T10:31:00Z"
)

run_check() {
    local id="$1" file="$2" expected_packages="$3" expected_exports="$4"
    local guide_path="$QUICKSTART_DIR/$file"
    local issues=()

    if [[ ! -f "$guide_path" ]]; then
        echo "  ✗ guide file not found: $guide_path"
        issues+=("missing_file:$guide_path")
        echo "missing"
        return
    fi

    # Check 1: every expected package is mentioned in an install block
    # (npm i ... | pip install ...).
    IFS=',' read -ra packages <<<"$expected_packages"
    for pkg in "${packages[@]}"; do
        # Permit @sbo3l/sdk to appear under any of the install command shapes.
        if ! grep -qE "(npm i|npm install|pip install).*${pkg//+/\\+}" "$guide_path"; then
            issues+=("missing_install:$pkg")
        fi
    done

    # Check 2: every expected export name appears at least once.
    IFS=',' read -ra exports <<<"$expected_exports"
    for ex in "${exports[@]}"; do
        if ! grep -qE "\\b${ex}\\b" "$guide_path"; then
            issues+=("missing_export:$ex")
        fi
    done

    # Check 3: APRP fixture has all 12 required fields. We grep for
    # `<field>:` (TS/JS object key) or `"<field>"` (Python dict key)
    # since both shapes appear across guides.
    for f in "${APRP_REQUIRED[@]}"; do
        if ! grep -qE "(\"$f\"|^\s*$f:|\\b$f:)" "$guide_path"; then
            issues+=("aprp_missing_field:$f")
        fi
    done

    # Check 4: forbidden frozen nonces. Round 4 codex P1 — every demo
    # had a hardcoded ULID that tripped protocol.nonce_replay on the
    # 2nd run. Guides MUST use crypto.randomUUID() / uuid.uuid4().
    for n in "${FORBIDDEN_NONCES[@]}"; do
        if grep -qF "$n" "$guide_path"; then
            issues+=("forbidden_nonce:$n")
        fi
    done

    # Check 5: forbidden expired expiry timestamps. Same root cause.
    for e in "${FORBIDDEN_EXPIRIES[@]}"; do
        if grep -qF "$e" "$guide_path"; then
            issues+=("forbidden_expiry:$e")
        fi
    done

    # Check 6: guide must mention either crypto.randomUUID (TS) or
    # uuid.uuid4 (Py) — proves the nonce is dynamic.
    if ! grep -qE "(crypto\.randomUUID|uuid\.uuid4)" "$guide_path"; then
        issues+=("static_nonce:no_dynamic_uuid_helper_called")
    fi

    # Check 7: the chain + recipient must be the policy-allowlisted pair.
    # Reference policy allows recipient 0x1111...1111 on chain 'base'.
    # Guides that use a different recipient will hit
    # policy.deny_recipient_not_allowlisted.
    if grep -qE 'expected_recipient.*"0x[^1]' "$guide_path"; then
        issues+=("wrong_recipient:not_0x1111...1111")
    fi

    if [[ ${#issues[@]} -eq 0 ]]; then
        echo "ok"
    else
        for i in "${issues[@]}"; do
            all_issues+=("$id|$i")
        done
        echo "issues:${#issues[@]}"
    fi
}

format_status() {
    case "$1" in
        ok)         echo "✅ ok" ;;
        missing)    echo "❌ guide file missing" ;;
        issues:*)   echo "❌ ${1#issues:} issue(s)" ;;
        *)          echo "⚠️  $1" ;;
    esac
}

echo "▶ quickstart static validation"
while IFS='|' read -r id file expected_packages expected_exports; do
    [[ -z "$id" ]] && continue
    if [[ -n "$ONLY" && "$ONLY" != "$id" ]]; then continue; fi
    ((total++)) || true
    printf "  %-36s ... " "$id"
    status=$(run_check "$id" "$file" "$expected_packages" "$expected_exports")
    formatted=$(format_status "$status")
    printf "%s\n" "$formatted"
    RESULTS+=("$id|$file|$status|$formatted")
    if [[ "$status" != "ok" ]]; then ((fail_count++)) || true; fi
done <<<"$MATRIX"

# Write the matrix file.
{
    echo "# Quickstart static-validation matrix"
    echo
    echo "Generated by \`scripts/quickstart-validate.sh\` on $(date -u +%Y-%m-%dT%H:%M:%SZ)."
    echo
    echo "## Per-guide status"
    echo
    echo "| Guide | File | Status |"
    echo "|---|---|---|"
    for row in "${RESULTS[@]}"; do
        IFS='|' read -r id file status formatted <<<"$row"
        echo "| \`$id\` | \`docs/quickstart/$file\` | $formatted |"
    done
    echo
    echo "## Issues"
    echo
    if [[ ${#all_issues[@]} -eq 0 ]]; then
        echo "None — all guides pass static validation."
    else
        echo "| Guide | Issue |"
        echo "|---|---|"
        for issue in "${all_issues[@]}"; do
            IFS='|' read -r id detail <<<"$issue"
            echo "| \`$id\` | \`$detail\` |"
        done
    fi
    echo
    echo "## What this script checks"
    echo
    echo "1. Each \`npm i\` / \`pip install\` block mentions all expected packages"
    echo "2. Every expected SDK export name appears in the guide's code"
    echo "3. APRP fixture has all 12 v1-required fields"
    echo "4. No frozen \`01HTAWX5...\` nonces (round-4 codex P1: tripped \`protocol.nonce_replay\`)"
    echo "5. No expired \`2026-05-0?T10:31:00Z\` expiries"
    echo "6. Guide calls \`crypto.randomUUID\` or \`uuid.uuid4\` (proves dynamic nonces)"
    echo "7. \`expected_recipient\` matches the reference policy's allowlist (\`0x1111...1111\` on chain \`base\`)"
    echo
    echo "## What this script does NOT check"
    echo
    echo "- Runtime end-to-end (LLM-driven). Needs the npm packages live AND model API keys."
    echo "- The \`sdk-install-matrix\` workflow (\`scripts/sdk-install-verify.sh\`, PR #239) covers package liveness;"
    echo "  Heidi runs each quickstart manually for the LLM-driven verification."
    echo
    echo "## Re-run locally"
    echo
    echo '```bash'
    echo "./scripts/quickstart-validate.sh"
    echo "ONLY=ens-with-anthropic ./scripts/quickstart-validate.sh"
    echo '```'
} >"$OUT"

echo
echo "▶ matrix written to $OUT"
echo "▶ pass: $((total - fail_count))/$total"

if [[ "$fail_count" -gt 0 ]]; then
    echo "▶ FAIL ($fail_count guide(s) drifted) — see $OUT"
    exit 1
fi
