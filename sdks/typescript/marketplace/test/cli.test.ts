import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve as resolvePath } from "node:path";

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as ed25519 from "@noble/ed25519";
import { bytesToHex } from "@noble/hashes/utils";

import { run } from "../src/cli.js";
import { signBundle } from "../src/index.js";

async function makeIssuer(): Promise<{ priv: string; pub: string }> {
  const priv = ed25519.utils.randomPrivateKey();
  const pub = await ed25519.getPublicKeyAsync(priv);
  return { priv: bytesToHex(priv), pub: bytesToHex(pub) };
}

const SAMPLE_POLICY = {
  version: 1,
  policy_id: "test-policy",
  default_decision: "deny",
  agents: [{ agent_id: "a", status: "active" }],
  rules: [],
};

let tmpDir: string;
let stdoutCalls: string[];
let stderrCalls: string[];
let stdoutSpy: ReturnType<typeof vi.spyOn> | undefined;
let stderrSpy: ReturnType<typeof vi.spyOn> | undefined;

beforeEach(async () => {
  // vitest workers forbid process.chdir; pass `cwd` to the CLI via the
  // adopt path argument or by working from absolute --as paths.
  tmpDir = await mkdtemp(join(tmpdir(), "sbo3l-marketplace-cli-"));
  stdoutCalls = [];
  stderrCalls = [];
  // Untyped spy — vitest's strict types reject the narrow string-only
  // signature against process.std{out,err}.write's overloaded shape.
  stdoutSpy = vi.spyOn(process.stdout, "write").mockImplementation((data: unknown) => {
    stdoutCalls.push(typeof data === "string" ? data : String(data));
    return true;
  }) as never;
  stderrSpy = vi.spyOn(process.stderr, "write").mockImplementation((data: unknown) => {
    stderrCalls.push(typeof data === "string" ? data : String(data));
    return true;
  }) as never;
});

afterEach(async () => {
  stdoutSpy?.mockRestore();
  stderrSpy?.mockRestore();
  await rm(tmpDir, { recursive: true, force: true });
});

describe("cli — top-level dispatch", () => {
  it("prints help when no command given (exit 2)", async () => {
    expect(await run([])).toBe(2);
    expect(stdoutCalls.join("")).toContain("USAGE:");
  });

  it("prints help on `help` (exit 0)", async () => {
    expect(await run(["help"])).toBe(0);
    expect(stdoutCalls.join("")).toContain("USAGE:");
  });

  it("rejects unknown command (exit 2)", async () => {
    expect(await run(["bogus"])).toBe(2);
    expect(stderrCalls.join("")).toContain("unknown command");
  });
});

describe("cli adopt", () => {
  it("adopt without --from exits 2", async () => {
    expect(await run(["adopt"])).toBe(2);
    expect(stderrCalls.join("")).toContain("--from");
  });

  it("adopt without --registry (and no env var) exits 2", async () => {
    delete process.env["SBO3L_MARKETPLACE"];
    expect(await run(["adopt", "--from", "sha256-abc"])).toBe(2);
    expect(stderrCalls.join("")).toContain("--registry");
  });

  it("adopt fetches, verifies, writes the policy to .sbo3l/policies/", async () => {
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "test policy",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });

    // Stub the global fetch so HttpTransport hits a fake registry that
    // returns our locally-signed bundle.
    const fakeFetch = vi.fn().mockImplementation(async (url: string) => {
      if (url.endsWith(`/v1/policies/${bundle.policy_id}`)) {
        return {
          ok: true,
          status: 200,
          json: async () => bundle,
        } as Response;
      }
      return { ok: false, status: 404 } as Response;
    });
    const prevFetch = globalThis.fetch;
    globalThis.fetch = fakeFetch as never;

    // Trusted-issuers config in the tmpdir.
    const issuers = join(tmpDir, "trusted-issuers.json");
    await writeFile(issuers, JSON.stringify({ "did:test:alice": pub }));

    try {
      const outDir = join(tmpDir, "out");
      const code = await run([
        "adopt",
        "--from",
        bundle.policy_id,
        "--registry",
        "https://registry.example.com",
        "--as",
        "my-policy",
        "--issuers",
        issuers,
        "--out-dir",
        outDir,
      ]);
      expect(code).toBe(0);
      expect(stdoutCalls.join("")).toContain("✓ adopted");
      expect(stdoutCalls.join("")).toContain("did:test:alice");
      const written = await readFile(resolvePath(outDir, "my-policy.json"), "utf-8");
      expect(JSON.parse(written)).toEqual(SAMPLE_POLICY);
    } finally {
      globalThis.fetch = prevFetch;
    }
  });

  it("adopt refuses bundle whose policy_id ≠ content hash (registry tampering)", async () => {
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });

    // Registry returns the bundle but caller asks for a DIFFERENT id.
    const fakeFetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => bundle,
    } as Response);
    const prevFetch = globalThis.fetch;
    globalThis.fetch = fakeFetch as never;

    try {
      const code = await run([
        "adopt",
        "--from",
        "sha256-" + "00".repeat(32), // bogus id
        "--registry",
        "https://registry.example.com",
        "--as",
        "my-policy",
      ]);
      expect(code).toBe(1);
      expect(stderrCalls.join("")).toContain("content tampering");
    } finally {
      globalThis.fetch = prevFetch;
    }
  });

  it("adopt 404 from registry exits 1 (not 2 — args were valid)", async () => {
    const fakeFetch = vi.fn().mockResolvedValue({ ok: false, status: 404 } as Response);
    const prevFetch = globalThis.fetch;
    globalThis.fetch = fakeFetch as never;
    try {
      const code = await run([
        "adopt",
        "--from",
        "sha256-abc",
        "--registry",
        "https://registry.example.com",
        "--as",
        "x",
      ]);
      expect(code).toBe(1);
      expect(stderrCalls.join("")).toContain("no bundle for");
    } finally {
      globalThis.fetch = prevFetch;
    }
  });
});

