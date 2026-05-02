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
} from "./zerog-uploader.mjs";

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
