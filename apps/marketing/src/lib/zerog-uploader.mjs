// Pure helper logic for ZeroGUploader.astro — extracted so it can be
// unit-tested with `node --test` without a bundler, jsdom, or React.
//
// The runtime in the .astro component duplicates these helpers inline
// (CSP requires `is:inline` scripts, which can't import modules), but
// both implementations must agree on behavior. The matching test file
// at zerog-uploader.test.mjs locks the contract.
//
// Written as .mjs (not .ts) so it runs on any Node >=18 without the
// experimental TS strip mode. JSDoc carries the type intent.

/**
 * Loose validation for a 0G Storage rootHash. Real verification is
 * content-addressing — we only check format here so the user gets a
 * helpful "this looks malformed" message before clicking through to
 * the storagescan permalink.
 *
 * @param {string} input
 * @returns {boolean}
 */
export function isValidRootHash(input) {
  if (typeof input !== "string") return false;
  return /^0x[0-9a-fA-F]{64}$/.test(input.trim());
}

/**
 * @typedef {{ ok: true, schema: string } | { ok: false, error: string }} CapsuleValidation
 */

/**
 * Validate that a parsed JSON value is a SBO3L Passport capsule shape.
 * We deliberately only check the top-level `schema` field; full
 * schema validation belongs in the WASM verifier on /proof.
 *
 * @param {unknown} parsed
 * @returns {CapsuleValidation}
 */
export function validateCapsule(parsed) {
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    return {
      ok: false,
      error:
        "JSON root must be an object (got " +
        (parsed === null ? "null" : Array.isArray(parsed) ? "array" : typeof parsed) +
        ").",
    };
  }
  const obj = /** @type {Record<string, unknown>} */ (parsed);
  if (typeof obj.schema !== "string" || !obj.schema) {
    return {
      ok: false,
      error:
        "Missing top-level `schema` field — this doesn't look like a SBO3L Passport capsule (expected schema like `sbo3l.passport_capsule.v1`).",
    };
  }
  return { ok: true, schema: obj.schema };
}

/**
 * Try to parse + validate raw file text in one shot. Returns a
 * uniform error envelope so the caller can render a single message.
 *
 * @param {string} text
 * @returns {CapsuleValidation}
 */
export function parseAndValidateCapsule(text) {
  if (!text || !text.trim()) {
    return { ok: false, error: "File is empty." };
  }
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return { ok: false, error: "Not valid JSON: " + msg };
  }
  return validateCapsule(parsed);
}

/**
 * Storagescan-galileo permalink for a given rootHash. Lowercase the
 * hex so the URL is canonical.
 *
 * @param {string} rootHash
 * @returns {string}
 */
export function permalinkFor(rootHash) {
  return "https://storagescan-galileo.0g.ai/file/" + rootHash.trim().toLowerCase();
}

/**
 * Human-readable byte count for the file-too-large message.
 *
 * @param {number} n
 * @returns {string}
 */
export function bytesHuman(n) {
  if (n < 1024) return n + " B";
  if (n < 1024 * 1024) return (n / 1024).toFixed(1) + " KB";
  return (n / (1024 * 1024)).toFixed(2) + " MB";
}

export const MAX_CAPSULE_BYTES = 1024 * 1024;

/**
 * localStorage key for the recently-uploaded rootHash list. Bump the
 * suffix if the schema ever changes (today: array of `{rootHash, ts}`).
 */
export const RECENT_STORAGE_KEY = "sbo3l.zerog.recent.v1";

/**
 * Cap on persisted history. The key + each entry is ~120 bytes, so
 * 5 entries is ~600 bytes — orders of magnitude under any browser
 * quota even at the strictest 5 MB cap. Cap exists for UI density,
 * not storage pressure.
 */
export const RECENT_HISTORY_LIMIT = 5;

/**
 * @typedef {{ rootHash: string, ts: number }} RecentEntry
 */

