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
