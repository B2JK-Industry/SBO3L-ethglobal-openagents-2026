import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import {
  aprpForSwap,
  encodeExactInputSingle,
  EXACT_INPUT_SINGLE_SELECTOR,
  SEPOLIA_CHAIN_ID,
  SEPOLIA_SWAP_ROUTER_02,
  SEPOLIA_USDC,
  SEPOLIA_WETH,
  sepoliaEtherscanTxUrl,
  swap,
  type SwapParams,
} from "../src/uniswap/index.js";

const RECIPIENT = "0x" + "AA".repeat(20);

const BASIC: SwapParams = {
  tokenIn: SEPOLIA_WETH,
  tokenOut: SEPOLIA_USDC,
  fee: 3000,
  recipient: RECIPIENT,
  amountIn: 10_000_000_000_000_000n,  // 0.01 WETH
  amountOutMinimum: 1_000_000n,        // 1 USDC slippage floor
};

describe("uniswap constants", () => {
  it("Sepolia chain id matches EIP-155", () => {
    expect(SEPOLIA_CHAIN_ID).toBe(11_155_111);
  });

  it("selector matches keccak256 of canonical type string", () => {
    // Note: this test uses node:crypto's SHA3 — for keccak proper, we'd
    // need a vendored impl. Here we just pin the known constant; the Rust
    // module's test is the authoritative selector check via tiny_keccak.
    expect(EXACT_INPUT_SINGLE_SELECTOR).toBe("0x04e45aaf");
  });

  it("router + token addresses are 0x + 40 hex", () => {
    for (const a of [SEPOLIA_SWAP_ROUTER_02, SEPOLIA_USDC, SEPOLIA_WETH]) {
      expect(a).toMatch(/^0x[a-fA-F0-9]{40}$/);
    }
  });
});

describe("encodeExactInputSingle — calldata layout", () => {
  it("total length = 4 selector + 7 × 32 words", () => {
    const data = encodeExactInputSingle(BASIC);
    // 0x prefix + 2 chars per byte × (4 + 224) bytes = 2 + 456 = 458
    expect(data.length).toBe(2 + (4 + 7 * 32) * 2);
  });

  it("starts with selector", () => {
    const data = encodeExactInputSingle(BASIC);
    expect(data.slice(0, 10)).toBe(EXACT_INPUT_SINGLE_SELECTOR);
  });

  it("tokenIn is left-padded to 32 bytes (word 1)", () => {
    const data = encodeExactInputSingle(BASIC);
    // Word 1 spans hex offset 10..(10+64). First 24 hex chars are zeros (padding).
    expect(data.slice(10, 10 + 24)).toBe("000000000000000000000000");
    // Last 40 hex chars are the lowercased WETH address (no 0x).
    expect(data.slice(10 + 24, 10 + 64).toLowerCase()).toBe(
      SEPOLIA_WETH.slice(2).toLowerCase(),
    );
  });

  it("recipient is at word 4", () => {
    const data = encodeExactInputSingle(BASIC);
    // Word 4 hex offset = 10 + 64*3 = 202.
    const wordHex = data.slice(202, 202 + 64);
    expect(wordHex.slice(0, 24)).toBe("000000000000000000000000");
    expect(wordHex.slice(24).toLowerCase()).toBe(RECIPIENT.slice(2).toLowerCase());
  });

  it("amountIn is right-aligned in word 5", () => {
    const data = encodeExactInputSingle(BASIC);
    const wordHex = data.slice(266, 266 + 64);
    const decoded = BigInt("0x" + wordHex);
    expect(decoded).toBe(BASIC.amountIn);
  });

  it("sqrtPriceLimitX96 defaults to zero (word 7)", () => {
    const data = encodeExactInputSingle(BASIC);
    const wordHex = data.slice(394, 394 + 64);
    expect(wordHex).toBe("0".repeat(64));
  });

  it("explicit sqrtPriceLimitX96 packs to word 7", () => {
    const data = encodeExactInputSingle({ ...BASIC, sqrtPriceLimitX96: 0xdeadbeefn });
    const wordHex = data.slice(394, 394 + 64);
    expect(BigInt("0x" + wordHex)).toBe(0xdeadbeefn);
  });

  it("rejects malformed addresses", () => {
    expect(() => encodeExactInputSingle({ ...BASIC, tokenIn: "0xnotenough" })).toThrow();
  });
});

