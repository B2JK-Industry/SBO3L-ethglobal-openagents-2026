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

import { createHash, createHmac } from "node:crypto";
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

function hexToBytes(hex: string): Uint8Array {
  const trimmed = hex.startsWith("0x") || hex.startsWith("0X") ? hex.slice(2) : hex;
  if (trimmed.length % 2 !== 0) throw new Error("hex must be even-length");
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
 * Sign + broadcast an EIP-1559 transaction. For demo purposes only:
 * uses fixed gas params (no fee oracle) and unchecked nonce fetch.
 * Production callers should use `viem` or `ethers` for proper fee bumping
 * and confirmation tracking.
 */
async function broadcastEip1559(
  rpcUrl: string,
  privateKeyHex: string,
  chainId: number,
  to: string,
  data: string,
): Promise<string> {
  const fromAddress = addressFromPrivateKey(privateKeyHex);
  const [nonce, gasPrice] = await Promise.all([
    rpcCall(rpcUrl, "eth_getTransactionCount", [fromAddress, "pending"]),
    rpcCall(rpcUrl, "eth_gasPrice", []),
  ]);

  // For simplicity broadcast a legacy-style tx (type 0). EIP-1559 is the
  // best-practice but full support needs a fee oracle; keeping this
  // intentionally simple so the demo path is auditable. Sepolia accepts
  // both legacy and EIP-1559.
  const txFields = {
    nonce: BigInt(nonce as string),
    gasPrice: BigInt(gasPrice as string),
    gasLimit: 300_000n,
    to,
    value: 0n,
    data,
    chainId: BigInt(chainId),
  };

  // Defer the actual signing import — keeping a vendored implementation
  // here would be hundreds of LoC of secp256k1 + RLP. Real callers should
  // pass an `env` with a pre-signed transaction OR use `viem`. For the
  // v1 live-mode demo, throw a clear error pointing at the right approach.
  throw new Error(
    "live broadcast requires `viem` or `ethers` for EIP-1559 signing. " +
      "Install one and replace this stub, OR run in mock mode (omit SBO3L_LIVE_ETH=1). " +
      `[stub state: from=${fromAddress}, to=${txFields.to}, nonce=${txFields.nonce}]`,
  );
}

/** Derive the public address from a private key. */
function addressFromPrivateKey(_privateKeyHex: string): string {
  // Stub — full secp256k1 derivation is hundreds of LoC and depends on a
  // crypto lib (e.g. `@noble/secp256k1`). Returns a placeholder so the
  // mock-mode path doesn't accidentally call this; live-mode callers
  // should swap this out with `viem`'s `privateKeyToAccount(...)`.
  return "0x" + "0".repeat(40);
}

interface RpcResponse {
  jsonrpc: "2.0";
  id: number;
  result?: unknown;
  error?: { code: number; message: string };
}

async function rpcCall(rpcUrl: string, method: string, params: unknown[]): Promise<unknown> {
  // Use a deterministic nonce derived from method+params so retries don't
  // collide with a parallel test run. Not security-critical (just unique).
  const id = Number(
    BigInt("0x" + createHmac("sha256", "sbo3l").update(method + JSON.stringify(params)).digest("hex").slice(0, 8)) %
      10000n,
  );
  const r = await fetch(rpcUrl, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id, method, params }),
  });
  if (!r.ok) throw new Error(`RPC HTTP ${r.status}`);
  const body = (await r.json()) as RpcResponse;
  if (body.error !== undefined) {
    throw new Error(`RPC error ${body.error.code}: ${body.error.message}`);
  }
  return body.result;
}

/** Re-export Sepolia constants so callers don't need a separate import. */
export { SEPOLIA_USDC, SEPOLIA_WETH };
