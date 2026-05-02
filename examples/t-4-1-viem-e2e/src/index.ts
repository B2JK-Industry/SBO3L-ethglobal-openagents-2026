/**
 * T-4-1 closeout — minimal viem client against the deployed Sepolia
 * OffchainResolver (`0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3`).
 *
 * What this proves:
 *
 * 1. The deployed contract is reachable: a public Sepolia RPC sees
 *    bytecode at the canonical address (smoke check).
 * 2. The CCIP-Read gateway responds: viem's built-in CCIP-Read
 *    handler catches the `OffchainLookup` revert from
 *    `text(node, key)`, fetches `sbo3l-ccip.vercel.app/api/{sender}/{data}.json`,
 *    submits the response back to the resolver's `resolveCallback`,
 *    and the resolver verifies the gateway-side EIP-191 signature
 *    on chain.
 * 3. The decoded value is what the gateway claimed it to be: the
 *    sbo3l:* text record for the queried subname.
 *
 * Run:
 *   pnpm install
 *   pnpm start          # default — research-agent.sbo3l-test.eth, sbo3l:agent_id
 *   pnpm start <fqdn> <key>  # custom name + record key
 *
 * Configuration via env (optional, sensible defaults):
 *   SBO3L_SEPOLIA_RPC_URL  — Sepolia JSON-RPC endpoint
 *                           (default: PublicNode public Sepolia)
 *   SBO3L_OFFCHAIN_RESOLVER — override the resolver address
 *                            (default: T-4-1 deploy)
 *
 * The resolver address is pinned canonically in the Rust
 * `sbo3l-identity::contracts::OFFCHAIN_RESOLVER_SEPOLIA` constant
 * (see `crates/sbo3l-identity/src/contracts.rs`); this script
 * mirrors it locally for the JS/TS demo path. A drift between the
 * two surfaces is caught by manual inspection — this example is
 * judge-facing, not test-suite-load-bearing.
 */

import { createPublicClient, encodeFunctionData, http, namehash } from 'viem';
import { sepolia } from 'viem/chains';

// Mirrors `sbo3l-identity::contracts::OFFCHAIN_RESOLVER_SEPOLIA`.
const OFFCHAIN_RESOLVER_SEPOLIA = '0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3' as const;

// Mirrors `sbo3l-identity::contracts::ENS_REGISTRY` (same on every
// network ENS is deployed on).
const ENS_REGISTRY = '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e' as const;

const DEFAULT_RPC =
  process.env.SBO3L_SEPOLIA_RPC_URL ?? 'https://ethereum-sepolia-rpc.publicnode.com';
const RESOLVER_ADDR = (process.env.SBO3L_OFFCHAIN_RESOLVER ??
  OFFCHAIN_RESOLVER_SEPOLIA) as `0x${string}`;

const DEFAULT_FQDN = 'research-agent.sbo3l-test.eth';
const DEFAULT_KEY = 'sbo3l:agent_id';

// Minimal IExtendedResolver ABI — just the two methods we hit.
const RESOLVER_ABI = [
  {
    name: 'resolve',
    type: 'function',
    stateMutability: 'view',
    inputs: [
      { name: 'name', type: 'bytes' },
      { name: 'data', type: 'bytes' },
    ],
    outputs: [{ name: '', type: 'bytes' }],
  },
] as const;

// Standard ENS resolver text() ABI.
const TEXT_ABI = [
  {
    name: 'text',
    type: 'function',
    stateMutability: 'view',
    inputs: [
      { name: 'node', type: 'bytes32' },
      { name: 'key', type: 'string' },
    ],
    outputs: [{ name: '', type: 'string' }],
  },
] as const;

