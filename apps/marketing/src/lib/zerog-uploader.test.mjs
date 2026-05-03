// Unit tests for the ZeroGUploader pure helpers (DG-1).
//
// Runs under Node's built-in test runner: `npm test` from
// apps/marketing/, or directly via `node --test`. No vitest, no jsdom,
// no React — the marketing site is static Astro and we keep it that
// way.
//
// The Astro component (ZeroGUploader.astro) duplicates these helpers
// inline because CSP forbids module imports inside `is:inline`
// scripts; this test locks the contract both implementations honor.

import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  isValidRootHash,
  validateCapsule,
  parseAndValidateCapsule,
  permalinkFor,
  bytesHuman,
  MAX_CAPSULE_BYTES,
  RECENT_STORAGE_KEY,
  RECENT_HISTORY_LIMIT,
  persistRecent,
  readRecent,
  clearRecent,
  isPopupBlocked,
} from "./zerog-uploader.mjs";

/**
 * In-memory Storage stub that supports an `injectedThrowOnSetItem` flag
 * for testing QuotaExceededError. Tests construct fresh instances so
 * state doesn't leak between cases.
 */
function makeStorage(opts = {}) {
  const map = new Map();
  return {
    getItem(k) {
      return map.has(k) ? map.get(k) : null;
    },
    setItem(k, v) {
      if (opts.throwOnSet) {
        const e = new Error("QuotaExceededError");
        e.name = "QuotaExceededError";
        throw e;
      }
      map.set(k, String(v));
    },
    removeItem(k) {
      map.delete(k);
    },
    get length() {
      return map.size;
    },
    key(i) {
      return Array.from(map.keys())[i] ?? null;
    },
    clear() {
      map.clear();
    },
    _internal: map,
  };
}

describe("isValidRootHash", () => {
  it("accepts a 32-byte 0x-prefixed lowercase hash", () => {
    const h = "0x" + "a".repeat(64);
    assert.equal(isValidRootHash(h), true);
  });

  it("accepts a mixed-case hash", () => {
    const h = "0xAbCdEf" + "0".repeat(58);
    assert.equal(isValidRootHash(h), true);
  });

  it("rejects a hash without 0x prefix", () => {
    const h = "a".repeat(64);
    assert.equal(isValidRootHash(h), false);
  });

  it("rejects a hash with wrong length", () => {
    assert.equal(isValidRootHash("0xabc"), false);
    assert.equal(isValidRootHash("0x" + "a".repeat(63)), false);
    assert.equal(isValidRootHash("0x" + "a".repeat(65)), false);
  });

  it("rejects non-hex chars", () => {
    assert.equal(isValidRootHash("0x" + "z".repeat(64)), false);
  });

  it("trims surrounding whitespace before validating", () => {
    const h = "  0x" + "a".repeat(64) + "  ";
    assert.equal(isValidRootHash(h), true);
  });

  it("rejects empty + non-string inputs", () => {
    assert.equal(isValidRootHash(""), false);
    assert.equal(isValidRootHash(/** @type {any} */ (null)), false);
    assert.equal(isValidRootHash(/** @type {any} */ (undefined)), false);
    assert.equal(isValidRootHash(/** @type {any} */ (42)), false);
  });
});

describe("validateCapsule", () => {
  it("accepts an object with a non-empty `schema` string", () => {
    const r = validateCapsule({ schema: "sbo3l.passport_capsule.v1", anything: 1 });
    assert.equal(r.ok, true);
    if (r.ok) assert.equal(r.schema, "sbo3l.passport_capsule.v1");
  });

  it("rejects a parsed null", () => {
    const r = validateCapsule(null);
    assert.equal(r.ok, false);
    if (!r.ok) assert.match(r.error, /must be an object/);
  });

  it("rejects an array", () => {
    const r = validateCapsule([{ schema: "x" }]);
    assert.equal(r.ok, false);
    if (!r.ok) assert.match(r.error, /array/);
  });

  it("rejects an object without `schema`", () => {
    const r = validateCapsule({ receipt: {} });
    assert.equal(r.ok, false);
    if (!r.ok) assert.match(r.error, /Missing top-level `schema` field/);
  });

  it("rejects an object with empty `schema`", () => {
    const r = validateCapsule({ schema: "" });
    assert.equal(r.ok, false);
  });

  it("rejects an object with non-string `schema`", () => {
    const r = validateCapsule({ schema: 42 });
    assert.equal(r.ok, false);
  });
});