/**
 * Persist a rootHash to the recently-uploaded list. Returns one of:
 *   - `{ ok: true, entries }`
 *   - `{ ok: false, reason: "quota" }` on QuotaExceededError
 *   - `{ ok: false, reason: "unavailable" }` if localStorage isn't
 *     accessible (private browsing, sandboxed iframe, etc.)
 *   - `{ ok: false, reason: "format" }` if the rootHash fails validation
 *
 * Caller renders the recent list from the returned `entries` (when ok)
 * or surfaces the failure reason inline (when not ok).
 *
 * @param {string} rootHash
 * @param {Storage} [storage] — defaults to globalThis.localStorage
 * @param {number} [now] — injection point for tests
 * @returns {{ ok: true, entries: RecentEntry[] } | { ok: false, reason: "quota" | "unavailable" | "format" }}
 */
export function persistRecent(rootHash, storage, now) {
  if (!isValidRootHash(rootHash)) {
    return { ok: false, reason: "format" };
  }
  const store = storage ?? (typeof localStorage !== "undefined" ? localStorage : null);
  if (!store) return { ok: false, reason: "unavailable" };

  /** @type {RecentEntry[]} */
  let existing = [];
  try {
    const raw = store.getItem(RECENT_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) {
        existing = parsed.filter(
          (e) => e && typeof e === "object" && typeof e.rootHash === "string" && typeof e.ts === "number",
        );
      }
    }
  } catch (_) {
    // Corrupt JSON in storage — discard, start fresh. Don't surface
    // as an error to the caller; recovery is automatic.
    existing = [];
  }

  const norm = rootHash.trim().toLowerCase();
  // De-dup by hash; promote any prior entry to the front.
  existing = existing.filter((e) => e.rootHash !== norm);
  existing.unshift({ rootHash: norm, ts: now ?? Date.now() });
  if (existing.length > RECENT_HISTORY_LIMIT) {
    existing = existing.slice(0, RECENT_HISTORY_LIMIT);
  }

  try {
    store.setItem(RECENT_STORAGE_KEY, JSON.stringify(existing));
    return { ok: true, entries: existing };
  } catch (e) {
    // QuotaExceededError name varies across browsers (Safari uses
    // "QuotaExceededError" / code 22; Firefox "NS_ERROR_DOM_QUOTA_REACHED";
    // Chrome both). Any setItem throw under a normal call is effectively
    // quota-related — treat uniformly.
    return { ok: false, reason: "quota" };
  }
}

/**
 * Read the recently-uploaded list out of localStorage. Returns an empty
 * array on missing key / corrupt JSON / unavailable storage.
 *
 * @param {Storage} [storage]
 * @returns {RecentEntry[]}
 */
export function readRecent(storage) {
  const store = storage ?? (typeof localStorage !== "undefined" ? localStorage : null);
  if (!store) return [];
  try {
    const raw = store.getItem(RECENT_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (e) => e && typeof e === "object" && typeof e.rootHash === "string" && typeof e.ts === "number",
    );
  } catch (_) {
    return [];
  }
}

/**
 * Clear the recently-uploaded list. Returns true on success, false on
 * unavailable storage.
 *
 * @param {Storage} [storage]
 * @returns {boolean}
 */
export function clearRecent(storage) {
  const store = storage ?? (typeof localStorage !== "undefined" ? localStorage : null);
  if (!store) return false;
  try {
    store.removeItem(RECENT_STORAGE_KEY);
    return true;
  } catch (_) {
    return false;
  }
}

/**
 * Detect a popup-blocked outcome from `window.open`. Returns true when
 * the result is null/undefined OR when the returned window's `closed`
 * flag is true immediately (some browsers return a stub window then
 * close it on the next tick — sample after a short timeout when
 * possible). For the synchronous check we just inspect what we got.
 *
 * @param {Window | null | undefined} winRef
 * @returns {boolean}
 */
export function isPopupBlocked(winRef) {
  if (winRef === null || winRef === undefined) return true;
  // `closed` may be true synchronously when a popup is blocked silently.
  try {
    if (winRef.closed) return true;
  } catch (_) {
    // Cross-origin access can throw — that means the popup DID open
    // (just on a different origin we can't inspect). Treat as not-blocked.
    return false;
  }
  return false;
}