async function main() {
  const fqdn = process.argv[2] ?? DEFAULT_FQDN;
  const recordKey = process.argv[3] ?? DEFAULT_KEY;

  const client = createPublicClient({
    chain: sepolia,
    transport: http(DEFAULT_RPC),
    // viem's built-in CCIP-Read handler is enabled by default;
    // explicit for emphasis here so a reader sees the contract
    // of trust at the call site.
    ccipRead: undefined, // undefined = use the default fetcher
  });

  console.log('═══════════════════════════════════════════════════════════════');
  console.log('T-4-1 viem E2E test — Sepolia OffchainResolver');
  console.log('═══════════════════════════════════════════════════════════════');
  console.log(`RPC:          ${DEFAULT_RPC}`);
  console.log(`Resolver:     ${RESOLVER_ADDR}`);
  console.log(`ENS Registry: ${ENS_REGISTRY}`);
  console.log(`Name (FQDN):  ${fqdn}`);
  console.log(`Record key:   ${recordKey}`);
  console.log();

  // 1. Smoke: bytecode present at the resolver address?
  console.log('Step 1/3 — verifying bytecode is deployed at resolver address...');
  const code = await client.getBytecode({ address: RESOLVER_ADDR });
  if (!code || code === '0x') {
    console.error(`✗ FAILED: no bytecode at ${RESOLVER_ADDR} on Sepolia.`);
    console.error('  Either the address is wrong or the deploy was reverted.');
    process.exit(2);
  }
  console.log(`  ✓ bytecode present (${code.length} hex chars).`);
  console.log();

  // 2. Build the inner `text(node, key)` calldata that the resolver
  //    will receive (or revert on, if it's an offchain resolver).
  const node = namehash(fqdn);
  console.log('Step 2/3 — calling resolver.resolve(dnsEncode(name), text(node, key))...');
  console.log(`  namehash(${fqdn}) = ${node}`);

  // viem doesn't expose a public `dnsEncode` for ENSIP-10. We build
  // it inline: each label length-prefixed, terminated by a zero byte.
  const dnsName = dnsEncode(fqdn);
  console.log(`  dnsEncode = 0x${Buffer.from(dnsName).toString('hex')}`);

  // Inner text() calldata that resolve() will forward.
  const textCalldata = encodeFunctionDataInline('text', TEXT_ABI, [node, recordKey]);
  console.log(`  text() calldata = ${textCalldata}`);
  console.log();

  // 3. Call resolver.resolve(name, data). viem catches the
  //    OffchainLookup revert, fetches from the gateway URL, and
  //    re-submits the signed response to resolveCallback. The result
  //    is the ABI-encoded `(string)` tuple — we decode below.
  console.log('Step 3/3 — submitting and following CCIP-Read flow...');
  let result: `0x${string}`;
  try {
    result = (await client.readContract({
      address: RESOLVER_ADDR,
      abi: RESOLVER_ABI,
      functionName: 'resolve',
      args: [`0x${Buffer.from(dnsName).toString('hex')}`, textCalldata],
    })) as `0x${string}`;
  } catch (err: unknown) {
    console.error(
      `✗ resolve() failed (this is expected if the FQDN has no record on the gateway):`
    );
    console.error(`  ${(err as Error).message}`);
    console.error();
    console.error(
      'A working test FQDN is added once Daniel runs register-fleet.sh against the chosen Sepolia apex (see docs/cli/ens-fleet-sepolia.md).'
    );
    process.exit(2);
  }

  // The resolver returns the ABI-encoded `(string)` tuple from the
  // inner text() call. Decode to the plain string.
  const value = decodeStringTupleHex(result);
  console.log(`  ✓ gateway responded; signature verified on-chain by resolver.`);
  console.log();
  console.log('═══════════════════════════════════════════════════════════════');
  console.log('Result');
  console.log('═══════════════════════════════════════════════════════════════');
  console.log(`${recordKey} = ${JSON.stringify(value)}`);
  console.log('═══════════════════════════════════════════════════════════════');
}

// ============================================================
// Inline helpers — kept self-contained so this example doesn't
// pull a viem version-specific helper that may not exist on every
// pinned viem major.
// ============================================================

function dnsEncode(name: string): Uint8Array {
  if (!name) return new Uint8Array([0]);
  const out: number[] = [];
  for (const label of name.split('.')) {
    if (!label) continue;
    // DNS wire format prefixes each label with its UTF-8 *byte*
    // length, NOT JavaScript's UTF-16 code-unit count. For non-ASCII
    // labels (emoji, accents) the JS `.length` is smaller than the
    // encoded byte count, which produces malformed names. Encode
    // first, then prefix with the encoded length.
    const encoded = new TextEncoder().encode(label);
    if (encoded.length > 63) {
      throw new Error(`label too long: ${encoded.length} bytes`);
    }
    out.push(encoded.length);
    for (const c of encoded) out.push(c);
  }
  out.push(0);
  return new Uint8Array(out);
}

function encodeFunctionDataInline(
  fn: string,
  abi: readonly { name: string; type: string; inputs: readonly { name: string; type: string }[] }[],
  args: readonly unknown[]
): `0x${string}` {
  // Thin wrapper around viem's encodeFunctionData. Imported as a
  // standard ESM named import — `require()` is undefined in this
  // file's runtime (package.json has `"type": "module"`).
  return encodeFunctionData({
    abi: abi as never,
    functionName: fn as never,
    args: args as never,
  });
}

function decodeStringTupleHex(hex: `0x${string}`): string {
  // ABI: head word = offset (always 0x20 for single-string),
  // followed by length + padded bytes.
  const buf = Buffer.from(hex.slice(2), 'hex');
  if (buf.length < 64) {
    throw new Error(`response too short: ${buf.length} bytes`);
  }
  const len = Number(BigInt('0x' + buf.subarray(56, 64).toString('hex')));
  const start = 64;
  const end = start + len;
  if (buf.length < end) {
    throw new Error(`string content OOB: end=${end}, len=${buf.length}`);
  }
  return buf.subarray(start, end).toString('utf8');
}

main().catch((e) => {
  console.error('Unhandled error:', e);
  process.exit(1);
});
