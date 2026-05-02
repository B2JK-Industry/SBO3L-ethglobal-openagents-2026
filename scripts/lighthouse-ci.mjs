#!/usr/bin/env node
/**
 * Lighthouse CI runner for the deployed surfaces.
 *
 * Runs Lighthouse against every URL in TARGETS at both desktop and
 * mobile presets. Writes one JSON report per (URL, preset) pair into
 * docs/submission/lighthouse-reports/. Fails (rc=1) when any score
 * falls below MIN_SCORE.
 *
 * Usage:
 *   npm install --no-save lighthouse chrome-launcher
 *   node scripts/lighthouse-ci.mjs [--preset=desktop|mobile|both]
 *
 * CI: .github/workflows/lighthouse.yml runs this on every PR touching
 * apps/* paths (matrix over the 4 deployed surfaces × 2 presets).
 */

import { writeFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const REPO_ROOT = resolve(__dirname, "..");
const OUT_DIR = resolve(REPO_ROOT, "docs", "submission", "lighthouse-reports");

const TARGETS = [
  { id: "marketing-home",      url: "https://sbo3l-marketing.vercel.app/" },
  { id: "marketing-features",  url: "https://sbo3l-marketing.vercel.app/features" },
  { id: "marketing-proof",     url: "https://sbo3l-marketing.vercel.app/proof" },
  { id: "marketing-submission", url: "https://sbo3l-marketing.vercel.app/submission" },
  { id: "marketing-demo",       url: "https://sbo3l-marketing.vercel.app/demo" },
  { id: "docs-home",           url: "https://sbo3l-docs.vercel.app/" },
  { id: "docs-quickstart",     url: "https://sbo3l-docs.vercel.app/quickstart" },
  { id: "docs-trust-dns",      url: "https://sbo3l-docs.vercel.app/concepts/trust-dns" },
];

const PRESETS = process.argv.includes("--preset=desktop")
  ? ["desktop"]
  : process.argv.includes("--preset=mobile")
  ? ["mobile"]
  : ["desktop", "mobile"];

const MIN_SCORE = 0.9;
const CATEGORIES = ["performance", "accessibility", "best-practices", "seo"];

async function main() {
  const lighthouse = (await import("lighthouse")).default;
  const chromeLauncher = await import("chrome-launcher");
  await mkdir(OUT_DIR, { recursive: true });

  const chrome = await chromeLauncher.launch({ chromeFlags: ["--headless=new"] });
  const failures = [];

  try {
    for (const target of TARGETS) {
      for (const preset of PRESETS) {
        process.stdout.write(`→ ${target.id}  (${preset}) ... `);
        const result = await lighthouse(target.url, {
          port: chrome.port,
          formFactor: preset === "mobile" ? "mobile" : "desktop",
          screenEmulation: preset === "mobile" ? undefined : { mobile: false, width: 1350, height: 940, deviceScaleFactor: 1, disabled: false },
          throttlingMethod: "simulate",
          onlyCategories: CATEGORIES,
        });

        const lh = result.lhr;
        const scores = Object.fromEntries(CATEGORIES.map((c) => [c, lh.categories[c]?.score ?? 0]));
        const summary = Object.entries(scores).map(([k, v]) => `${k}=${(v * 100).toFixed(0)}`).join(" ");
        const minBelow = Object.entries(scores).filter(([, v]) => v < MIN_SCORE);
        if (minBelow.length > 0) failures.push({ target, preset, scores });

        const out = resolve(OUT_DIR, `${target.id}.${preset}.json`);
        await writeFile(out, JSON.stringify({ url: target.url, preset, scores, fetched_at: new Date().toISOString() }, null, 2));
        process.stdout.write(`${summary}${minBelow.length > 0 ? "  ✗" : "  ✓"}\n`);
      }
    }
  } finally {
    await chrome.kill();
  }

  if (failures.length > 0) {
    console.error(`\n${failures.length} below-threshold result(s):`);
    for (const f of failures) {
      const flagged = Object.entries(f.scores).filter(([, v]) => v < MIN_SCORE).map(([k, v]) => `${k}=${(v * 100).toFixed(0)}`).join(" ");
      console.error(`  ${f.target.id} (${f.preset})  ${flagged}`);
    }
    process.exit(1);
  }
  console.log("\nAll targets ≥ 90 across all categories.");
}

main().catch((e) => { console.error(e); process.exit(1); });
