#!/usr/bin/env bash
#
# T-3-3 / T-3-4 — register an SBO3L agent fleet via Durin under
# `sbo3lagent.eth`. Reads a YAML config (scripts/fleet-config/*.yaml),
# derives deterministic Ed25519 pubkeys for each agent, builds the
# Durin issuance calldata via `sbo3l agent register --dry-run`, then
# broadcasts via `cast send` against the operator's RPC + key.
#
# Why two-phase (dry-run + cast send)? The dry-run output is
# byte-identical across operators — anyone with the same YAML
# re-derives the same calldata. The broadcast is the only operator-
# specific step. This keeps the issuance pipeline auditable: the
# manifest committed to the repo records both the calldata bytes and
# the tx hash, so reviewers can re-derive calldata locally and
# compare against what landed on-chain.
#
# This script ALSO works once Dev 1's T-3-1 broadcast slice ships
# (sbo3l agent register --broadcast). At that point the cast-send
# fallback can be deleted; the script structure stays the same.
#
# Usage:
#   export SEPOLIA_RPC_URL=https://sepolia.alchemy.com/v2/...
#   export SBO3L_DEPLOYER_PRIVATE_KEY=0x...
#   ./scripts/register-fleet.sh scripts/fleet-config/agents-5.yaml
#
# Output:
#   docs/proof/ens-fleet-<date>.json   manifest with all agents + tx hashes
#
# Exit codes:
#   0  success — all agents registered, manifest written
#   1  IO / shell error
#   2  config / env-var validation error
#   3  partial failure — some agents registered, some failed (manifest
#      records both)

set -euo pipefail

CONFIG_PATH="${1:-}"
if [ -z "$CONFIG_PATH" ]; then
    echo "ERROR: usage: $0 <config-yaml>" >&2
    echo "       e.g.   $0 scripts/fleet-config/agents-5.yaml" >&2
    exit 2
fi

if [ ! -f "$CONFIG_PATH" ]; then
    echo "ERROR: config not found: $CONFIG_PATH" >&2
    exit 2
fi

# ---- Env validation ------------------------------------------------

require_env() {
    if [ -z "${!1:-}" ]; then
        echo "ERROR: $1 must be set." >&2
        exit 2
    fi
}

NETWORK=$(python3 -c "import sys, yaml; c = yaml.safe_load(open(sys.argv[1])); print(c.get('network', 'sepolia'))" "$CONFIG_PATH")

case "$NETWORK" in
    sepolia)
        require_env SEPOLIA_RPC_URL
        RPC_URL="$SEPOLIA_RPC_URL"
        ETHERSCAN_BASE="https://sepolia.etherscan.io/tx"
        ;;
    mainnet)
        if [ "${SBO3L_ALLOW_MAINNET_TX:-}" != "1" ]; then
            cat >&2 <<EOF
ERROR: refusing --network mainnet without SBO3L_ALLOW_MAINNET_TX=1.

Mainnet fleet registration is gas-bearing on Daniel's wallet
(~\$5-10 per agent at 50 gwei × N agents). Set SBO3L_ALLOW_MAINNET_TX=1
to acknowledge before re-running. Default network is sepolia.
EOF
            exit 2
        fi
        require_env MAINNET_RPC_URL
        RPC_URL="$MAINNET_RPC_URL"
        ETHERSCAN_BASE="https://etherscan.io/tx"
        ;;
    *)
        echo "ERROR: unknown network: $NETWORK" >&2
        exit 2
        ;;
esac

require_env SBO3L_DEPLOYER_PRIVATE_KEY

if ! command -v cast >/dev/null 2>&1; then
    echo "ERROR: \`cast\` (Foundry) not on PATH. Install via foundryup." >&2
    exit 2
fi

if ! command -v sbo3l >/dev/null 2>&1; then
    # Fall back to cargo-installed binary in workspace target/release.
    if [ -x ./target/release/sbo3l ]; then
        SBO3L_BIN="./target/release/sbo3l"
    elif [ -x ./target/debug/sbo3l ]; then
        SBO3L_BIN="./target/debug/sbo3l"
    else
        echo "ERROR: \`sbo3l\` not on PATH and no built binary found." >&2
        echo "       Run \`cargo build --release -p sbo3l-cli\` first." >&2
        exit 2
    fi