describe("parseAndValidateCapsule", () => {
  it("rejects empty input", () => {
    const r = parseAndValidateCapsule("");
    assert.equal(r.ok, false);
    if (!r.ok) assert.match(r.error, /empty/i);
  });

  it("rejects whitespace-only input", () => {
    const r = parseAndValidateCapsule("   \n\t  ");
    assert.equal(r.ok, false);
    if (!r.ok) assert.match(r.error, /empty/i);
  });

  it("returns a JSON-parse error for malformed JSON", () => {
    const r = parseAndValidateCapsule("{not json");
    assert.equal(r.ok, false);
    if (!r.ok) assert.match(r.error, /Not valid JSON/);
  });

  it("walks all the way through for a real-shaped capsule", () => {
    const goodCapsule = JSON.stringify({
      schema: "sbo3l.passport_capsule.v1",
      generated_at: "2026-04-29T10:00:00Z",
      agent: { agent_id: "research-agent-01" },
      request: {},
    });
    const r = parseAndValidateCapsule(goodCapsule);
    assert.equal(r.ok, true);
    if (r.ok) assert.equal(r.schema, "sbo3l.passport_capsule.v1");
  });

  it("rejects valid JSON that lacks the schema field", () => {
    const r = parseAndValidateCapsule('{"foo": "bar"}');
    assert.equal(r.ok, false);
    if (!r.ok) assert.match(r.error, /Missing top-level `schema`/);
  });
});

describe("permalinkFor", () => {
  it("formats the storagescan-galileo URL with lowercased hex", () => {
    const h = "0xABCD" + "0".repeat(60);
    assert.equal(
      permalinkFor(h),
      "https://storagescan-galileo.0g.ai/file/0xabcd" + "0".repeat(60),
    );
  });

  it("trims surrounding whitespace from the hash", () => {
    const h = "  0x" + "a".repeat(64) + "  ";
    assert.equal(
      permalinkFor(h),
      "https://storagescan-galileo.0g.ai/file/0x" + "a".repeat(64),
    );
  });
});

describe("bytesHuman", () => {
  it("formats sub-KB as bytes", () => {
    assert.equal(bytesHuman(0), "0 B");
    assert.equal(bytesHuman(1023), "1023 B");
  });
  it("formats sub-MB as KB", () => {
    assert.equal(bytesHuman(1024), "1.0 KB");
    assert.equal(bytesHuman(1500), "1.5 KB");
  });
  it("formats >= 1 MB as MB", () => {
    assert.equal(bytesHuman(1024 * 1024), "1.00 MB");
    assert.equal(bytesHuman(MAX_CAPSULE_BYTES + 1), "1.00 MB");
    assert.equal(bytesHuman(2 * 1024 * 1024), "2.00 MB");
  });
});

describe("file-too-large guard (ergonomic check)", () => {
  it("MAX_CAPSULE_BYTES is exactly 1 MB so the error message stays accurate", () => {
    assert.equal(MAX_CAPSULE_BYTES, 1024 * 1024);
  });
});

describe("simulated fallback flow — manual rootHash entry", () => {
  // Models the documented happy path the user takes when the SDK probe
  // times out: open new tab, paste rootHash, validate, render permalink.
  // We model the validate + permalink half here; window.open is purely
  // a side effect that the .astro runtime does inline.
  it("validates then formats permalink for a fresh paste", () => {
    const userPaste = "0x" + "9".repeat(64);
    assert.equal(isValidRootHash(userPaste), true);
    const url = permalinkFor(userPaste);
    assert.equal(
      url,
      "https://storagescan-galileo.0g.ai/file/0x" + "9".repeat(64),
    );
  });

  it("rejects a paste with stray prefix from a copy-paste mishap", () => {
    const sloppy = "rootHash: 0x" + "1".repeat(64);
    assert.equal(isValidRootHash(sloppy), false);
  });
});

// ─────────────────────────────────────────────────────────────────────
// R20 edge cases (#436 batch B):
//   1. non-JSON file        — covered by "Not valid JSON" suite above
//   2. empty file           — covered by "File is empty" suite above
//   3. valid JSON not capsule — covered by "Missing top-level schema" above
//   4. localStorage quota   — new tests below
//   5. popup blocked        — new tests below
//   6. mobile drag-drop     — pure-CSS swap (no JS to test); the
//      .astro template uses `@media (pointer: coarse)` to swap copy
//      between desktop and touch primary devices.
// ─────────────────────────────────────────────────────────────────────

