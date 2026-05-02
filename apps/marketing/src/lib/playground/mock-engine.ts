// Mock decision engine — TS reimplementation that mimics real
// sbo3l-core behaviour for teaching purposes only.
//
// LOAD-BEARING DISCLAIMER. Mock capsules CANNOT pass the real
// strict-mode WASM verifier because:
//   1. They lack an Ed25519 signature (we have no private key in
//      the browser to sign with — and putting one there would be
//      a security incident waiting to happen)
//   2. They use a distinct schema string ("sbo3l.playground_mock.v1")
//      so the real verifier rejects them as schema-mismatched
//   3. The hashes are computed by browser SubtleCrypto, deterministic
//      but the verifier expects them to be signed
//
// A visitor can verify the boundary holds: paste a mock capsule into
// /proof and watch the strict-mode verifier reject it.
//
// Real decisions: /playground/live (Vercel-hosted real daemon).

export interface MockDecision {
  outcome: "allow" | "deny" | "require_human";
  deny_code?: string;
  matched_rule?: string;
  request_hash: string;     // sha256("mock:" + canonical APRP)
  policy_hash: string;      // sha256("mock:" + policy TOML bytes)
  audit_event_id: string;   // ulid-shaped (mock)
  mock_capsule: MockCapsule;
}

export interface MockCapsule {
  schema: "sbo3l.playground_mock.v1";
  policy_receipt: {
    request_hash: string;
    outcome: "allow" | "deny" | "require_human";
    deny_code?: string;
    matched_rule?: string;
    policy_hash: string;
    audit_event_id: string;
    signature: "MOCK_NOT_SIGNED";
    verifier_pubkey: "MOCK_NO_KEY";
  };
  evidence: {
    aprp_canonical: string;
    policy_canonical: string;
    decision_input_summary: string;
  };
  notice: "This capsule was produced by the in-browser mock engine. It is not cryptographically real and will be rejected by the strict-mode WASM verifier. For real signed receipts, use /playground/live.";
}

interface AprpEnvelope {
  schema_version?: number;
  agent_id?: string;
  intent?: Record<string, unknown> & { kind?: string };
  attestations?: unknown[];
  nonce?: string;
  timestamp_ms?: number;
}

// Browser-side SHA-256 via SubtleCrypto. Returns hex string with the
// "0x" prefix to match the on-chain anchor format.
async function sha256Hex(input: string): Promise<string> {
  const bytes = new TextEncoder().encode(input);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  const hex = Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
  return "0x" + hex;
}

// Best-effort canonical JSON. Real sbo3l uses RFC 8785 JCS; this mock
// uses JSON.stringify with sorted keys, which produces the same result
// for the simple shapes the playground deals with. Visitor sees the
// canonical form in the capsule's `evidence` field.
function canonicalJson(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return "[" + value.map(canonicalJson).join(",") + "]";
  const keys = Object.keys(value as Record<string, unknown>).sort();
  const entries = keys.map((k) => JSON.stringify(k) + ":" + canonicalJson((value as Record<string, unknown>)[k]));
  return "{" + entries.join(",") + "}";
}

// Mock ULID — month-stable id derived from the inputs so the same
// scenario produces the same audit_event_id every time the visitor
// re-runs it. Real ULIDs come from the daemon's monotonic clock.
async function mockUlid(seed: string): Promise<string> {
  const h = await sha256Hex(seed);
  return "01HZRG-MOCK-" + h.slice(2, 16).toUpperCase();
}

interface PolicyParseResult {
  tenant?: string;
  intents: Array<{
    kind?: string;
    where?: Record<string, unknown>;
    require?: Array<Record<string, unknown>>;
  }>;
}

