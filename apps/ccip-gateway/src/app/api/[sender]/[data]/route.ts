/**
 * SBO3L CCIP-Read gateway endpoint (ENSIP-25 / EIP-3668).
 *
 *     GET /api/{sender}/{data}.json
 *
 * - `{sender}` is the OffchainResolver's address (0x-prefixed,
 *   40 hex chars; case-insensitive — the resolver lower-cases on the
 *   way out).
 * - `{data}` is the original `text(node, key)` or `addr(node)`
 *   calldata, hex-encoded with a `.json` suffix.
 *
 * Returns:
 *
 *     {"data":"0x...","ttl":60}
 *
 * `data` is the ABI-encoded `(bytes value, uint64 expires, bytes signature)`
 * tuple the OffchainResolver's callback verifies on-chain. Signing
 * happens in `lib/sign.ts` with `GATEWAY_PRIVATE_KEY` (Vercel env).
 */

import { NextResponse } from "next/server";
import type { Hex } from "viem";

import { decodeResolverCall } from "../../../../lib/ens.js";
import { lookupByNode } from "../../../../lib/records.js";
import {
  encodeEmptyStringResult,
  encodeStringResult,
  signGatewayResponse,
} from "../../../../lib/sign.js";

interface RouteParams {
  params: Promise<{ sender: string; data: string }>;
}

const HEX_ADDR_RE = /^0x[0-9a-fA-F]{40}$/;
const HEX_DATA_RE = /^0x[0-9a-fA-F]+\.json$/;

export async function GET(_req: Request, { params }: RouteParams) {
  const { sender: rawSender, data: rawData } = await params;

  if (!HEX_ADDR_RE.test(rawSender)) {
    return NextResponse.json(
      {
        error: "bad_request",
        reason: "sender must be 0x-prefixed 40-hex-char address",
      },
      { status: 400 },
    );
  }
  if (!HEX_DATA_RE.test(rawData)) {
    return NextResponse.json(
      {
        error: "bad_request",
        reason:
          "data must be 0x-prefixed ABI-encoded calldata with .json suffix",
      },
      { status: 400 },
    );
  }

  const sender = rawSender.toLowerCase() as Hex;
  // Strip `.json` suffix and normalise to lowercase.
  const callData = (rawData.slice(0, -".json".length).toLowerCase()) as Hex;

  const decoded = decodeResolverCall(callData);
  if (!decoded) {
    return NextResponse.json(
      {
        error: "unsupported_function",
        reason:
          "gateway only handles `text(bytes32,string)` and `addr(bytes32)` for now",
      },
      { status: 400 },
    );
  }

  const agent = lookupByNode(decoded.node);
  if (!agent) {
    // PublicResolver convention: unknown record returns empty string,
    // not a revert. We honour that — sign an empty result so the
    // caller's resolver callback can decode it cleanly.
    try {
      const signed = await signGatewayResponse({
        resolver: sender,
        callData,
        result: encodeEmptyStringResult(),
      });
      return NextResponse.json(signed, { status: 200 });
    } catch (err) {
      return signingFailure(err);
    }
  }

  let resultBytes: Hex;
  if (decoded.kind === "text") {
    const value = agent.records[decoded.key] ?? "";
    resultBytes = encodeStringResult(value);
  } else {
    // addr(node) — for now, return zero address. Agents don't have
    // their own EVM addresses bound at the resolver level in T-4-1;
    // T-4-2 ERC-8004 entry carries the address.
    resultBytes = encodeStringResult("");
  }

  try {
    const signed = await signGatewayResponse({
      resolver: sender,
      callData,
      result: resultBytes,
    });
    return NextResponse.json(signed, { status: 200 });
  } catch (err) {
    return signingFailure(err);
  }
}

function signingFailure(err: unknown) {
  const message = err instanceof Error ? err.message : String(err);
  return NextResponse.json(
    {
      error: "signing_failed",
      reason: message,
    },
    { status: 500 },
  );
}

export async function OPTIONS() {
  return new Response(null, {
    status: 204,
    headers: {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, OPTIONS",
      "Access-Control-Max-Age": "86400",
    },
  });
}