describe("edge case 4 — localStorage quota + persistence", () => {
  it("rejects an invalid rootHash format before touching storage", () => {
    const store = makeStorage();
    const r = persistRecent("not-a-valid-hash", store);
    assert.equal(r.ok, false);
    assert.equal(r.reason, "format");
    // Storage untouched.
    assert.equal(store._internal.size, 0);
  });

  it("persists a fresh rootHash to an empty store", () => {
    const store = makeStorage();
    const h = "0x" + "a".repeat(64);
    const r = persistRecent(h, store, 1700000000000);
    assert.equal(r.ok, true);
    assert.equal(r.entries.length, 1);
    assert.equal(r.entries[0].rootHash, h);
    assert.equal(r.entries[0].ts, 1700000000000);
    // Storage now contains the serialized list.
    assert.ok(store._internal.has(RECENT_STORAGE_KEY));
  });

  it("de-dupes by hash and promotes the entry to the front", () => {
    const store = makeStorage();
    const h1 = "0x" + "1".repeat(64);
    const h2 = "0x" + "2".repeat(64);
    persistRecent(h1, store, 1);
    persistRecent(h2, store, 2);
    const r = persistRecent(h1, store, 3);  // re-add h1
    assert.equal(r.ok, true);
    assert.equal(r.entries.length, 2);
    assert.equal(r.entries[0].rootHash, h1);  // promoted to front
    assert.equal(r.entries[0].ts, 3);          // with new timestamp
    assert.equal(r.entries[1].rootHash, h2);
  });

  it("caps history at RECENT_HISTORY_LIMIT entries", () => {
    const store = makeStorage();
    for (let i = 0; i < RECENT_HISTORY_LIMIT + 3; i++) {
      // Hex-encode i to get a unique 64-char hash per call.
      const h = "0x" + i.toString(16).padStart(64, "0");
      persistRecent(h, store, i);
    }
    const entries = readRecent(store);
    assert.equal(entries.length, RECENT_HISTORY_LIMIT);
    // Most recent (highest i) should be at the front.
    const expectedFront = "0x" + (RECENT_HISTORY_LIMIT + 2).toString(16).padStart(64, "0");
    assert.equal(entries[0].rootHash, expectedFront);
  });

  it("returns reason='quota' on QuotaExceededError without crashing", () => {
    const store = makeStorage({ throwOnSet: true });
    const h = "0x" + "a".repeat(64);
    const r = persistRecent(h, store);
    assert.equal(r.ok, false);
    assert.equal(r.reason, "quota");
  });

  it("returns reason='unavailable' when storage is provided as undefined + no global localStorage", () => {
    // Stash + restore the global so this test works regardless of
    // Node/jsdom version (recent Node added experimental localStorage).
    const orig = globalThis.localStorage;
    try {
      // @ts-expect-error — clearing for the duration of the test
      delete globalThis.localStorage;
      const r = persistRecent("0x" + "a".repeat(64), undefined);
      assert.equal(r.ok, false);
      assert.equal(r.reason, "unavailable");
    } finally {
      if (orig !== undefined) globalThis.localStorage = orig;
    }
  });

  it("readRecent returns [] on missing key", () => {
    assert.deepEqual(readRecent(makeStorage()), []);
  });

  it("readRecent recovers gracefully from corrupt JSON in storage", () => {
    const store = makeStorage();
    store.setItem(RECENT_STORAGE_KEY, "{not-valid-json");
    assert.deepEqual(readRecent(store), []);
  });

  it("readRecent filters malformed entries (defensive)", () => {
    const store = makeStorage();
    // Mix of valid + malformed entries — only valid ones should survive.
    store.setItem(
      RECENT_STORAGE_KEY,
      JSON.stringify([
        { rootHash: "0x" + "a".repeat(64), ts: 1 },
        { rootHash: 42, ts: 2 },           // wrong type
        null,
        "string-not-object",
        { rootHash: "0x" + "b".repeat(64) },  // missing ts
        { rootHash: "0x" + "c".repeat(64), ts: 3 },
      ]),
    );
    const entries = readRecent(store);
    assert.equal(entries.length, 2);
    assert.equal(entries[0].rootHash, "0x" + "a".repeat(64));
    assert.equal(entries[1].rootHash, "0x" + "c".repeat(64));
  });

  it("clearRecent removes the key + returns true on success", () => {
    const store = makeStorage();
    persistRecent("0x" + "a".repeat(64), store);
    assert.equal(clearRecent(store), true);
    assert.deepEqual(readRecent(store), []);
  });

  it("clearRecent returns false on unavailable storage", () => {
    const orig = globalThis.localStorage;
    try {
      // @ts-expect-error — clearing for the duration of the test
      delete globalThis.localStorage;
      assert.equal(clearRecent(undefined), false);
    } finally {
      if (orig !== undefined) globalThis.localStorage = orig;
    }
  });
});

describe("edge case 5 — popup blocked detection", () => {
  it("treats null return from window.open as blocked", () => {
    assert.equal(isPopupBlocked(null), true);
  });

  it("treats undefined return as blocked", () => {
    assert.equal(isPopupBlocked(undefined), true);
  });

  it("treats a stub-window with closed=true as blocked", () => {
    assert.equal(isPopupBlocked({ closed: true }), true);
  });

  it("treats a real window stub with closed=false as not-blocked", () => {
    assert.equal(isPopupBlocked({ closed: false }), false);
  });

  it("treats a cross-origin window (closed throws) as not-blocked", () => {
    const crossOrigin = {
      get closed() {
        throw new Error("SecurityError: Blocked a frame with origin …");
      },
    };
    // Cross-origin access throws — that means the popup DID open
    // (just on a different origin we can't inspect). Don't flag.
    assert.equal(isPopupBlocked(crossOrigin), false);
  });
});
