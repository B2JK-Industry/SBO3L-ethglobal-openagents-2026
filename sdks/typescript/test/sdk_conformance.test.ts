// SDK conformance — TypeScript runner.
//
// Walks `test-corpus/sdk-conformance/manifest.json` and asserts the
// TypeScript SDK's structural verifier produces the same outcome as
// the manifest's `verify_ok` field. The Rust runner
// (`crates/sbo3l-core/tests/sdk_conformance.rs`) and Python runner
// (`sdks/python/tests/test_sdk_conformance.py`) walk the same
// manifest — drift between SDKs is the regression mode this catches.

import { describe, it, expect } from "vitest";
import * as fs from "node:fs";
import * as path from "node:path";

import { verify } from "../src/index.js";

const MANIFEST_SCHEMA = "sbo3l.sdk_conformance_manifest.v1";
const SDK_NAME = "typescript";

interface Vector {
  name: string;
  fixture: string;
  schema_version: number;
  verify_ok: boolean;
  known_drift?: string[];
  comment?: string;
}

interface Manifest {
  schema: string;
  vectors: Vector[];
}

function corpusRoot(): string {
  // tests live at sdks/typescript/test/sdk_conformance.test.ts;
  // walk up to repo root.
  return path.resolve(__dirname, "..", "..", "..", "test-corpus");
}

function loadManifest(): Manifest {
  const raw = fs.readFileSync(
    path.join(corpusRoot(), "sdk-conformance", "manifest.json"),
    "utf8",
  );
  return JSON.parse(raw) as Manifest;
}

function loadCapsule(rel: string): unknown {
  const raw = fs.readFileSync(path.join(corpusRoot(), rel), "utf8");
  return JSON.parse(raw);
}

describe("SDK conformance — TypeScript runner", () => {
  const manifest = loadManifest();

  it("manifest schema id is the documented value", () => {
    expect(manifest.schema).toBe(MANIFEST_SCHEMA);
  });

  it("manifest has at least one vector", () => {
    expect(manifest.vectors.length).toBeGreaterThan(0);
  });

  it("manifest vector count is pinned (sync with manifest.json)", () => {
    // Adding/removing a fixture must update both the manifest and
    // this assertion together.
    expect(manifest.vectors.length).toBe(19);
  });

  it("typescript SDK matches every manifest vector (skipping known_drift)", () => {
    // The conformance contract: every fixture's structural verify
    // outcome agrees with the manifest. Vectors listed under
    // `known_drift: [..., "typescript", ...]` are SKIPPED — those
    // are documented gaps where the TS SDK currently disagrees with
    // the Rust reference. The skip is loud (printed below) so a
    // regression on a non-drift vector still fails the test.
    const failures: string[] = [];
    const pending: string[] = [];

    for (const vector of manifest.vectors) {
      if ((vector.known_drift ?? []).includes(SDK_NAME)) {
        pending.push(vector.name);
        continue;
      }
      const capsule = loadCapsule(vector.fixture) as Record<string, unknown>;
      const result = verify(capsule);
      const actualOk = result.ok;
      if (actualOk !== vector.verify_ok) {
        const codes = !actualOk
          ? result.failures.map((c) => c.code).join(",")
          : "";
        failures.push(
          `[${vector.name}] expected verify_ok=${vector.verify_ok}, got ${actualOk} (failures: ${codes})`,
        );
      }
    }

    if (pending.length > 0) {
      // eslint-disable-next-line no-console
      console.log(
        `SDK conformance: ${pending.length} vector(s) pending TS SDK fix (${pending.join(", ")})`,
      );
    }

    expect(failures, failures.join("\n")).toEqual([]);
  });
});
