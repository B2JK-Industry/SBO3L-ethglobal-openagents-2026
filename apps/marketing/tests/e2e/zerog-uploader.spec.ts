/**
 * ZeroGUploader Playwright e2e — exercises the 6 R20 edge cases through
 * a real browser against the static-built /try page.
 *
 * NOT run in CI by default — the unit-shaped suite at
 * `src/lib/zerog-uploader.test.mjs` (42 tests under `node --test`)
 * locks the contract for the pure-helper layer (which the .astro
 * runtime duplicates inline). This Playwright spec catches the few
 * failure modes that only surface in a real browser:
 *
 *   - CSP blocks the runtime script (caught at first interaction)
 *   - DOM event wiring drifts between Astro template + runtime
 *   - touch-pointer media query doesn't actually swap the copy
 *   - real localStorage QuotaExceededError surfaces the warning UI
 *   - real popup-blocker behavior matches our detection
 *
 * Run locally:
 *
 *   cd apps/marketing
 *   npm install -D @playwright/test
 *   npx playwright install --with-deps chromium
 *   npm run build
 *   npx playwright test tests/e2e/zerog-uploader.spec.ts
 *
 * Or run against `npm run dev` by setting BASE_URL=http://localhost:4321
 * before invoking playwright.
 *
 * Why no Playwright in package.json devDeps: adding @playwright/test +
 * Chromium binaries is ~700 MB of CI overhead. The marketing site is
 * static-only Astro and we keep CI fast by running `node --test` for
 * the helpers + a manual Playwright spike when we touch the runtime.
 */

import { expect, test } from "@playwright/test";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Resolve the static-built try page. Honors BASE_URL override (so the
// spec works against `npm run dev` too).
const TRY_URL =
  process.env.BASE_URL !== undefined
    ? `${process.env.BASE_URL}/try`
    : `file://${path.resolve(__dirname, "../../dist/try/index.html")}`;

const VALID_CAPSULE = JSON.stringify(
  {
    schema: "sbo3l.passport_capsule.v1",
    version: 1,
    request: { request_hash: "a".repeat(64) },
    decision: { result: "allow" },
    audit: { audit_event_id: "evt-test-001" },
  },
  null,
  2,
);
const VALID_ROOT_HASH = "0x" + "a".repeat(64);

async function pickFile(page: import("@playwright/test").Page, name: string, content: string) {
  // Astro renders <input type="file" hidden> inside .zg-drop. Playwright
  // can target it by selector even though it's hidden.
  const input = page.locator(".zg-drop input[type='file']");
  await input.setInputFiles({
    name,
    mimeType: name.endsWith(".json") ? "application/json" : "text/plain",
    buffer: Buffer.from(content, "utf-8"),
  });
}

test.describe("ZeroGUploader edge cases", () => {
  test("[edge 1] non-JSON file shows parse error", async ({ page }) => {
    await page.goto(TRY_URL);
    await pickFile(page, "not-json.txt", "this is plain text, not JSON\n");
    await expect(page.locator(".zg-error")).toContainText(/Not valid JSON/);
    // Upload button stays disabled until a valid capsule is loaded.
    await expect(page.locator(".zg-upload")).toBeDisabled();
  });

  test("[edge 2] empty file shows 'File is empty' error", async ({ page }) => {
    await page.goto(TRY_URL);
    await pickFile(page, "empty.json", "");
    await expect(page.locator(".zg-error")).toContainText(/empty/i);
  });

  test("[edge 3] valid JSON missing schema field shows 'Missing top-level' error", async ({ page }) => {
    await page.goto(TRY_URL);
    await pickFile(page, "no-schema.json", JSON.stringify({ hello: "world" }));
    await expect(page.locator(".zg-error")).toContainText(/Missing top-level `schema` field/);
  });

  test("[edge 4] localStorage quota exceeded surfaces warning in success card", async ({ page }) => {
    await page.goto(TRY_URL);
    // Stub localStorage.setItem to throw QuotaExceededError BEFORE any
    // interactions. The runtime's safeWriteRecent catches and shows
    // .zg-quota-warning; the rootHash itself stays valid + visible.
    await page.evaluate(() => {
      const orig = window.localStorage.setItem.bind(window.localStorage);
      window.localStorage.setItem = (k: string, v: string) => {
        if (k.startsWith("sbo3l.zerog.recent")) {
          const e = new DOMException("Quota exceeded", "QuotaExceededError");
          throw e;
        }
        orig(k, v);
      };
    });

    await pickFile(page, "capsule.json", VALID_CAPSULE);
    await expect(page.locator(".zg-error")).toBeHidden();
    await page.locator(".zg-upload").click();
    // Click through the fallback panel's manual rootHash flow to reach success.
    await page.locator(".zg-manual-hash").fill(VALID_ROOT_HASH);
    await page.locator(".zg-manual-submit").click();
    await expect(page.locator(".zg-hash-value")).toHaveText(VALID_ROOT_HASH);
    // Quota warning IS visible because persistAndRenderRecent failed.
    await expect(page.locator(".zg-quota-warning")).toBeVisible();
  });

  test("[edge 5] popup blocked surfaces inline warning in fallback panel", async ({ page, context }) => {
    // Block all popups for this context — the runtime's window.open
    // call returns null, which our isPopupBlocked() catches.
    await context.route("**/storagescan-galileo.0g.ai/**", (route) => route.abort());
    await page.evaluate(() => {
      // Replace window.open with a stub that returns null (simulates
      // browser popup blocker).
      window.open = () => null;
    });

    await page.goto(TRY_URL);
    await pickFile(page, "capsule.json", VALID_CAPSULE);
    await page.locator(".zg-upload").click();
    // Fallback panel reveals + the popup-blocked warning shows.
    await expect(page.locator(".zg-fallback")).toBeVisible();
    await expect(page.locator(".zg-popup-blocked-warning")).toBeVisible();
    await expect(page.locator(".zg-popup-blocked-warning")).toContainText(/blocked the new tab/);
  });

  test("[edge 6] mobile copy swap (pointer: coarse) hides desktop strings", async ({ browser }) => {
    // Emulate a touch device so @media (pointer: coarse) takes effect.
    const ctx = await browser.newContext({
      hasTouch: true,
      isMobile: true,
      viewport: { width: 390, height: 844 }, // iPhone 14 Pro footprint
    });
    const page = await ctx.newPage();
    await page.goto(TRY_URL);

    // Mobile prompt is visible; desktop prompt is hidden via display: none.
    await expect(page.locator(".zg-drop-prompt-mobile")).toBeVisible();
    await expect(page.locator(".zg-drop-prompt-desktop")).toBeHidden();
    await expect(page.locator(".zg-drop-prompt-mobile")).toHaveText(/Tap/);

    await ctx.close();
  });

  test("happy path stays green (sanity — none of the new code regressed it)", async ({ page }) => {
    await page.goto(TRY_URL);
    await pickFile(page, "capsule.json", VALID_CAPSULE);
    await expect(page.locator(".zg-error")).toBeHidden();
    await expect(page.locator(".zg-upload")).toBeEnabled();
    await page.locator(".zg-upload").click();
    await expect(page.locator(".zg-fallback")).toBeVisible();
    await page.locator(".zg-manual-hash").fill(VALID_ROOT_HASH);
    await page.locator(".zg-manual-submit").click();
    await expect(page.locator(".zg-hash-value")).toHaveText(VALID_ROOT_HASH);
    await expect(page.locator(".zg-permalink")).toHaveAttribute(
      "href",
      `https://storagescan-galileo.0g.ai/file/${VALID_ROOT_HASH}`,
    );
  });
});
