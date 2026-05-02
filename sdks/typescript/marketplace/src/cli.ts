/**
 * `sbo3l-marketplace` — minimal CLI binary for the marketplace SDK.
 *
 * Subcommands:
 *
 *   sbo3l-marketplace adopt --from <policy_id> [--registry <url>] [--as <name>]
 *     - Fetches the signed bundle from the registry.
 *     - Verifies the signature against the trusted-issuer registry
 *       (loaded from `~/.sbo3l/trusted-issuers.json` or `--issuers`).
 *     - Writes the unwrapped policy JSON to `.sbo3l/policies/<name>.json`
 *       (default name = `policy_id` slug).
 *     - Re-checks `policy_id == sha256(canonical_json(policy))` after
 *       fetch, refusing to write if they disagree (registry tampering
 *       check, independent of signature).
 *
 *   sbo3l-marketplace verify --file <path>
 *     - Reads a `SignedPolicyBundle` from `<path>`.
 *     - Verifies all 5 invariants (metadata, content hash, signature,
 *       issuer trusted, pubkey match).
 *     - Exits 0 on ok, 1 on verify fail; prints the failure code.
 *
 *   sbo3l-marketplace publish --file <path> [--registry <url>]
 *     - PUTs an already-signed bundle to a registry.
 *
 * Out of scope: signing happens in the producer's own tooling (key
 * material doesn't belong in a public CLI flag). Use `signBundle` from
 * the SDK for that.
 */

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, resolve } from "node:path";

import {
  HttpTransport,
  IssuerRegistry,
  type SignedPolicyBundle,
  bootstrapOfficialRegistry,
  computePolicyId,
  fetchPolicy,
  publishPolicy,
  verifyBundle,
} from "./index.js";

interface ParsedArgs {
  command: string | undefined;
  flags: Record<string, string>;
}

function parseArgs(argv: string[]): ParsedArgs {
  const command = argv[0];
  const flags: Record<string, string> = {};
  for (let i = 1; i < argv.length; i++) {
    const tok = argv[i];
    if (tok === undefined) continue;
    if (tok.startsWith("--")) {
      const key = tok.slice(2);
      const next = argv[i + 1];
      if (next !== undefined && !next.startsWith("--")) {
        flags[key] = next;
        i++;
      } else {
        flags[key] = "true";
      }
    }
  }
  return { command, flags };
}

const HELP = `sbo3l-marketplace — adopt + verify + publish signed policy bundles

USAGE:
  sbo3l-marketplace adopt   --from <policy_id> [--registry <url>] [--as <name>] [--issuers <path>]
  sbo3l-marketplace verify  --file <path> [--issuers <path>]
  sbo3l-marketplace publish --file <path> [--registry <url>]
  sbo3l-marketplace help

FLAGS:
  --from       Content-addressed policy id to fetch (sha256-<hex>)
  --registry   HTTP marketplace registry base URL (default: $SBO3L_MARKETPLACE)
  --as         Local name for the adopted policy (default: derived from policy_id)
  --file       Path to a SignedPolicyBundle JSON file
  --issuers    Path to trusted issuers JSON (default: \\$XDG_CONFIG_HOME/sbo3l/trusted-issuers.json)

EXIT CODES:
  0  ok
  1  verify failed / fetch failed / file write failed
  2  bad arguments
`;

/** Public entry point — exported for testing without a real process.exit. */
export async function run(argv: string[]): Promise<number> {
  const { command, flags } = parseArgs(argv);

  if (command === undefined || command === "help" || flags.help === "true") {
    process.stdout.write(HELP);
    return command === undefined ? 2 : 0;
  }

  switch (command) {
    case "adopt":
      return cmdAdopt(flags);
    case "verify":
      return cmdVerify(flags);
    case "publish":
      return cmdPublish(flags);
    default:
      process.stderr.write(`unknown command: ${command}\n${HELP}`);
      return 2;
  }
}

/**
 * Load the trusted-issuer registry. Precedence:
 *   1. `--issuers <path>` flag
 *   2. `$XDG_CONFIG_HOME/sbo3l/trusted-issuers.json` (Linux)
 *   3. `~/.sbo3l/trusted-issuers.json`
 *   4. fallback: SBO3L official issuer only (no pubkey configured →
 *      every verify will return `issuer_pubkey_mismatch` until the
 *      operator wires real keys; clear failure mode > silent trust)
 */