// Sub-set TOML parser — handles the policy shapes the playground uses.
// Not a general TOML parser. If the visitor writes something exotic,
// we fall back to a single-rule "allow everything" interpretation
// rather than throw, so the playground stays interactive even with
// malformed input.
function parsePolicyToml(toml: string): PolicyParseResult {
  const result: PolicyParseResult = { intents: [] };
  const lines = toml.split("\n").map((l) => l.trim()).filter((l) => l && !l.startsWith("#"));

  let currentIntent: PolicyParseResult["intents"][0] | null = null;
  for (const line of lines) {
    if (line === "[[intents]]") {
      if (currentIntent) result.intents.push(currentIntent);
      currentIntent = { where: {}, require: [] };
      continue;
    }
    if (line.startsWith("tenant")) {
      const m = /tenant\s*=\s*"([^"]*)"/.exec(line);
      if (m) result.tenant = m[1];
      continue;
    }
    if (currentIntent) {
      const kindM = /^kind\s*=\s*"([^"]*)"/.exec(line);
      if (kindM) {
        currentIntent.kind = kindM[1];
        continue;
      }
      const whereM = /^where\.([\w.]+)\s*=\s*(.+)$/.exec(line);
      if (whereM && currentIntent.where) {
        currentIntent.where[whereM[1]] = whereM[2];
        continue;
      }
      const reqM = /^require\s*=\s*\[(.*)\]$/.exec(line);
      if (reqM && currentIntent.require) {
        currentIntent.require.push({ raw: reqM[1] });
      }
    }
  }
  if (currentIntent) result.intents.push(currentIntent);
  return result;
}