else
    SBO3L_BIN="sbo3l"
fi

# ---- Derive pubkeys ------------------------------------------------

PUBKEYS_FILE="$(mktemp)"
trap 'rm -f "$PUBKEYS_FILE"' EXIT

python3 scripts/derive-fleet-keys.py \
    --config "$CONFIG_PATH" \
    --output-pubkeys "$PUBKEYS_FILE" >/dev/null

# ---- Per-agent registration loop -----------------------------------

# Manifest filename = ens-fleet-${CONFIG_BASE}-${DATE}.json so two
# fleets registered on the same day (e.g. agents-5.yaml and
# agents-60.yaml) don't collide on a single output path. The
# CONFIG_BASE comes from the YAML basename (`agents-5`, `agents-60`,
# whatever the operator named it) — keeps the path self-describing
# without requiring an extra flag. (codex P1 fix on #138 + #173.)
DATE_TAG=$(date -u +%Y-%m-%d)
CONFIG_BASE=$(basename "$CONFIG_PATH" .yaml)
MANIFEST_PATH="docs/proof/ens-fleet-${CONFIG_BASE}-${DATE_TAG}.json"
mkdir -p "$(dirname "$MANIFEST_PATH")"

# Build the manifest skeleton with python (richer JSON than bash heredoc).
python3 - "$CONFIG_PATH" "$PUBKEYS_FILE" "$MANIFEST_PATH" "$NETWORK" <<'PY'
import json, sys, datetime, pathlib
config_path, pubkeys_path, manifest_path, network = sys.argv[1:]
import yaml
cfg = yaml.safe_load(pathlib.Path(config_path).read_text())
pubkeys = json.loads(pathlib.Path(pubkeys_path).read_text())["agents"]
manifest = {
    "schema": "sbo3l.ens_fleet_manifest.v1",
    "generated_at_utc": datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ"),
    "network": network,
    "parent": cfg["parent"],
    "seed_doc": cfg["seed_doc"],
    "agents": [],
    "totals": {
        "agent_count": len(cfg["agents"]),
        "succeeded": 0,
        "failed": 0,
        "gas_used_total": None,
    },
}
for entry in cfg["agents"]:
    label = entry["label"]
    fqdn = f"{label}.{cfg['parent']}"
    manifest["agents"].append({
        "label": label,
        "fqdn": fqdn,
        "agent_id": entry["agent_id"],
        "endpoint": entry["endpoint"],
        "policy_url": entry["policy_url"],
        "capabilities": entry["capabilities"],
        "pubkey_ed25519": pubkeys[label],
        # Filled in by the broadcast loop below.
        "register_tx_hash": None,
        "multicall_tx_hash": None,
        "etherscan": None,
        "status": "pending",
        "error": None,
    })
pathlib.Path(manifest_path).write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
print(f"manifest skeleton written to {manifest_path}", file=sys.stderr)
PY

echo
echo "==> Network:    $NETWORK"
echo "==> Config:     $CONFIG_PATH"
echo "==> Manifest:   $MANIFEST_PATH"
echo "==> Agents:     $(python3 -c "import yaml,sys; print(len(yaml.safe_load(open(sys.argv[1]))['agents']))" "$CONFIG_PATH")"
echo

# ---- Broadcast loop -----------------------------------------------
#
# For each agent:
#   1. sbo3l agent register --dry-run --records '...'  → calldata
#   2. cast send <durin_registrar> <register_calldata>  → register_tx_hash
#   3. cast send <public_resolver> <multicall_calldata> → multicall_tx_hash
#   4. Update manifest entry.

