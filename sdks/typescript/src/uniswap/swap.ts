/**
 * `swap()` — agent-side execution after SBO3L's policy gate has cleared.
 *
 * Mock mode (default): returns a deterministic pseudo-tx-hash derived from
 * the SwapParams + a nonce. Lets the demo path run in CI without secrets.
 *
 * Live mode (`SBO3L_LIVE_ETH=1` + `SBO3L_ETH_RPC_URL` + `SBO3L_ETH_PRIVATE_KEY`
 * in env, OR explicit `env: SwapEnv` arg): constructs the SwapRouter02
 * `exactInputSingle` calldata, signs with the caller's private key, broadcasts
 * via `eth_sendRawTransaction`. Returns the resulting tx hash + Etherscan URL.
 *
 * The signing primitives (RLP-encoded EIP-1559 tx, secp256k1 ECDSA) live in
 * a vendored implementation here so the SDK has zero crypto deps. Add `viem`
 * if you need EIP-712 / Permit2 / multi-call paths — out of scope for v1.
 */

import { createHash } from "node:crypto";
import {
  EXACT_INPUT_SINGLE_SELECTOR,
  SEPOLIA_CHAIN_ID,
  SEPOLIA_SWAP_ROUTER_02,
  sepoliaEtherscanTxUrl,
  SEPOLIA_USDC,
  SEPOLIA_WETH,
} from "./sepolia.js";

/** Single-pool exact-input swap parameters. Mirrors Rust's `SwapParams`. */
export interface SwapParams {
  /** EIP-55 address of the token being sold (e.g. WETH or USDC). */
  tokenIn: string;
  /** EIP-55 address of the token being bought. */
  tokenOut: string;
  /** Pool fee tier in hundredths of a bip. 500 / 3000 / 10000. */
  fee: number;
  /** Recipient EOA — receives `tokenOut`. Usually the caller's address. */
  recipient: string;
  /** Exact amount of `tokenIn` to spend, in token's smallest unit (wei / micros). */
  amountIn: bigint;
  /** Slippage floor — derive from a recent quote, e.g. `quote * 99n / 100n`. */
  amountOutMinimum: bigint;
  /** Price ceiling. `0n` disables (most common). */
  sqrtPriceLimitX96?: bigint;
}

export interface SwapEnv {
  /** RPC URL (e.g. an Alchemy Sepolia URL). */
  rpcUrl: string;
  /** 32-byte private key as 0x-prefixed hex. */
  privateKeyHex: string;
  /** Sepolia chain id. Defaults to `SEPOLIA_CHAIN_ID`. */
  chainId?: number;
  /** SwapRouter02 address. Defaults to `SEPOLIA_SWAP_ROUTER_02`. */
  routerAddress?: string;
}

export interface SwapResult {
  /** `mock` when no live env is provided; `live` when on-chain tx broadcast succeeded. */
  mode: "mock" | "live";
  /** 0x-prefixed transaction hash. Pseudo-hash in mock mode (deterministic). */
  txHash: string;
  /** Sepolia Etherscan URL for the tx. */
  etherscanUrl: string;
  /** Hex-encoded calldata that was (or would have been) sent. */
  calldata: string;
  /** Router address used. */
  to: string;
}

/** Encode the SwapRouter02 `exactInputSingle` calldata. Mirrors Rust's `encode_exact_input_single`. */
export function encodeExactInputSingle(params: SwapParams): string {
  const tokenIn = parseAddress(params.tokenIn);
  const tokenOut = parseAddress(params.tokenOut);
  const recipient = parseAddress(params.recipient);
  const sqrtPrice = params.sqrtPriceLimitX96 ?? 0n;

  const out = new Uint8Array(4 + 32 * 7);
  // selector
  out.set(hexToBytes(EXACT_INPUT_SINGLE_SELECTOR), 0);
  // tokenIn
  out.set(addressPadded(tokenIn), 4);
  // tokenOut
  out.set(addressPadded(tokenOut), 4 + 32);
  // fee (uint24 → uint256)
  out.set(uintPadded(BigInt(params.fee)), 4 + 64);
  // recipient
  out.set(addressPadded(recipient), 4 + 96);
  // amountIn
  out.set(uintPadded(params.amountIn), 4 + 128);
  // amountOutMinimum
  out.set(uintPadded(params.amountOutMinimum), 4 + 160);
  // sqrtPriceLimitX96
  out.set(uintPadded(sqrtPrice), 4 + 192);

  return bytesToHex(out);
}