// The actual decision logic. Pattern-matches against the 8 known
// scenarios; for unknown shapes, defaults to allow with a synthetic
// rule id so the playground always returns *something*.
function evalDecision(aprp: AprpEnvelope, policy: PolicyParseResult): { outcome: MockDecision["outcome"]; deny_code?: string; matched_rule?: string } {
  const intent = aprp.intent ?? {};
  const kind = typeof intent.kind === "string" ? intent.kind : "unknown";

  // Replay nonce — flagged by suffix in the mock fixtures.
  if (typeof aprp.nonce === "string" && aprp.nonce.endsWith("REPLAY")) {
    return { outcome: "deny", deny_code: "protocol.nonce_replay" };
  }

  // Expired APRP — 60-second skew tolerance.
  // Codex review fix (PR #353): the previous bound was hard-coded to
  // 1714565000000 - 60_000 (the fixed scenario timestamp), so any APRP
  // edited in 2026+ would always pass the expiry check. Use Date.now()
  // so the 60s tolerance behaves like the real daemon. The seeded
  // scenarios deliberately include `deny-aprp-expired` with a
  // ts_ms 5 minutes in the past relative to the same fixed reference
  // point, so it still denies via the path below for that scenario.
  const SKEW_TOLERANCE_MS = 60_000;
  const SCENARIO_REF_MS = 1714565000000;
  if (typeof aprp.timestamp_ms === "number") {
    const now = Date.now();
    const isFromSeededScenario = Math.abs(aprp.timestamp_ms - SCENARIO_REF_MS) < 24 * 60 * 60 * 1000;
    const baseline = isFromSeededScenario ? SCENARIO_REF_MS : now;
    if (aprp.timestamp_ms < baseline - SKEW_TOLERANCE_MS) {
      return { outcome: "deny", deny_code: "protocol.aprp_expired" };
    }
  }

  // MEV slippage breach.
  if (kind === "uniswap.swap" && typeof intent.slippage_bps === "number") {
    const cap = policy.intents.find((i) => i.kind === "uniswap.swap")?.where?.["slippage_bps"];
    if (typeof cap === "string" && /lte\s*=\s*(\d+)/.exec(cap)) {
      const limit = parseInt(/lte\s*=\s*(\d+)/.exec(cap)![1]!, 10);
      if (intent.slippage_bps > limit) {
        return { outcome: "deny", deny_code: "policy.deny_mev_slippage", matched_rule: "uniswap.swap-slippage" };
      }
    }
  }

  // Provider allowlist.
  if (kind === "erc20.transfer") {
    const provider = typeof intent.provider === "string" ? intent.provider : "unknown";
    const allowlistRaw = policy.intents.find((i) => i.kind === "erc20.transfer")?.where?.["provider"];
    if (typeof allowlistRaw === "string") {
      const m = /allowlist\s*=\s*\[(.*)\]/.exec(allowlistRaw);
      if (m) {
        const allowed = m[1]!.split(",").map((s) => s.trim().replace(/^"|"$/g, ""));
        if (!allowed.includes(provider)) {
          return { outcome: "deny", deny_code: "policy.deny_unknown_provider", matched_rule: "erc20.transfer-provider" };
        }
      }
    }
  }

  // Token gate — APRP needs to claim an attestation.
  if (kind === "compute.train") {
    const requires = policy.intents.find((i) => i.kind === "compute.train")?.require ?? [];
    const wantsTokenGate = requires.some((r) => typeof r.raw === "string" && r.raw.includes("token_gate"));
    const claimed = Array.isArray(aprp.attestations) && aprp.attestations.length > 0;
    if (wantsTokenGate && !claimed) {
      return { outcome: "deny", deny_code: "policy.deny_token_gate_missing", matched_rule: "compute.train-token-gate" };
    }
  }

  // Human-2FA threshold.
  if (kind === "erc20.transfer" && typeof intent.amount === "number") {
    const requires = policy.intents.find((i) => i.kind === "erc20.transfer")?.require ?? [];
    const human2fa = requires.find((r) => typeof r.raw === "string" && r.raw.includes("human_2fa"));
    if (human2fa) {
      const m = /amount\s*=\s*\{\s*gt\s*=\s*(\d+)/.exec(human2fa.raw as string);
      const threshold = m ? parseInt(m[1]!, 10) : 10000;
      if (intent.amount > threshold) {
        return { outcome: "require_human", matched_rule: "erc20.transfer-human-2fa" };
      }
    }
  }

  // Default allow with a synthetic matched_rule so the result panel
  // has something to show.
  return { outcome: "allow", matched_rule: `${kind}-default-allow` };
}

export interface DecideError {
  kind: "schema" | "json" | "internal";
  message: string;
}

export async function decideMock(aprpRaw: string, policyToml: string): Promise<{ ok: true; decision: MockDecision } | { ok: false; error: DecideError }> {
  let aprp: AprpEnvelope;
  try {
    aprp = JSON.parse(aprpRaw) as AprpEnvelope;
  } catch (e) {
    return { ok: false, error: { kind: "json", message: `APRP is not valid JSON: ${(e as Error).message}` } };
  }

  // Lightweight schema validation — real schema lives at
  // schemas/aprp-envelope.v1.json. We check the load-bearing fields.
  if (typeof aprp.schema_version !== "number") {
    return { ok: false, error: { kind: "schema", message: "APRP missing schema_version (number)" } };
  }
  if (typeof aprp.agent_id !== "string") {
    return { ok: false, error: { kind: "schema", message: "APRP missing agent_id (string)" } };
  }
  if (!aprp.intent || typeof aprp.intent !== "object") {
    return { ok: false, error: { kind: "schema", message: "APRP missing intent (object)" } };
  }

  const policy = parsePolicyToml(policyToml);
  const verdict = evalDecision(aprp, policy);

  const aprpCanonical = canonicalJson(aprp);
  const policyCanonical = policyToml;

  const requestHash = await sha256Hex("mock:" + aprpCanonical);
  const policyHash = await sha256Hex("mock:" + policyCanonical);
  const auditEventId = await mockUlid(requestHash + policyHash);

  const summary = `${verdict.outcome.toUpperCase()}${verdict.deny_code ? ` · ${verdict.deny_code}` : ""}${verdict.matched_rule ? ` · matched ${verdict.matched_rule}` : ""}`;

  const capsule: MockCapsule = {
    schema: "sbo3l.playground_mock.v1",
    policy_receipt: {
      request_hash: requestHash,
      outcome: verdict.outcome,
      deny_code: verdict.deny_code,
      matched_rule: verdict.matched_rule,
      policy_hash: policyHash,
      audit_event_id: auditEventId,
      signature: "MOCK_NOT_SIGNED",
      verifier_pubkey: "MOCK_NO_KEY",
    },
    evidence: {
      aprp_canonical: aprpCanonical,
      policy_canonical: policyCanonical,
      decision_input_summary: summary,
    },
    notice: "This capsule was produced by the in-browser mock engine. It is not cryptographically real and will be rejected by the strict-mode WASM verifier. For real signed receipts, use /playground/live.",
  };

  return {
    ok: true,
    decision: {
      outcome: verdict.outcome,
      deny_code: verdict.deny_code,
      matched_rule: verdict.matched_rule,
      request_hash: requestHash,
      policy_hash: policyHash,
      audit_event_id: auditEventId,
      mock_capsule: capsule,
    },
  };
}
