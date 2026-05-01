/**
 * SBO3L CCIP-Read gateway endpoint (ENSIP-25 / EIP-3668).
 *
 * URL shape per the spec:
 *
 *     GET /api/{sender}/{data}.json
 *
 * - {sender} = 0x-prefixed 40-hex-char OffchainResolver contract
 *              address (lowercased).
 * - {data}   = 0x-prefixed ABI-encoded calldata of the original
 *              query (e.g. text(node, key), addr(node)).
 *
 * Response (200) per the convention:
 *
 *     {
 *       "data": "0x...",   // ABI-encoded (bytes value, uint64 expires, bytes signature)
 *       "ttl":  60         // gateway hint for clients/proxies
 *     }
 *
 * STATUS: pre-scaffold stub. Returns 501 Not Implemented until the
 * T-4-1 main PR ships the record source + signing logic. The route's
 * shape, error envelope, and CORS headers are pinned now so deployers
 * can wire the OffchainResolver against the eventual production URL
 * without surprises.
 *
 * Design doc: docs/design/T-4-1-ccip-read-prep.md
 * Ticket:     T-4-1 in docs/win-backlog/06-phase-2.md
 */

import { NextResponse } from "next/server";

interface RouteParams {
  params: Promise<{ sender: string; data: string }>;
}

const HEX_ADDR_RE = /^0x[0-9a-fA-F]{40}$/;
const HEX_DATA_RE = /^0x[0-9a-fA-F]+\.json$/;

export async function GET(_req: Request, { params }: RouteParams) {
  const { sender, data } = await params;

  if (!HEX_ADDR_RE.test(sender)) {
    return NextResponse.json(
      { error: "bad_request", reason: "sender must be 0x-prefixed 40-hex-char address" },
      { status: 400 },
    );
  }

  if (!HEX_DATA_RE.test(data)) {
    return NextResponse.json(
      { error: "bad_request", reason: "data must be 0x-prefixed ABI-encoded calldata with .json suffix" },
      { status: 400 },
    );
  }

  // T-4-1 main PR fills in:
  //   1. Decode the calldata to extract (node, key) for text()
  //      or node for addr().
  //   2. Look up the value in the record source (static JSON in
  //      apps/ccip-gateway/data/records.json initially).
  //   3. ABI-encode (value, expires, signature) where signature is
  //      EthSigner over keccak256(value || expires || extraData)
  //      using GATEWAY_PRIVATE_KEY (Vercel env).
  //   4. Return { data: "0x...", ttl: 60 }.
  return NextResponse.json(
    {
      error: "not_implemented",
      reason:
        "CCIP-Read gateway is pre-scaffold. T-4-1 main PR will land the record lookup + signing logic.",
      design_doc:
        "https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/design/T-4-1-ccip-read-prep.md",
    },
    { status: 501 },
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
