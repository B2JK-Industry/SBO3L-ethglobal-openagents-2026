#!/usr/bin/env bash
# R14 P2 — reproducible competitive-benchmarks runner.
#
# Runs the full criterion suite at benchmarks/competitive/ and emits a
# machine-readable summary (estimates.json per bench function) plus a
# single-file rollup at benchmarks/competitive/results-<host>-<date>.json.
#
# Usage:
#   ./scripts/run-competitive-benchmarks.sh           # full run
#   ./scripts/run-competitive-benchmarks.sh quick     # 5s/sample (smoke)
#   ./scripts/run-competitive-benchmarks.sh policy    # filter to "policy"
#
# Output:
#   benchmarks/competitive/target/criterion/<bench>/<func>/base/estimates.json
#   benchmarks/competitive/results-$(hostname)-$(date +%Y-%m-%d).json
#   benchmarks/competitive/target/criterion/report/index.html
#
# Reproducibility checklist (baked into the rollup):
#   - Hostname + uname -a + lscpu/sysctl -a (CPU model + freq)
#   - Rust toolchain version (rustc --version)
#   - cargo lockfile sha256
#   - Hostname + date + git HEAD

set -euo pipefail

cd "$(dirname "$0")/.."

MODE="${1:-full}"
FILTER=""
case "$MODE" in
    quick)
        export CRITERION_FAST=1
        echo "[bench] quick mode: ~5s/sample"
        ;;
    full)
        echo "[bench] full mode: ~10s/sample, 100 samples per fn"
        ;;
    *)
        FILTER="$MODE"
        echo "[bench] filter mode: matching '$FILTER'"
        ;;
esac

cd benchmarks/competitive

# ---- Rig fingerprint ----
HOSTNAME=$(hostname)
DATE=$(date +%Y-%m-%d)
SUFFIX="${HOSTNAME}-${DATE}"
ROLLUP="results-${SUFFIX}.json"

UNAME=$(uname -a)
RUSTC_VERSION=$(rustc --version)
GIT_HEAD=$(git rev-parse HEAD 2>/dev/null || echo "(not in git)")
CARGO_LOCK_HASH=$(sha256sum Cargo.toml 2>/dev/null | awk '{print $1}' || shasum -a 256 Cargo.toml | awk '{print $1}')

# CPU info — best-effort cross-platform.
if [[ "$(uname -s)" == "Darwin" ]]; then
    CPU_MODEL=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "unknown")
    CPU_CORES=$(sysctl -n hw.ncpu)
else
    CPU_MODEL=$(grep 'model name' /proc/cpuinfo | head -1 | sed 's/.*: //' || echo "unknown")
    CPU_CORES=$(nproc 2>/dev/null || echo "unknown")
fi

# Memory total in GB.
if [[ "$(uname -s)" == "Darwin" ]]; then
    MEM_GB=$(echo "$(sysctl -n hw.memsize) / 1024 / 1024 / 1024" | bc 2>/dev/null || echo "unknown")
else
    MEM_GB=$(free -g | awk '/^Mem:/{print $2}' 2>/dev/null || echo "unknown")
fi

echo "[bench] rig fingerprint:"
echo "  host       : $HOSTNAME"
echo "  date       : $DATE"
echo "  uname      : $UNAME"
echo "  cpu model  : $CPU_MODEL"
echo "  cpu cores  : $CPU_CORES"
echo "  memory GB  : $MEM_GB"
echo "  rustc      : $RUSTC_VERSION"
echo "  git HEAD   : $GIT_HEAD"
echo

# ---- Run criterion ----
if [[ -n "$FILTER" ]]; then
    cargo bench -- "$FILTER"
else
    cargo bench
fi

# ---- Aggregate results ----
echo
echo "[bench] aggregating estimates.json files..."

python3 - <<PY
import json
import os
from glob import glob

rollup = {
    "rig": {
        "host": "$HOSTNAME",
        "date": "$DATE",
        "uname": """$UNAME""",
        "cpu_model": "$CPU_MODEL",
        "cpu_cores": "$CPU_CORES",
        "memory_gb": "$MEM_GB",
        "rustc_version": """$RUSTC_VERSION""",
        "cargo_toml_hash": "$CARGO_LOCK_HASH",
        "git_head": "$GIT_HEAD",
    },
    "results": {},
}

for est_path in sorted(glob("target/criterion/*/*/base/estimates.json")):
    parts = est_path.split("/")
    bench_group = parts[2]
    bench_fn = parts[3]
    with open(est_path) as f:
        data = json.load(f)
    mean_ns = data.get("mean", {}).get("point_estimate")
    median_ns = data.get("median", {}).get("point_estimate")
    if mean_ns is None:
        continue
    key = f"{bench_group}/{bench_fn}"
    rollup["results"][key] = {
        "mean_ns": mean_ns,
        "median_ns": median_ns,
        "ops_per_sec_mean": int(1e9 / mean_ns) if mean_ns > 0 else None,
    }

out_path = "$ROLLUP"
with open(out_path, "w") as f:
    json.dump(rollup, f, indent=2)
print(f"[bench] wrote {out_path}")
print()
print("[bench] summary:")
for key, v in sorted(rollup["results"].items()):
    if v["ops_per_sec_mean"]:
        print(f"  {key:60s} {v['mean_ns']:>10.1f} ns  ({v['ops_per_sec_mean']:>10,} ops/sec)")
PY

echo
echo "[bench] HTML report: open benchmarks/competitive/target/criterion/report/index.html"
echo "[bench] rollup JSON: benchmarks/competitive/$ROLLUP"
echo
echo "Next: paste the rollup into docs/proof/competitive-benchmarks.md"