agent_count=$(python3 -c "import yaml,sys; print(len(yaml.safe_load(open(sys.argv[1]))['agents']))" "$CONFIG_PATH")
i=0
while [ "$i" -lt "$agent_count" ]; do
    LABEL=$(python3 -c "import yaml,sys; print(yaml.safe_load(open(sys.argv[1]))['agents'][int(sys.argv[2])]['label'])" "$CONFIG_PATH" "$i")
    AGENT_ID=$(python3 -c "import yaml,sys; print(yaml.safe_load(open(sys.argv[1]))['agents'][int(sys.argv[2])]['agent_id'])" "$CONFIG_PATH" "$i")
    ENDPOINT=$(python3 -c "import yaml,sys; print(yaml.safe_load(open(sys.argv[1]))['agents'][int(sys.argv[2])]['endpoint'])" "$CONFIG_PATH" "$i")
    POLICY_URL=$(python3 -c "import yaml,sys; print(yaml.safe_load(open(sys.argv[1]))['agents'][int(sys.argv[2])]['policy_url'])" "$CONFIG_PATH" "$i")
    CAPABILITIES_JSON=$(python3 -c "import yaml,sys,json; print(json.dumps(yaml.safe_load(open(sys.argv[1]))['agents'][int(sys.argv[2])]['capabilities']))" "$CONFIG_PATH" "$i")
    PUBKEY=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['agents'][sys.argv[2]])" "$PUBKEYS_FILE" "$LABEL")

    PARENT=$(python3 -c "import yaml,sys; print(yaml.safe_load(open(sys.argv[1]))['parent'])" "$CONFIG_PATH")
    OWNER=$(cast wallet address "$SBO3L_DEPLOYER_PRIVATE_KEY")

    RECORDS_JSON=$(python3 -c "
import json, sys
print(json.dumps({
    'sbo3l:agent_id':       sys.argv[1],
    'sbo3l:endpoint':       sys.argv[2],
    'sbo3l:pubkey_ed25519': sys.argv[3],
    'sbo3l:policy_url':     sys.argv[4],
    'sbo3l:capabilities':   sys.argv[5],
}))" "$AGENT_ID" "$ENDPOINT" "$PUBKEY" "$POLICY_URL" "$CAPABILITIES_JSON")

    echo "==> [$((i+1))/$agent_count] $LABEL.$PARENT"
    echo "    pubkey (Ed25519): $PUBKEY"

    DRY_RUN_OUT=$(mktemp)
    # Defense-in-depth: pass --dry-run explicitly even though it's
    # the CLI default. clap's conflicts_with rejects --broadcast in
    # the same call, so a future flip of the CLI default cannot
    # silently turn this envelope-build invocation into a real tx.
    # (codex P1 fix on #138.)
    if ! "$SBO3L_BIN" agent register \
            --name "$LABEL" \
            --parent "$PARENT" \
            --network "$NETWORK" \
            --records "$RECORDS_JSON" \
            --owner "$OWNER" \
            --dry-run \
            --out "$DRY_RUN_OUT" >/dev/null 2>&1; then
        echo "    ERROR: dry-run failed for $LABEL" >&2
        python3 -c "
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
m = json.loads(p.read_text())
m['agents'][int(sys.argv[2])]['status'] = 'dry_run_failed'
m['agents'][int(sys.argv[2])]['error'] = 'sbo3l agent register --dry-run failed'
m['totals']['failed'] += 1
p.write_text(json.dumps(m, indent=2, sort_keys=True) + '\n')
" "$MANIFEST_PATH" "$i"
        i=$((i+1))
        continue
    fi

    REGISTER_CALLDATA=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['register_calldata_hex'])" "$DRY_RUN_OUT")
    MULTICALL_CALLDATA=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['multicall_calldata_hex'])" "$DRY_RUN_OUT")
    DURIN_REGISTRAR=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1])).get('durin_registrar', ''))" "$DRY_RUN_OUT")
    PUBLIC_RESOLVER=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1])).get('resolver', ''))" "$DRY_RUN_OUT")
    rm -f "$DRY_RUN_OUT"

    # If the dry-run envelope doesn't carry durin_registrar (T-3-1 main
    # PR scope predates the broadcast slice that adds it), the
    # operator must supply DURIN_REGISTRAR via env. Same for
    # PUBLIC_RESOLVER if not present.
    if [ -z "$DURIN_REGISTRAR" ]; then
        DURIN_REGISTRAR="${DURIN_REGISTRAR_ADDR:-}"
    fi
    if [ -z "$DURIN_REGISTRAR" ]; then
        echo "    ERROR: dry-run envelope missing durin_registrar AND DURIN_REGISTRAR_ADDR env unset" >&2
        echo "           Set DURIN_REGISTRAR_ADDR=0x... and re-run." >&2
        exit 2
    fi
    if [ -z "$PUBLIC_RESOLVER" ]; then
        echo "    ERROR: dry-run envelope missing resolver address" >&2
        exit 2
    fi

    echo "    register_calldata: ${REGISTER_CALLDATA:0:18}…  → $DURIN_REGISTRAR"
    echo "    multicall_calldata: ${MULTICALL_CALLDATA:0:18}…  → $PUBLIC_RESOLVER"

    REGISTER_TX=$(cast send "$DURIN_REGISTRAR" "$REGISTER_CALLDATA" \
        --rpc-url "$RPC_URL" \
        --private-key "$SBO3L_DEPLOYER_PRIVATE_KEY" \
        --json 2>&1 | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d.get('transactionHash',''))" || echo "")

    if [ -z "$REGISTER_TX" ]; then
        echo "    ERROR: register tx broadcast failed for $LABEL" >&2
        python3 -c "
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
m = json.loads(p.read_text())
m['agents'][int(sys.argv[2])]['status'] = 'register_failed'
m['agents'][int(sys.argv[2])]['error'] = 'cast send (register) returned no tx hash'
m['totals']['failed'] += 1
p.write_text(json.dumps(m, indent=2, sort_keys=True) + '\n')
" "$MANIFEST_PATH" "$i"
        i=$((i+1))
        continue
    fi

    echo "    register_tx_hash:  $REGISTER_TX"

    MULTICALL_TX=$(cast send "$PUBLIC_RESOLVER" "$MULTICALL_CALLDATA" \
        --rpc-url "$RPC_URL" \
        --private-key "$SBO3L_DEPLOYER_PRIVATE_KEY" \
        --json 2>&1 | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d.get('transactionHash',''))" || echo "")

    if [ -z "$MULTICALL_TX" ]; then
        echo "    WARNING: multicall tx broadcast failed for $LABEL" >&2
        python3 -c "
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
m = json.loads(p.read_text())
m['agents'][int(sys.argv[2])]['status'] = 'partial_register_only'
m['agents'][int(sys.argv[2])]['register_tx_hash'] = sys.argv[3]
m['agents'][int(sys.argv[2])]['etherscan'] = sys.argv[4] + '/' + sys.argv[3]
m['agents'][int(sys.argv[2])]['error'] = 'multicall (setText) tx failed; subname registered but records empty'
m['totals']['failed'] += 1
p.write_text(json.dumps(m, indent=2, sort_keys=True) + '\n')
" "$MANIFEST_PATH" "$i" "$REGISTER_TX" "$ETHERSCAN_BASE"
        i=$((i+1))
        continue
    fi

    echo "    multicall_tx_hash: $MULTICALL_TX"

    python3 -c "
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
m = json.loads(p.read_text())
m['agents'][int(sys.argv[2])]['status'] = 'success'
m['agents'][int(sys.argv[2])]['register_tx_hash'] = sys.argv[3]
m['agents'][int(sys.argv[2])]['multicall_tx_hash'] = sys.argv[4]
m['agents'][int(sys.argv[2])]['etherscan'] = sys.argv[5] + '/' + sys.argv[3]
m['totals']['succeeded'] += 1
p.write_text(json.dumps(m, indent=2, sort_keys=True) + '\n')
" "$MANIFEST_PATH" "$i" "$REGISTER_TX" "$MULTICALL_TX" "$ETHERSCAN_BASE"

    i=$((i+1))
done

echo
echo "==================================================================="
SUCCEEDED=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['totals']['succeeded'])" "$MANIFEST_PATH")
FAILED=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['totals']['failed'])" "$MANIFEST_PATH")
echo "  TOTAL: $agent_count agents  |  SUCCEEDED: $SUCCEEDED  |  FAILED: $FAILED"
echo "  MANIFEST: $MANIFEST_PATH"
echo "==================================================================="

if [ "$FAILED" -gt 0 ]; then
    exit 3
fi
exit 0