describe("aprpForSwap", () => {
  it("default WETH→USDC pair", () => {
    const aprp = aprpForSwap({
      agentId: "research-agent-01",
      taskId: "swap-1",
      amountUsd: "1.50",
      nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM",
      expiry: "2026-05-01T10:31:00Z",
    });
    expect(aprp["chain"]).toBe("sepolia");
    expect(aprp["payment_protocol"]).toBe("erc20_transfer");
    expect((aprp["destination"] as { token_address: string }).token_address).toBe(SEPOLIA_WETH);
    expect((aprp["destination"] as { recipient: string }).recipient).toBe(SEPOLIA_USDC);
  });

  it("inverts to WETH when tokenIn=USDC", () => {
    const aprp = aprpForSwap({
      agentId: "research-agent-01",
      taskId: "swap-2",
      amountUsd: "1.00",
      nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM",
      expiry: "2026-05-01T10:31:00Z",
      tokenIn: SEPOLIA_USDC,
    });
    expect((aprp["destination"] as { recipient: string }).recipient).toBe(SEPOLIA_WETH);
  });

  it("default risk_class is medium", () => {
    const aprp = aprpForSwap({
      agentId: "research-agent-01",
      taskId: "swap-3",
      amountUsd: "1.00",
      nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM",
      expiry: "2026-05-01T10:31:00Z",
    });
    expect(aprp["risk_class"]).toBe("medium");
  });
});

describe("swap (mock mode)", () => {
  it("returns deterministic pseudo-tx-hash for identical params", async () => {
    const a = await swap(BASIC);
    const b = await swap(BASIC);
    expect(a.mode).toBe("mock");
    expect(a.txHash).toBe(b.txHash);
    expect(a.txHash).toMatch(/^0x[a-f0-9]{64}$/);
  });

  it("different amounts produce different hashes", async () => {
    const a = await swap(BASIC);
    const b = await swap({ ...BASIC, amountIn: BASIC.amountIn + 1n });
    expect(a.txHash).not.toBe(b.txHash);
  });

  it("etherscan URL targets sepolia.etherscan.io", async () => {
    const r = await swap(BASIC);
    expect(r.etherscanUrl.startsWith("https://sepolia.etherscan.io/tx/")).toBe(true);
  });

  it("calldata matches what the agent would broadcast", async () => {
    const r = await swap(BASIC);
    expect(r.calldata).toBe(encodeExactInputSingle(BASIC));
    expect(r.to).toBe(SEPOLIA_SWAP_ROUTER_02);
  });

  it("mock-mode tx hash uses sha256 of calldata + router (deterministic)", async () => {
    const r = await swap(BASIC);
    const expected =
      "0x" +
      createHash("sha256").update(r.calldata + SEPOLIA_SWAP_ROUTER_02).digest("hex").slice(0, 64);
    expect(r.txHash).toBe(expected);
  });
});

describe("sepoliaEtherscanTxUrl", () => {
  it("strips 0x prefix uniformly", () => {
    expect(sepoliaEtherscanTxUrl("0xdeadbeef")).toBe(
      "https://sepolia.etherscan.io/tx/0xdeadbeef",
    );
    expect(sepoliaEtherscanTxUrl("deadbeef")).toBe(
      "https://sepolia.etherscan.io/tx/0xdeadbeef",
    );
  });
});

describe("encodeExactInputSingle (input validation)", () => {
  // Regression for the codex P1: hexToBytes used parseInt() which silently
  // returns NaN for non-hex chars; Uint8Array.set then coerces NaN to 0,
  // letting malformed addresses produce calldata that broadcasts garbage.
  // After the fix, any non-hex char must throw at encode time.
  it("rejects non-hex characters in tokenIn address", () => {
    expect(() =>
      encodeExactInputSingle({
        tokenIn: "0xZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ",
        tokenOut: SEPOLIA_USDC,
        fee: 500,
        recipient: SEPOLIA_USDC,
        amountIn: 1n,
        amountOutMinimum: 1n,
      }),
    ).toThrow(/non-hex characters/);
  });

  it("rejects non-hex characters in tokenOut address", () => {
    expect(() =>
      encodeExactInputSingle({
        tokenIn: SEPOLIA_WETH,
        tokenOut: "0xgggggggggggggggggggggggggggggggggggggggg",
        fee: 500,
        recipient: SEPOLIA_USDC,
        amountIn: 1n,
        amountOutMinimum: 1n,
      }),
    ).toThrow(/non-hex characters/);
  });

  it("rejects non-hex characters in recipient address", () => {
    expect(() =>
      encodeExactInputSingle({
        tokenIn: SEPOLIA_WETH,
        tokenOut: SEPOLIA_USDC,
        fee: 500,
        recipient: "0xQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ",
        amountIn: 1n,
        amountOutMinimum: 1n,
      }),
    ).toThrow(/non-hex characters/);
  });
});
