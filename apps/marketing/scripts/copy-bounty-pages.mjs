#!/usr/bin/env node
/**
 * Materialise bounty one-pagers into the marketing site's content
 * collection. Source-of-truth: docs/submission/bounty-*.md (owned by
 * QA per #196 submission pack). This script reads each file, extracts
 * the first H1 as the page title, prepends Astro frontmatter, and
 * writes into apps/marketing/src/content/submissions/<slug>.md.
 *
 * Runs in the marketing site's prebuild hook; output is gitignored
 * (regenerated on every build).
 */

import { readdir, readFile, writeFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const REPO_ROOT = resolve(__dirname, "..", "..", "..");
const SRC_DIR = resolve(REPO_ROOT, "docs", "submission");
const OUT_DIR = resolve(REPO_ROOT, "apps", "marketing", "src", "content", "submissions");

function bountyTitle(body) {
  const match = body.match(/^# (.+)$/m);
  return match ? match[1].trim() : "Bounty submission";
}

function bountyAudience(body) {
  // Pulls the audience line if present (the bounty docs use a quoted
  // "**Audience:** ..." line just below the H1).
  const match = body.match(/\*\*Audience:\*\*\s*([^\n]+)/);
  return match ? match[1].replace(/\.$/, "").trim() : undefined;
}

function escapeYaml(value) {
  return value.replace(/"/g, '\\"');
}

async function main() {
  await mkdir(OUT_DIR, { recursive: true });
  const entries = await readdir(SRC_DIR);
  const bounties = entries.filter((f) => f.startsWith("bounty-") && f.endsWith(".md"));
  if (bounties.length === 0) {
    console.warn("No bounty-*.md files in docs/submission/ — skipping.");
    return;
  }

  for (const file of bounties) {
    const slug = file.replace(/^bounty-/, "").replace(/\.md$/, "");
    const body = await readFile(resolve(SRC_DIR, file), "utf8");
    const title = bountyTitle(body);
    const audience = bountyAudience(body);

    // Note: `slug` is reserved in Astro 5 (auto-derived from filename);
    // do NOT include it in frontmatter or the content schema validation
    // fails with InvalidContentEntryDataError. The output filename below
    // (`<slug>.md`) becomes the auto-derived slug.
    const frontmatter =
      `---\n` +
      `title: "${escapeYaml(title)}"\n` +
      (audience ? `audience: "${escapeYaml(audience)}"\n` : "") +
      `source_file: docs/submission/${file}\n` +
      `---\n\n`;

    const out = resolve(OUT_DIR, `${slug}.md`);
    await writeFile(out, frontmatter + body);
    console.log(`→ ${slug}  (${title})`);
  }
}

main().catch((e) => { console.error(e); process.exit(1); });
