#!/usr/bin/env node
/**
 * Auto-records 4 browser scenes for the SBO3L demo video.
 *
 * Scenes:
 *   1. Homepage hero (18s)        → scene-1-home.mp4
 *   3. /proof drag-drop (32s)     → scene-3-proof.mp4
 *   5. Etherscan UNI-A1 (35s)     → scene-5-uniswap.mp4
 *   6. Outro slide (35s)          → scene-6-outro.mp4
 *
 * Usage:
 *   cd apps/marketing
 *   node ../../docs/submission/video-final-record-pack/record-browser-scenes.mjs
 *
 * Requires:
 *   - playwright (already installed under apps/marketing/node_modules)
 *   - ffmpeg (brew install ffmpeg) — playwright video pipeline needs it
 *
 * Output: ./scenes/scene-{1,3,5,6}-*.webm + .mp4 (1920x1080)
 */

import { chromium } from 'playwright';
import { mkdir, rm, readdir, rename } from 'node:fs/promises';
import { execSync } from 'node:child_process';
import { resolve } from 'node:path';

const OUT_DIR = resolve('scenes');
await rm(OUT_DIR, { recursive: true, force: true });
await mkdir(OUT_DIR, { recursive: true });

const VIEWPORT = { width: 1920, height: 1080 };

// Each scene gets its own context with its own video output dir so the
// playwright-generated webm files are addressable by scene.
async function recordScene(name, durationMs, action) {
  const sceneDir = resolve(OUT_DIR, `_raw-${name}`);
  await mkdir(sceneDir, { recursive: true });
  const browser = await chromium.launch();
  const ctx = await browser.newContext({
    viewport: VIEWPORT,
    recordVideo: { dir: sceneDir, size: VIEWPORT },
  });
  const page = await ctx.newPage();
  console.log(`▶ ${name} (${(durationMs / 1000).toFixed(0)}s)`);
  await action(page);
  await page.waitForTimeout(durationMs);
  await ctx.close();
  await browser.close();
  // Find the generated .webm and convert to .mp4
  const files = await readdir(sceneDir);
  const webm = files.find(f => f.endsWith('.webm'));
  if (!webm) throw new Error(`no webm produced for ${name}`);
  const webmPath = resolve(sceneDir, webm);
  const mp4Path = resolve(OUT_DIR, `scene-${name}.mp4`);
  console.log(`  converting → ${mp4Path}`);
  execSync(
    `ffmpeg -y -i "${webmPath}" -c:v libx264 -preset fast -crf 23 -pix_fmt yuv420p -movflags +faststart "${mp4Path}"`,
    { stdio: 'pipe' }
  );
  await rm(sceneDir, { recursive: true, force: true });
  console.log(`✓ ${name}`);
}

// ─── Scene 1 — Homepage hero (18s) ─────────────────────────────────────
await recordScene('1-home', 18_000, async (page) => {
  await page.goto('https://sbo3l-marketing.vercel.app', { waitUntil: 'networkidle' });
  await page.waitForTimeout(2000);
  // Slow scroll to reveal the architecture diagram + UNI-A1 banner
  await page.evaluate(() => window.scrollBy({ top: 0, behavior: 'smooth' }));
});

// ─── Scene 3 — /proof drag-drop demo (32s) ─────────────────────────────
await recordScene('3-proof', 32_000, async (page) => {
  await page.goto('https://sbo3l-marketing.vercel.app/proof', { waitUntil: 'networkidle' });
  await page.waitForTimeout(3000);
  // Fetch the canonical golden capsule, paste into textarea
  const capsuleText = await page.evaluate(async () => {
    const r = await fetch('https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/capsule.json');
    return r.text();
  });
  const textarea = page.locator('textarea').first();
  await textarea.fill(capsuleText);
  await page.waitForTimeout(1500);
  // Click Verify (button label may vary)
  const verifyBtn = page.locator('button:has-text("Verify")').first();
  if (await verifyBtn.count() > 0) {
    await verifyBtn.click();
  }
  await page.waitForTimeout(8000);
  // Now demonstrate tamper: flip a byte in the audit_event_hash
  const tampered = capsuleText.replace(/"audit_event_hash":\s*"([0-9a-f])/, (m, c) => {
    const flip = c === 'a' ? 'b' : 'a';
    return m.replace(c, flip);
  });
  await textarea.fill(tampered);
  await page.waitForTimeout(1500);
  if (await verifyBtn.count() > 0) {
    await verifyBtn.click();
  }
  await page.waitForTimeout(5000);
});

// ─── Scene 5 — Etherscan UNI-A1 tx (35s) ───────────────────────────────
await recordScene('5-uniswap', 35_000, async (page) => {
  await page.goto(
    'https://etherscan.io/tx/0xed68d1301b479c4229bc89cca5162b56517b80cbaeb654323e05b183000aff0b',
    { waitUntil: 'networkidle', timeout: 45000 }
  );
  await page.waitForTimeout(4000);
  // Slow scroll down to reveal Token Transfers section
  await page.evaluate(() => {
    let y = 0;
    const interval = setInterval(() => {
      y += 30;
      window.scrollTo({ top: y, behavior: 'auto' });
      if (y > 800) clearInterval(interval);
    }, 100);
  });
  await page.waitForTimeout(15000);
  // Scroll back up to top
  await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'smooth' }));
  await page.waitForTimeout(8000);
});

// ─── Scene 6 — Outro slide (35s) ────────────────────────────────────────
// Just stay on /status truth-table page which has all the numbers
await recordScene('6-outro', 35_000, async (page) => {
  await page.goto('https://sbo3l-marketing.vercel.app/status', { waitUntil: 'networkidle' });
  await page.waitForTimeout(3000);
  // Slow scroll through the truth-table to show all 26 rows
  await page.evaluate(() => {
    let y = 0;
    const interval = setInterval(() => {
      y += 50;
      window.scrollTo({ top: y, behavior: 'auto' });
      if (y > 3000) clearInterval(interval);
    }, 200);
  });
  await page.waitForTimeout(20000);
  // Back to top, settle on tagline
  await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'smooth' }));
  await page.waitForTimeout(8000);
});

console.log('\n✓ All 4 browser scenes recorded.');
console.log(`  Output dir: ${OUT_DIR}`);
console.log('  Next: copy these to docs/submission/video-final-record-pack/scenes/, then run stitch.sh');
