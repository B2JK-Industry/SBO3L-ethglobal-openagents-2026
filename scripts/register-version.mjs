#!/usr/bin/env node
/**
 * Append a new tagged version to apps/docs/src/data/versions.json.
 *
 * Usage:
 *   node scripts/register-version.mjs v1.3.0
 *
 * Idempotent — if the tag is already in the registry, exits with rc=0
 * and no change. Used by .github/workflows/build-versioned-docs.yml on
 * tag push.
 *
 * Today's date provider lets the workflow inject GITHUB_RUN_TIMESTAMP
 * if it wants a deterministic date; otherwise we use the current
 * UTC date (YYYY-MM-DD).
 */

import { readFile, writeFile } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REGISTRY = resolve(__dirname, "..", "apps", "docs", "src", "data", "versions.json");

function todayUTC() {
  const d = new Date();
  const yyyy = d.getUTCFullYear();
  const mm = String(d.getUTCMonth() + 1).padStart(2, "0");
  const dd = String(d.getUTCDate()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

async function main() {
  const tag = process.argv[2];
  if (!tag || !tag.startsWith("v")) {
    console.error(`usage: ${process.argv[1]} v<semver>   (got: ${tag ?? "(none)"})`);
    process.exit(2);
  }

  const raw = await readFile(REGISTRY, "utf8");
  const json = JSON.parse(raw);

  if (json.versions.some((v) => v.id === tag || v.tag === tag)) {
    console.log(`${tag} already registered — no change.`);
    return;
  }

  const entry = {
    id: tag,
    label: tag,
    tag,
    date: todayUTC(),
    default: false,
    description: `Auto-registered on tag push at ${todayUTC()}.`,
  };

  // Insert just below "latest" so the dropdown ordering stays
  // newest-first.
  const latestIdx = json.versions.findIndex((v) => v.id === "latest");
  if (latestIdx >= 0) {
    json.versions.splice(latestIdx + 1, 0, entry);
  } else {
    json.versions.unshift(entry);
  }

  await writeFile(REGISTRY, JSON.stringify(json, null, 2) + "\n");
  console.log(`Registered ${tag} in ${REGISTRY}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
