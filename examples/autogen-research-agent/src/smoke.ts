/**
 * Smoke runner — proves the demo's function path end-to-end against a
 * running SBO3L daemon WITHOUT an OpenAI key. Calls each AutoGen function
 * descriptor's `call()` directly with hardcoded args.
 */

import {
  dataFetchFunction,
  buildSbo3lPayFunction,
  defaultClient,
  KH_WORKFLOW_ID,
} from "./tools.js";

// Fresh nonce + expiry per run — daemon's protocol.nonce_replay rejects
// duplicate (nonce, agent_id) tuples, so a static nonce only works once.
function freshNonce(): string {
  return globalThis.crypto?.randomUUID?.() ?? `nonce-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

const APRP = {
  agent_id: "research-agent-01",
  task_id: "demo-autogen-smoke-1",
  intent: "purchase_api_call",
  amount: { value: "0.05", currency: "USD" },
  token: "USDC",
  destination: {
    type: "x402_endpoint",
    url: "https://api.example.com/v1/inference",
    method: "POST",
  },
  payment_protocol: "x402",
  chain: "base",
  provider_url: "https://api.example.com",
  expiry: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
  nonce: freshNonce(),
  risk_class: "low",
};

async function main(): Promise<void> {
  console.log(`▶ smoke: KH workflow target = ${KH_WORKFLOW_ID}\n`);

  // Step 1
  console.log("▶ function: data_fetch (GitHub status — public, low-noise)");
  const fetchOut = await dataFetchFunction.call({
    url: "https://www.githubstatus.com/api/v2/status.json",
  });
  if ("error" in fetchOut && fetchOut.error !== undefined) {
    console.log(`  fetch warning: ${fetchOut.error} (continuing — SBO3L gate is the test)`);
  } else {
    console.log(`  ✓ HTTP ${(fetchOut as { status?: number }).status ?? "?"}`);
  }

  // Step 2
  console.log("\n▶ function: sbo3l_payment_request (APRP → SBO3L → KH adapter)");
  const client = defaultClient();
  const sbo3lPay = buildSbo3lPayFunction(client);
  const result = await sbo3lPay.call(APRP);

  console.log("  envelope:");
  for (const [k, v] of Object.entries(result)) {
    console.log(`    ${k}: ${JSON.stringify(v)}`);
  }

  if (result.decision === "allow") {
    console.log(`\n✓ allow — execution_ref ${result.execution_ref ?? "(none)"}`);
    console.log(`  audit_event_id: ${result.audit_event_id ?? "(unknown)"}`);
    process.exit(0);
  }
  if (result.error !== undefined) {
    console.log(`\n✗ transport error — ${result.error}`);
    process.exit(2);
  }
  console.log(`\n✗ ${result.decision ?? "?"} — deny_code ${result.deny_code ?? "?"}`);
  process.exit(2);
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(1);
});