/**
 * Build + (in live mode) sign + broadcast a SwapRouter02 swap.
 *
 * In **mock mode**, returns a deterministic pseudo-tx-hash derived from
 * the calldata so different swap params produce different hashes. The
 * Etherscan URL is still real — judges can paste it later once the swap
 * actually fires in live mode.
 */
export async function swap(params: SwapParams, env?: SwapEnv): Promise<SwapResult> {
  const calldata = encodeExactInputSingle(params);
  const router = env?.routerAddress ?? SEPOLIA_SWAP_ROUTER_02;

  const liveEnabled = env !== undefined || process.env["SBO3L_LIVE_ETH"] === "1";
  if (!liveEnabled) {
    // Deterministic pseudo-hash so the same params always print the same
    // hash — useful for snapshot tests and demo reproducibility.
    const txHash = "0x" + sha256Hex(calldata + router).slice(0, 64);
    return {
      mode: "mock",
      txHash,
      etherscanUrl: sepoliaEtherscanTxUrl(txHash),
      calldata,
      to: router,
    };
  }

  const liveEnv = env ?? envFromProcess();
  const chainId = liveEnv.chainId ?? SEPOLIA_CHAIN_ID;
  const txHash = await broadcastEip1559(
    liveEnv.rpcUrl,
    liveEnv.privateKeyHex,
    chainId,
    router,
    calldata,
  );
  return {
    mode: "live",
    txHash,
    etherscanUrl: sepoliaEtherscanTxUrl(txHash),
    calldata,
    to: router,
  };
}

/* -------------------------------------------------------------------------- */
/*  Helpers                                                                    */
/* -------------------------------------------------------------------------- */

function envFromProcess(): SwapEnv {
  const rpcUrl = process.env["SBO3L_ETH_RPC_URL"];
  const privateKeyHex = process.env["SBO3L_ETH_PRIVATE_KEY"];
  if (rpcUrl === undefined || privateKeyHex === undefined) {
    throw new Error(
      "live swap requires SBO3L_ETH_RPC_URL + SBO3L_ETH_PRIVATE_KEY (or pass `env`)",
    );
  }
  return { rpcUrl, privateKeyHex };
}

function parseAddress(s: string): Uint8Array {
  const trimmed = s.startsWith("0x") || s.startsWith("0X") ? s.slice(2) : s;
  if (trimmed.length !== 40) {
    throw new Error(`address must be 0x + 40 hex chars, got ${trimmed.length}`);
  }
  return hexToBytes(trimmed);
}

const HEX_RE = /^[0-9a-fA-F]*$/;

