#!/usr/bin/env bash
# scripts/sdk-install-verify.sh — verify each published SDK package is
# installable + importable from a clean container.
#
# Catches the "tag exists but package not live" gap that bit us in
# v1.2.0 (NPM_TOKEN missing → 4 npm tags pushed but never published).
#
# For each (package, ecosystem) pair: spin a fresh Docker container,
# `npm install` (or `pip install`) the published package + a smoke
# import. Pass = package resolves AND a top-level export is callable.
# Fail = registry doesn't have the version OR import throws.
#
# Output: a markdown table to docs/proof/sdk-install-matrix.md plus a
# non-zero exit if any package failed (so CI catches regressions).
#
# Usage:
#   ./scripts/sdk-install-verify.sh                # uses 1.2.0 cohort
#   VERSION=1.3.0 ./scripts/sdk-install-verify.sh  # override cohort
#   ONLY=langchain-py ./scripts/sdk-install-verify.sh  # one package
#   SKIP_DOCKER=1 ./scripts/sdk-install-verify.sh  # quick-check (no install)

set -euo pipefail

VERSION="${VERSION:-1.2.0}"
ONLY="${ONLY:-}"
SKIP_DOCKER="${SKIP_DOCKER:-0}"
OUT="${OUT:-docs/proof/sdk-install-matrix.md}"

# Per-package metadata. Edit when adding a new adapter.
# Format: id|ecosystem|pkg|smoke_command_template
#
# smoke_command_template — the shell command to run inside the container
# AFTER install. Use $PKG (substituted at runtime) for the package name.
# Must exit 0 on a working import, non-zero otherwise.
read -r -d '' MATRIX <<'EOF' || true
langchain-py|pypi|sbo3l-langchain|python -c "import sbo3l_langchain; assert callable(sbo3l_langchain.sbo3l_tool)"
crewai-py|pypi|sbo3l-crewai|python -c "import sbo3l_crewai; assert callable(sbo3l_crewai.sbo3l_tool)"
llamaindex-py|pypi|sbo3l-llamaindex|python -c "import sbo3l_llamaindex; assert callable(sbo3l_llamaindex.sbo3l_tool)"
langgraph-py|pypi|sbo3l-langgraph|python -c "import sbo3l_langgraph; assert callable(sbo3l_langgraph.PolicyGuardNode)"
agno-py|pypi|sbo3l-agno|python -c "import sbo3l_agno; assert callable(sbo3l_agno.sbo3l_payment_request_func)"
sdk-py|pypi|sbo3l-sdk|python -c "from sbo3l_sdk import SBO3LClient, SBO3LClientSync; assert SBO3LClient and SBO3LClientSync"
langchain-ts|npm|@sbo3l/langchain|node -e "import('@sbo3l/langchain').then(m => { if (typeof m.sbo3lTool !== 'function') throw new Error('export missing'); })"
autogen|npm|@sbo3l/autogen|node -e "import('@sbo3l/autogen').then(m => { if (typeof m.sbo3lFunction !== 'function') throw new Error('export missing'); })"
elizaos|npm|@sbo3l/elizaos|node -e "import('@sbo3l/elizaos').then(m => { if (typeof m.sbo3lAction !== 'function' && typeof m.sbo3lTool !== 'function') throw new Error('export missing'); })"
vercel-ai|npm|@sbo3l/vercel-ai|node -e "import('@sbo3l/vercel-ai').then(m => { if (typeof m.sbo3lTool !== 'function') throw new Error('export missing'); })"
openai-assistants|npm|@sbo3l/openai-assistants|node -e "import('@sbo3l/openai-assistants').then(m => { if (typeof m.sbo3lAssistantTool !== 'function') throw new Error('export missing'); })"
anthropic|npm|@sbo3l/anthropic|node -e "import('@sbo3l/anthropic').then(m => { if (typeof m.sbo3lTool !== 'function') throw new Error('export missing'); })"
anthropic-computer-use|npm|@sbo3l/anthropic-computer-use|node -e "import('@sbo3l/anthropic-computer-use').then(m => { if (typeof m.gateComputerAction !== 'function') throw new Error('export missing'); })"
mastra|npm|@sbo3l/mastra|node -e "import('@sbo3l/mastra').then(m => { if (typeof m.sbo3lTool !== 'function') throw new Error('export missing'); })"
vellum|npm|@sbo3l/vellum|node -e "import('@sbo3l/vellum').then(m => { if (typeof m.sbo3lTool !== 'function') throw new Error('export missing'); })"
sdk-ts|npm|@sbo3l/sdk|node -e "import('@sbo3l/sdk').then(m => { if (typeof m.SBO3LClient !== 'function') throw new Error('export missing'); })"
EOF

if [[ "$SKIP_DOCKER" != "1" ]]; then
    if ! command -v docker >/dev/null 2>&1; then
        echo "ERROR: docker not found. Install Docker or set SKIP_DOCKER=1 for a registry-only check." >&2
        exit 2
    fi
fi

declare -a RESULTS=()

run_check() {
    local id="$1" ecosystem="$2" pkg="$3" smoke="$4"

    if [[ "$SKIP_DOCKER" == "1" ]]; then
        # Registry-only: HEAD the version's JSON metadata URL on the
        # public registry. Avoids `pip index` (subject to PyPI's tight
        # unauthenticated rate limit — bursts of >5 calls flake) and
        # `npm view` (slower and re-resolves the dependency tree).
        # Cheap shape check; useful in tight CI budgets.
        local url
        if [[ "$ecosystem" == "npm" ]]; then
            # npm scoped packages need the / encoded as %2f.
            local encoded="${pkg/\//%2f}"
            url="https://registry.npmjs.org/${encoded}/${VERSION}"
        else
            url="https://pypi.org/pypi/${pkg}/${VERSION}/json"
        fi
        local code
        code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 10 "$url" || echo "000")
        case "$code" in
            200) echo "ok" ;;
            404) echo "registry_miss" ;;
            *)   echo "registry_error_$code" ;;
        esac
        return
    fi

    # Full install + import via Docker.
    local image cmd
    if [[ "$ecosystem" == "npm" ]]; then
        image="node:20-bookworm-slim"
        # The smoke template uses literal $PKG; substitute now.
        local smoke_resolved="${smoke//\$PKG/$pkg}"
        cmd="set -e
