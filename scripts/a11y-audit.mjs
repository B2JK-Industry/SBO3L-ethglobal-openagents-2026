#!/usr/bin/env node
/**
 * Accessibility audit using axe-core via Playwright.
 *
 * Walks every URL in TARGETS in headless Chromium, injects axe-core,
 * runs WCAG 2.1 Level AA rules, writes one JSON per URL into
 * docs/submission/a11y-reports/. Fails (rc=1) on any violation.
 *
 * Usage:
 *   npm install --no-save playwright @axe-core/playwright
 *   npx playwright install chromium
 *   node scripts/a11y-audit.mjs
 */

import { writeFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const OUT_DIR = resolve(__dirname, "..", "docs", "submission", "a11y-reports");

const TARGETS = [
  "https://sbo3l-marketing.vercel.app/",
  "https://sbo3l-marketing.vercel.app/features",
  "https://sbo3l-marketing.vercel.app/proof",
  "https://sbo3l-marketing.vercel.app/submission",
  "https://sbo3l-marketing.vercel.app/demo",
  "https://sbo3l-docs.vercel.app/",
  "https://sbo3l-docs.vercel.app/quickstart",
  "https://sbo3l-docs.vercel.app/concepts/trust-dns",
];

async function main() {
  const { chromium } = await import("playwright");
  const { default: AxeBuilder } = await import("@axe-core/playwright");

  await mkdir(OUT_DIR, { recursive: true });

  const browser = await chromium.launch();
  const failures = [];

  try {
    for (const url of TARGETS) {
      process.stdout.write(`→ ${url} ... `);
      const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
      const page = await ctx.newPage();
      await page.goto(url, { waitUntil: "networkidle" });

      const results = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
        .analyze();

      const id = new URL(url).pathname.replace(/\W+/g, "-").replace(/^-|-$/g, "") || "root";
      const host = new URL(url).host.replace(/\W+/g, "-");
      const out = resolve(OUT_DIR, `${host}.${id}.json`);
      await writeFile(out, JSON.stringify({ url, violations: results.violations, fetched_at: new Date().toISOString() }, null, 2));

      if (results.violations.length > 0) {
        failures.push({ url, violations: results.violations.map((v) => ({ id: v.id, impact: v.impact, count: v.nodes.length })) });
        process.stdout.write(`${results.violations.length} violations  ✗\n`);
      } else {
        process.stdout.write("0 violations  ✓\n");
      }
      await ctx.close();
    }
  } finally {
    await browser.close();
  }

  if (failures.length > 0) {
    console.error(`\n${failures.length} URL(s) with violations:`);
    for (const f of failures) {
      console.error(`  ${f.url}`);
      for (const v of f.violations) console.error(`    ${v.id} [${v.impact}] × ${v.count}`);
    }
    process.exit(1);
  }
  console.log("\nAll targets pass WCAG 2.1 AA.");
}

main().catch((e) => { console.error(e); process.exit(1); });