async function loadIssuerRegistry(flag: string | undefined): Promise<IssuerRegistry> {
  // When `--issuers <path>` is explicitly provided, fail loudly on
  // read/parse error instead of silently falling through to the
  // discovery candidates. A typo or malformed JSON in the explicit
  // trust store would otherwise route verification through a
  // different (and potentially permissive) registry — that is
  // exactly the silent-substitution failure mode we want to avoid.
  if (flag !== undefined) {
    let raw: string;
    try {
      raw = await readFile(flag, "utf-8");
    } catch (e) {
      throw new Error(
        `--issuers ${flag} could not be read: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
    let parsed: Record<string, string>;
    try {
      parsed = JSON.parse(raw) as Record<string, string>;
    } catch (e) {
      throw new Error(
        `--issuers ${flag} is not valid JSON: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
    const r = new IssuerRegistry();
    for (const [issuer_id, pubkey_hex] of Object.entries(parsed)) {
      r.trust(issuer_id, pubkey_hex);
    }
    return r;
  }

  // No explicit flag — try the standard discovery paths in order.
  const candidates: string[] = [];
  if (process.env["XDG_CONFIG_HOME"] !== undefined) {
    candidates.push(`${process.env["XDG_CONFIG_HOME"]}/sbo3l/trusted-issuers.json`);
  }
  candidates.push(`${homedir()}/.sbo3l/trusted-issuers.json`);

  for (const path of candidates) {
    try {
      const raw = await readFile(path, "utf-8");
      const parsed = JSON.parse(raw) as Record<string, string>;
      const r = new IssuerRegistry();
      for (const [issuer_id, pubkey_hex] of Object.entries(parsed)) {
        r.trust(issuer_id, pubkey_hex);
      }
      return r;
    } catch {
      // try the next candidate
    }
  }
  // No issuers file found → bootstrap with the official issuer + a
  // placeholder pubkey. Verify will fail with `issuer_pubkey_mismatch`
  // until the operator drops a real trusted-issuers.json in place.
  // Surfacing this as a clear deny is preferable to silently trusting
  // nothing or (worse) trusting everything.
  return bootstrapOfficialRegistry("00".repeat(32));
}

/** Slugify a policy_id into a filesystem-safe local name. */
function defaultLocalName(policyId: string): string {
  // sha256-abcdef... → policy-abcdef-12-chars
  const stripped = policyId.replace(/^sha256-/, "");
  return `policy-${stripped.slice(0, 12)}`;
}

async function cmdAdopt(flags: Record<string, string>): Promise<number> {
  const policyId = flags["from"];
  const registry = flags["registry"] ?? process.env["SBO3L_MARKETPLACE"];
  const localName = flags["as"] ?? (policyId !== undefined ? defaultLocalName(policyId) : undefined);
  // `--out-dir` defaults to `.sbo3l/policies/` (relative to cwd), the
  // canonical location the daemon's policy loader scans. Tests override
  // it to a tmpdir to avoid mutating the cwd.
  const outDir = flags["out-dir"] ?? ".sbo3l/policies";

  if (policyId === undefined) {
    process.stderr.write("adopt: --from <policy_id> is required\n");
    return 2;
  }
  if (registry === undefined) {
    process.stderr.write(
      "adopt: --registry <url> required (or set SBO3L_MARKETPLACE env var)\n",
    );
    return 2;
  }
  if (localName === undefined) {
    process.stderr.write("adopt: --as <name> is required\n");
    return 2;
  }

  const transport = new HttpTransport(registry);
  let bundle;
  try {
    bundle = await fetchPolicy(transport, policyId);
  } catch (e) {
    // HttpTransport.get throws on non-404 HTTP / network failures
    // (5xx, connection refused, DNS failure, etc). Surface as exit 1
    // with a clear message rather than letting the unhandled rejection
    // print a stack trace from main()'s top-level await.
    process.stderr.write(
      `adopt: fetch from ${registry} failed: ${e instanceof Error ? e.message : String(e)}\n`,
    );
    return 1;
  }
  if (bundle === undefined) {
    process.stderr.write(`adopt: registry has no bundle for ${policyId}\n`);
    return 1;
  }

  // Tamper check independent of signature: did the registry hand us
  // bytes that hash to the requested id? Catches a registry that
  // silently returns the wrong bundle (e.g. on a misconfigured CDN).
  const actualId = computePolicyId(bundle.policy);
  if (actualId !== policyId) {
    process.stderr.write(
      `adopt: registry returned ${actualId} instead of ${policyId} (content tampering?)\n`,
    );
    return 1;
  }

  let registryIss;
  try {
    registryIss = await loadIssuerRegistry(flags["issuers"]);
  } catch (e) {
    process.stderr.write(`adopt: ${e instanceof Error ? e.message : String(e)}\n`);
    return 1;
  }
  const result = await verifyBundle(bundle, registryIss);
  if (!result.ok) {
    process.stderr.write(`adopt: verify failed (${result.code}): ${result.detail}\n`);
    return 1;
  }

  const dest = resolve(`${outDir}/${localName}.json`);
  await mkdir(dirname(dest), { recursive: true });
  await writeFile(dest, JSON.stringify(bundle.policy, null, 2) + "\n", "utf-8");

  process.stdout.write(
    [
      `✓ adopted ${policyId}`,
      `  issuer:  ${result.issuer_id}`,
      `  label:   ${result.metadata.label}`,
      `  risk:    ${result.metadata.risk_class}`,
      `  written: ${dest}`,
      "",
    ].join("\n"),
  );
  return 0;
}

async function cmdVerify(flags: Record<string, string>): Promise<number> {
  const file = flags["file"];
  if (file === undefined) {
    process.stderr.write("verify: --file <path> is required\n");
    return 2;
  }

  let raw: string;
  try {
    raw = await readFile(file, "utf-8");
  } catch (e) {
    process.stderr.write(`verify: cannot read ${file}: ${e instanceof Error ? e.message : String(e)}\n`);
    return 1;
  }

  let bundle: SignedPolicyBundle;
  try {
    bundle = JSON.parse(raw) as SignedPolicyBundle;
  } catch (e) {
    process.stderr.write(`verify: ${file} is not valid JSON: ${e instanceof Error ? e.message : String(e)}\n`);
    return 1;
  }

  let registry;
  try {
    registry = await loadIssuerRegistry(flags["issuers"]);
  } catch (e) {
    process.stderr.write(`verify: ${e instanceof Error ? e.message : String(e)}\n`);
    return 1;
  }
  const result = await verifyBundle(bundle, registry);
  if (!result.ok) {
    process.stderr.write(`verify: failed (${result.code}): ${result.detail}\n`);
    return 1;
  }
  process.stdout.write(
    [
      `✓ verified ${result.policy_id}`,
      `  issuer:  ${result.issuer_id}`,
      `  label:   ${result.metadata.label}`,
      `  risk:    ${result.metadata.risk_class}`,
      "",
    ].join("\n"),
  );
  return 0;
}

async function cmdPublish(flags: Record<string, string>): Promise<number> {
  const file = flags["file"];
  const registry = flags["registry"] ?? process.env["SBO3L_MARKETPLACE"];
  if (file === undefined) {
    process.stderr.write("publish: --file <path> is required\n");
    return 2;
  }
  if (registry === undefined) {
    process.stderr.write("publish: --registry <url> required\n");
    return 2;
  }

  let bundle: SignedPolicyBundle;
  try {
    const raw = await readFile(file, "utf-8");
    bundle = JSON.parse(raw) as SignedPolicyBundle;
  } catch (e) {
    process.stderr.write(`publish: cannot read ${file}: ${e instanceof Error ? e.message : String(e)}\n`);
    return 1;
  }

  const transport = new HttpTransport(registry);
  try {
    const id = await publishPolicy(transport, bundle);
    process.stdout.write(`✓ published ${id} to ${registry}\n`);
    return 0;
  } catch (e) {
    process.stderr.write(`publish: ${e instanceof Error ? e.message : String(e)}\n`);
    return 1;
  }
}

// Bin entry — only fires when invoked as the binary, NOT when imported
// in tests. `process.argv[1]` is the script path Node was launched with.
const isMain = (() => {
  const argv1 = process.argv[1];
  if (argv1 === undefined) return false;
  // dist/cli.js OR dist/cli.cjs both qualify.
  return argv1.endsWith("/cli.js") || argv1.endsWith("/cli.cjs") || argv1.endsWith("\\cli.js");
})();

if (isMain) {
  run(process.argv.slice(2)).then((code) => {
    process.exit(code);
  });
}