describe("cli verify", () => {
  it("verify without --file exits 2", async () => {
    expect(await run(["verify"])).toBe(2);
  });

  it("verify reports missing file (exit 1)", async () => {
    expect(await run(["verify", "--file", "/no/such/file.json"])).toBe(1);
    expect(stderrCalls.join("")).toContain("cannot read");
  });

  it("verify rejects malformed JSON", async () => {
    const path = join(tmpDir, "bad.json");
    await writeFile(path, "{not-json");
    expect(await run(["verify", "--file", path])).toBe(1);
    expect(stderrCalls.join("")).toContain("not valid JSON");
  });

  it("verify ok on a freshly-signed bundle with matching trusted issuer", async () => {
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const path = join(tmpDir, "bundle.json");
    await writeFile(path, JSON.stringify(bundle));
    const issuers = join(tmpDir, "trusted-issuers.json");
    await writeFile(issuers, JSON.stringify({ "did:test:alice": pub }));

    const code = await run(["verify", "--file", path, "--issuers", issuers]);
    expect(code).toBe(0);
    expect(stdoutCalls.join("")).toContain("✓ verified");
  });

  it("verify rejects bundle whose issuer is not in registry", async () => {
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:bob",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const path = join(tmpDir, "bundle.json");
    await writeFile(path, JSON.stringify(bundle));
    const issuers = join(tmpDir, "trusted-issuers.json");
    await writeFile(issuers, JSON.stringify({ "did:test:alice": pub })); // bob NOT trusted

    const code = await run(["verify", "--file", path, "--issuers", issuers]);
    expect(code).toBe(1);
    expect(stderrCalls.join("")).toContain("issuer_unknown");
  });
});

describe("cli publish", () => {
  it("publish without --file exits 2", async () => {
    expect(await run(["publish", "--registry", "https://example.com"])).toBe(2);
  });

  it("publish without --registry (and no env) exits 2", async () => {
    delete process.env["SBO3L_MARKETPLACE"];
    const path = join(tmpDir, "bundle.json");
    await writeFile(path, "{}");
    expect(await run(["publish", "--file", path])).toBe(2);
  });

  it("publish PUTs the bundle to the registry", async () => {
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const path = join(tmpDir, "bundle.json");
    await writeFile(path, JSON.stringify(bundle));

    const fakeFetch = vi.fn().mockResolvedValue({ ok: true, status: 200 } as Response);
    const prevFetch = globalThis.fetch;
    globalThis.fetch = fakeFetch as never;
    try {
      const code = await run([
        "publish",
        "--file",
        path,
        "--registry",
        "https://registry.example.com",
      ]);
      expect(code).toBe(0);
      expect(fakeFetch).toHaveBeenCalledWith(
        `https://registry.example.com/v1/policies/${bundle.policy_id}`,
        expect.objectContaining({ method: "PUT" }),
      );
      expect(stdoutCalls.join("")).toContain("✓ published");
    } finally {
      globalThis.fetch = prevFetch;
    }
  });
});

describe("loadIssuerRegistry — fallback", () => {
  it("uses bootstrap (official-only) when no issuers file is found anywhere", async () => {
    // Adopt with no --issuers and no XDG path → falls back to bootstrap.
    // Verify must then fail with issuer_unknown for our test bundle.
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:bob",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const fakeFetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => bundle,
    } as Response);
    const prevFetch = globalThis.fetch;
    globalThis.fetch = fakeFetch as never;

    // Ensure the default discovery paths don't match by overriding HOME
    // + XDG_CONFIG_HOME to the tmpdir (which has no issuers file).
    const prevHome = process.env["HOME"];
    const prevXdg = process.env["XDG_CONFIG_HOME"];
    process.env["HOME"] = tmpDir;
    delete process.env["XDG_CONFIG_HOME"];

    try {
      const code = await run([
        "adopt",
        "--from",
        bundle.policy_id,
        "--registry",
        "https://r.example.com",
        "--as",
        "x",
      ]);
      expect(code).toBe(1);
      // The fallback only trusts the official issuer, so a bob-signed
      // bundle hits issuer_unknown — the loud failure mode the
      // implementation comment promises.
      expect(stderrCalls.join("")).toContain("issuer_unknown");
    } finally {
      globalThis.fetch = prevFetch;
      if (prevHome !== undefined) process.env["HOME"] = prevHome;
      if (prevXdg !== undefined) process.env["XDG_CONFIG_HOME"] = prevXdg;
    }
  });
});
