#!/usr/bin/env bash
# Generate Python SDK reference HTML from published sbo3l-* packages.
#
# Run locally: bash apps/docs/scripts/gen-sdk-py-refs.sh
# CI: .github/workflows/sdk-refs.yml fires on PyPI publish events.
#
# Output → apps/docs/public/sdk-ref/python/<package>/  (sphinx-build _build/html)
# Wrapper Starlight pages at apps/docs/src/content/docs/reference/sdk-python/
# link out to these via /sdk-ref/python/<package>/.
set -euo pipefail

PACKAGES=(
  "sbo3l-sdk"
  "sbo3l-langchain"
  "sbo3l-crewai"
  "sbo3l-llamaindex"
)

REPO_ROOT="$(git rev-parse --show-toplevel)"
WORK="$(mktemp -d)"
OUT="${REPO_ROOT}/apps/docs/public/sdk-ref/python"
CONFIG_DIR="${REPO_ROOT}/apps/docs/scripts/sphinx"

mkdir -p "$OUT"

# One-shot venv keeps the host system clean.
python3 -m venv "${WORK}/venv"
# shellcheck source=/dev/null
. "${WORK}/venv/bin/activate"
pip install --quiet --upgrade pip
pip install --quiet sphinx furo "${PACKAGES[@]}"

for pkg in "${PACKAGES[@]}"; do
  echo "== ${pkg} =="
  module="${pkg//-/_}"
  build_dir="${WORK}/${pkg}"
  mkdir -p "${build_dir}"

  # Sphinx config + a single autoapi-shaped index.
  cp "${CONFIG_DIR}/conf.py" "${build_dir}/conf.py"
  cat > "${build_dir}/index.rst" <<RST
${pkg}
$(printf '=%.0s' $(seq 1 ${#pkg}))

.. automodule:: ${module}
   :members:
   :undoc-members:
   :show-inheritance:
RST

  sphinx-build -q -b html "${build_dir}" "${OUT}/${pkg}"
done

deactivate
echo "Py SDK refs generated at ${OUT}/"
