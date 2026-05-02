/**
 * SBO3L gRPC quickstart (TypeScript).
 *
 * Connects to a running `sbo3l-server-grpc` daemon, calls Health,
 * submits one Decide RPC, and walks the audit chain. Prints
 * everything to stdout so a CI smoke test can grep for the expected
 * shape.
 *
 * Run with:
 *   1. cargo run -p sbo3l-server --features grpc --bin sbo3l-server-grpc
 *   2. (in another terminal) npm start
 *
 * Environment:
 *   SBO3L_GRPC_ADDR   gRPC `host:port` (default 127.0.0.1:8731)
 */

import { createClient, Decision, PaymentStatus } from '@sbo3l/grpc-client';

const address = process.env.SBO3L_GRPC_ADDR ?? '127.0.0.1:8731';

async function main(): Promise<void> {
  console.log(`[grpc-ts] connecting to ${address}`);
  const client = createClient({ address });

  try {
    // 1. Liveness probe.
    const health = await client.health();
    console.log('[health]', JSON.stringify(health));

    // 2. Submit one decision. We hand-roll the APRP rather than
    //    importing the SDK's APRP builder so this example stays
    //    self-contained — the wire shape is the same.
    const aprp = {
      agent_id: 'research-agent-01',
      task_id: 'demo-grpc-quickstart',
      intent: 'purchase_api_call',
      amount: { value: '0.05', currency: 'USD' },
      token: 'USDC',
      destination: {
        type: 'x402_endpoint',
        url: 'https://api.example.com/v1/inference',
        method: 'POST',
        expected_recipient: '0x1111111111111111111111111111111111111111',
      },
      payment_protocol: 'x402',
      chain: 'base',
      provider_url: 'https://api.example.com',
      x402_payload: null,
      // Crockford Base32 — no I, L, O, U; first char must be 0-7.
      // Generated fresh each run so repeated invocations don't
      // collide with the persistent nonce-replay store.
      nonce: generateNonce(),
      expiry: '2099-01-01T00:00:00Z',
      expected_result: null,
      risk_class: 'low',
    };

    const decideResp = await client.decide({ aprp_json: JSON.stringify(aprp) });
    console.log('[decide]', {
      status: PaymentStatus[decideResp.status],
      decision: Decision[decideResp.decision],
      audit_event_id: decideResp.audit_event_id,
      receipt_signed: decideResp.receipt_json.length > 0,
    });

    // 3. Walk the audit chain.
    let count = 0;
    for await (const ev of client.auditChainStream({ since_seq: 0, limit: 10 })) {
      console.log('[chain]', ev.seq, ev.event_type, ev.event_hash.slice(0, 12) + '...');
      count++;
    }
    console.log(`[chain] ${count} event(s) emitted`);
  } finally {
    client.close();
  }
}

/**
 * Generate a Crockford-Base32 ULID-shaped nonce (26 chars, first
 * character 0-7). The proto/server schema enforces the exact regex
 * `^[0-7][0-9A-HJKMNP-TV-Z]{25}$`.
 */
function generateNonce(): string {
  // Crockford Base32 alphabet, omitting I, L, O, U.
  const alphabet = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
  // First character: 0-7 only (top 3 bits zero per ULID timestamp encoding).
  let out = alphabet[Math.floor(Math.random() * 8)];
  for (let i = 0; i < 25; i++) {
    out += alphabet[Math.floor(Math.random() * alphabet.length)];
  }
  return out;
}

main().catch((err) => {
  console.error('[error]', err);
  process.exit(1);
});