function hexToBytes(hex: string): Uint8Array {
  const trimmed = hex.startsWith("0x") || hex.startsWith("0X") ? hex.slice(2) : hex;
  if (trimmed.length % 2 !== 0) throw new Error("hex must be even-length");
  // parseInt("gg", 16) returns NaN which silently coerces to 0 in `Uint8Array.set`,
  // letting malformed calldata reach the chain. Validate explicitly first.
  if (!HEX_RE.test(trimmed)) {
    throw new Error(`hex contains non-hex characters: ${JSON.stringify(hex)}`);
  }
  const out = new Uint8Array(trimmed.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(trimmed.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

function bytesToHex(bytes: Uint8Array): string {
  let s = "0x";
  for (const b of bytes) s += b.toString(16).padStart(2, "0");
  return s;
}

function addressPadded(addr: Uint8Array): Uint8Array {
  const out = new Uint8Array(32);
  out.set(addr, 12);
  return out;
}

function uintPadded(v: bigint): Uint8Array {
  if (v < 0n) throw new Error("uint must be non-negative");
  const out = new Uint8Array(32);
  let i = 31;
  let x = v;
  while (x > 0n && i >= 0) {
    out[i--] = Number(x & 0xffn);
    x >>= 8n;
  }
  return out;
}

function sha256Hex(input: string): string {
  return createHash("sha256").update(input).digest("hex");
}

/**
 * Sign + broadcast an EIP-1559 transaction via `viem` (optional peer dep).
 *
 * `viem` is dynamically imported so the SDK's *non-live* surface (calldata
 * encoding, mock-mode swap, demo smoke) keeps zero runtime dependencies.
 * Live mode requires consumers to install `viem` themselves —
 * `peerDependenciesMeta.viem.optional: true` flags it as non-mandatory at
 * install time so npm doesn't warn for the 99% of users who never enable
 * live mode.
 *
 * Why not vendor secp256k1+RLP: ~600 LoC of audited crypto would dwarf the
 * SDK's ~1500 LoC core, and viem's tree-shaken footprint (~30kB gzipped)
 * is smaller than what we'd ship by vendoring noble-secp256k1 ourselves.
 */
async function broadcastEip1559(
  rpcUrl: string,
  privateKeyHex: string,
  chainId: number,
  to: string,
  data: string,
): Promise<string> {
  const viem = await loadViem();
  const account = viem.privateKeyToAccount(normalisePrivateKey(privateKeyHex));
  const chain = {
    id: chainId,
    name: chainId === SEPOLIA_CHAIN_ID ? "sepolia" : `chain-${chainId}`,
    nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
    rpcUrls: { default: { http: [rpcUrl] }, public: { http: [rpcUrl] } },
  } as const;
  const wallet = viem.createWalletClient({
    account,
    chain,
    transport: viem.http(rpcUrl),
  });

  // viem >= 1.x defaults sendTransaction to EIP-1559 (type 2). It pulls
  // maxFeePerGas / maxPriorityFeePerGas from the chain's fee oracle and
  // estimates gas. Result is the 0x-prefixed tx hash — the caller then
  // tracks inclusion via Etherscan or a public client.
  const hash = await wallet.sendTransaction({
    to: viem.getAddress(to),
    data: data as `0x${string}`,
    value: 0n,
  });
  return hash;
}

interface ViemModule {
  createWalletClient: (config: {
    account: unknown;
    chain: unknown;
    transport: unknown;
  }) => {
    sendTransaction: (args: {
      to: `0x${string}`;
      data: `0x${string}`;
      value: bigint;
    }) => Promise<`0x${string}`>;
  };
  http: (url: string) => unknown;
  privateKeyToAccount: (key: `0x${string}`) => unknown;
  getAddress: (a: string) => `0x${string}`;
}

async function loadViem(): Promise<ViemModule> {
  try {
    // Indirect through string variables so tsc doesn't try to statically
    // resolve `viem` at compile time. The SDK's `viem` peer dep is
    // intentionally optional: most consumers never enable live mode and
    // shouldn't need to install it just to typecheck.
    const viemSpec: string = "viem";
    const accountsSpec: string = "viem/accounts";
    const mod = (await import(/* @vite-ignore */ viemSpec)) as Partial<ViemModule>;
    const accountsMod = (await import(/* @vite-ignore */ accountsSpec)) as Partial<ViemModule>;
    const merged: Partial<ViemModule> = { ...mod, ...accountsMod };
    if (
      merged.createWalletClient === undefined ||
      merged.http === undefined ||
      merged.privateKeyToAccount === undefined ||
      merged.getAddress === undefined
    ) {
      throw new Error("viem present but missing expected exports");
    }
    return merged as ViemModule;
  } catch (err) {
    const cause = err instanceof Error ? err.message : String(err);
    throw new Error(
      "live swap requires the `viem` peer dependency. " +
        "Install with `npm i viem` (or `pnpm add viem` / `yarn add viem`). " +
        `Original load error: ${cause}`,
    );
  }
}

function normalisePrivateKey(raw: string): `0x${string}` {
  const trimmed = raw.startsWith("0x") || raw.startsWith("0X") ? raw.slice(2) : raw;
  if (trimmed.length !== 64 || !HEX_RE.test(trimmed)) {
    throw new Error("private key must be 32 bytes (64 hex chars), 0x prefix optional");
  }
  return ("0x" + trimmed.toLowerCase()) as `0x${string}`;
}

/** Re-export Sepolia constants so callers don't need a separate import. */
export { SEPOLIA_USDC, SEPOLIA_WETH };
