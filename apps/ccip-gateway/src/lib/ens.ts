/**
 * ENS namehash + selector helpers for the CCIP-Read gateway.
 *
 * The gateway only needs to *decode* incoming `text(node, key)` and
 * `addr(node)` calldata to know what to look up — no namehash recompute
 * is needed because the inbound calldata already carries the namehash.
 * We do however need to map namehash → FQDN for the records lookup,
 * which is impractical without a known agent set; T-4-1 ships a static
 * reverse-lookup table in `records.ts` keyed by FQDN, so the gateway
 * iterates known agents and matches namehashes precomputed at startup.
 */

import { keccak256, toBytes } from "viem";

export const TEXT_SELECTOR = "0x59d1d43c"; // keccak256("text(bytes32,string)")[..4]
export const ADDR_SELECTOR = "0x3b3b57de"; // keccak256("addr(bytes32)")[..4]

/**
 * EIP-137 namehash. Walks labels right-to-left:
 *   namehash("")        = 32 zero bytes
 *   namehash("x.y")     = keccak256(namehash("y") || keccak256("x"))
 */
export function namehash(domain: string): `0x${string}` {
  if (!domain) {
    return ("0x" + "00".repeat(32)) as `0x${string}`;
  }
  let node = new Uint8Array(32); // 32 zero bytes
  const labels = domain.split(".").reverse();
  for (const label of labels) {
    if (!label) {
      throw new Error(`namehash: empty label in ${domain}`);
    }
    const labelHash = keccak256(toBytes(label));
    const buf = new Uint8Array(64);
    buf.set(node, 0);
    buf.set(toBytes(labelHash), 32);
    node = new Uint8Array(toBytes(keccak256(buf)));
  }
  return ("0x" +
    Array.from(node)
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("")) as `0x${string}`;
}

/** Decoded shape of a `text(bytes32, string)` calldata payload. */
export interface DecodedTextCall {
  kind: "text";
  node: `0x${string}`;
  key: string;
}

/** Decoded shape of an `addr(bytes32)` calldata payload. */
export interface DecodedAddrCall {
  kind: "addr";
  node: `0x${string}`;
}

export type DecodedCall = DecodedTextCall | DecodedAddrCall;

/**
 * Decode the calldata that an OffchainResolver receives at `resolve()`.
 * Currently supports `text` and `addr`; other functions surface as
 * `null` (the gateway returns 400 for those).
 */
export function decodeResolverCall(data: `0x${string}`): DecodedCall | null {
  const selector = data.slice(0, 10).toLowerCase();
  if (selector === TEXT_SELECTOR) {
    return decodeTextCall(data);
  }
  if (selector === ADDR_SELECTOR) {
    return decodeAddrCall(data);
  }
  return null;
}

function decodeTextCall(data: `0x${string}`): DecodedTextCall {
  // Layout (after selector):
  //   word 0: bytes32 node              (32 B)
  //   word 1: offset to string key      (32 B; constant 0x40)
  //   word 2: string length             (32 B)
  //   word 3+: string bytes, padded to multiple of 32
  const body = data.slice(10);                       // hex without selector
  const node = ("0x" + body.slice(0, 64)) as `0x${string}`;
  // word 1 (offset) is at hex chars 64..128 — we don't validate it
  // strictly; clients always emit 0x40 for a single trailing-string
  // tuple. word 2 (length) is at chars 128..192.
  const lenHex = body.slice(128, 192);
  const len = parseInt(lenHex, 16);
  const keyHex = body.slice(192, 192 + len * 2);
  const key = Buffer.from(keyHex, "hex").toString("utf-8");
  return { kind: "text", node, key };
}

function decodeAddrCall(data: `0x${string}`): DecodedAddrCall {
  const node = ("0x" + data.slice(10, 10 + 64)) as `0x${string}`;
  return { kind: "addr", node };
}
