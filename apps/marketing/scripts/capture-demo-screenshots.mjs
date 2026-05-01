#!/usr/bin/env node
/**
 * Capture /demo step screenshots for the marketing site.
 *
 * Usage:
 *   cd apps/marketing
 *   npm install --no-save playwright
 *   npx playwright install chromium
 *   node scripts/capture-demo-screenshots.mjs
 *
 * Output → apps/marketing/public/demo/step-{1,2,3,4}{,-mobile}.png
 *
 * Run after every meaningful UI change. Idempotent. Daniel runs locally
 * before submission deadlines; CI will run this in a follow-up workflow
 * (own ticket).
 */

import { chromium } from "playwright";
import { mkdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const PUBLIC_DIR = resolve(__dirname, "..", "public", "demo");
const BASE_URL = process.env.DEMO_BASE_URL ?? "http://localhost:4321";

const STEPS = [
  { n: 1, slug: "1-meet-the-agents",       wait: 1500 },
  { n: 2, slug: "2-watch-a-decision",      wait: 500 },
  { n: 3, slug: "3-verify-yourself",       wait: 600 },
  { n: 4, slug: "4-explore-the-trust-graph", wait: 1500 },
];

const VIEWPORTS = [
  { name: "",        width: 1440, height: 900 },
  { name: "-mobile", width: 375,  height: 667 },
];

async function main() {
  await mkdir(PUBLIC_DIR, { recursive: true });
  const browser = await chromium.launch();
  try {
    for (const vp of VIEWPORTS) {
      const ctx = await browser.newContext({ viewport: { width: vp.width, height: vp.height }, deviceScaleFactor: 2 });
      const page = await ctx.newPage();
      for (const step of STEPS) {
        const url = `${BASE_URL}/demo/${step.slug}`;
        console.log(`→ ${url}  (${vp.width}x${vp.height})`);
        await page.goto(url, { waitUntil: "networkidle" });
        await page.waitForTimeout(step.wait);
        const out = `${PUBLIC_DIR}/step-${step.n}${vp.name}.png`;
        await page.screenshot({ path: out, fullPage: false });
        console.log(`  saved ${out}`);
      }
      await ctx.close();
    }
  } finally {
    await browser.close();
  }
}
main().catch((e) => { console.error(e); process.exit(1); });