mkdir /work && cd /work
echo '{\"type\":\"module\"}' > package.json
npm install --no-audit --no-fund ${pkg}@${VERSION} >/tmp/install.log 2>&1 || { echo INSTALL_FAIL; tail -20 /tmp/install.log >&2; exit 1; }
${smoke_resolved} || { echo IMPORT_FAIL; exit 1; }
echo OK"
    else
        image="python:3.12-slim-bookworm"
        local smoke_resolved="${smoke//\$PKG/$pkg}"
        cmd="set -e
pip install --quiet ${pkg}==${VERSION} >/tmp/install.log 2>&1 || { echo INSTALL_FAIL; tail -20 /tmp/install.log >&2; exit 1; }
${smoke_resolved} || { echo IMPORT_FAIL; exit 1; }
echo OK"
    fi

    # Run, capture last line of stdout.
    local out
    if out=$(docker run --rm --network bridge "$image" sh -c "$cmd" 2>&1); then
        if grep -q '^OK$' <<<"$out"; then
            echo "ok"
        else
            echo "unknown"
        fi
    else
        if grep -q '^INSTALL_FAIL$' <<<"$out"; then
            echo "install_fail"
        elif grep -q '^IMPORT_FAIL$' <<<"$out"; then
            echo "import_fail"
        else
            echo "container_error"
        fi
    fi
}

format_status() {
    case "$1" in
        ok) echo "✅ live" ;;
        registry_miss) echo "❌ not on registry" ;;
        install_fail) echo "❌ install fail" ;;
        import_fail) echo "❌ import fail" ;;
        container_error) echo "❌ container error" ;;
        *) echo "⚠️  $1" ;;
    esac
}

mkdir -p "$(dirname "$OUT")"

# Run.
echo "▶ install-verify (version=$VERSION, mode=$([[ "$SKIP_DOCKER" == "1" ]] && echo registry || echo docker))"
fail_count=0
total=0
while IFS='|' read -r id ecosystem pkg smoke; do
    [[ -z "$id" ]] && continue
    if [[ -n "$ONLY" && "$ONLY" != "$id" ]]; then continue; fi
    ((total++)) || true
    printf "  %-26s %-4s %-32s ... " "$id" "$ecosystem" "$pkg"
    status=$(run_check "$id" "$ecosystem" "$pkg" "$smoke")
    formatted=$(format_status "$status")
    printf "%s\n" "$formatted"
    RESULTS+=("$id|$ecosystem|$pkg|$status|$formatted")
    if [[ "$status" != "ok" ]]; then ((fail_count++)) || true; fi
done <<<"$MATRIX"

# Write the matrix file.
{
    echo "# SDK install matrix (v${VERSION})"
    echo
    echo "Generated by \`scripts/sdk-install-verify.sh\` on $(date -u +%Y-%m-%dT%H:%M:%SZ)."
    echo
    echo "Mode: **$([[ "$SKIP_DOCKER" == "1" ]] && echo registry-probe || echo docker-install-and-import)**"
    echo
    echo "| Package | Ecosystem | Status | Why |"
    echo "|---|---|---|---|"
    for row in "${RESULTS[@]}"; do
        IFS='|' read -r id ecosystem pkg status formatted <<<"$row"
        case "$status" in
            ok)              why="\`${pkg}@${VERSION}\` installs and imports cleanly" ;;
            registry_miss)   why="version ${VERSION} not yet on registry — see \`docs/release/v1.2.0-recovery-runbook.md\`" ;;
            install_fail)    why="dependency resolution failed inside container" ;;
            import_fail)     why="installs but expected top-level export missing" ;;
            container_error) why="docker run errored — see CI logs" ;;
            *)               why="—" ;;
        esac
        echo "| \`$pkg\` | $ecosystem | $formatted | $why |"
    done
    echo
    echo "## Summary"
    echo
    pass_count=$((total - fail_count))
    echo "- ✅ live: **${pass_count}/${total}**"
    echo "- ❌ failing: **${fail_count}/${total}**"
    if [[ "$fail_count" -gt 0 ]]; then
        echo
        echo "**Recovery:** see [\`docs/release/v1.2.0-recovery-runbook.md\`](../release/v1.2.0-recovery-runbook.md) for the unblock steps (NPM_TOKEN, PyPI trusted publishers, \`gh workflow run\` re-fires)."
    fi
    echo
    echo "## Re-run locally"
    echo
    echo '```bash'
    echo "./scripts/sdk-install-verify.sh                  # full docker matrix"
    echo "SKIP_DOCKER=1 ./scripts/sdk-install-verify.sh    # quick registry probe (no install)"
    echo "ONLY=langchain-py ./scripts/sdk-install-verify.sh  # one package"
    echo "VERSION=1.3.0 ./scripts/sdk-install-verify.sh    # next cohort"
    echo '```'
} >"$OUT"

echo
echo "▶ matrix written to $OUT"
echo "▶ pass: $((total - fail_count))/$total"

if [[ "$fail_count" -gt 0 ]]; then
    echo "▶ FAIL ($fail_count package(s) not live) — check $OUT for the per-package status"
    exit 1
fi
