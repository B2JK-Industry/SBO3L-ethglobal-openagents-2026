import { NextResponse } from "next/server";

// Real decision endpoint. Skeleton: validates request shape, returns
// a placeholder until WASM + Postgres + signing key are wired.

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

interface DecideRequest {
  aprp?: unknown;
  policy?: unknown;
}

export async function POST(req: Request): Promise<NextResponse> {
  let body: DecideRequest;
  try {
    body = (await req.json()) as DecideRequest;
  } catch {
    return NextResponse.json(
      { error: "invalid_body", detail: "request body is not valid JSON" },
      { status: 400 },
    );
  }
  if (!body.aprp || typeof body.aprp !== "object") {
    return NextResponse.json(
      { error: "missing_aprp", detail: "request must include `aprp` object" },
      { status: 400 },
    );
  }
  if (!body.policy || typeof body.policy !== "string") {
    return NextResponse.json(
      { error: "missing_policy", detail: "request must include `policy` string (TOML)" },
      { status: 400 },
    );
  }

  // TODO: replace with real pipeline once wired:
  //   1. import { decideAprp } from "@/lib/wasm-loader" → load
  //      sbo3l-core wasm32-wasi module + call into it
  //   2. import { auditAppend } from "@/lib/db" → append decision
  //      to Postgres audit_events with hash chain
  //   3. import { signReceipt } from "@/lib/signer" → Ed25519 sign
  //      with the per-deploy key
  //   4. import { storeCapsule } from "@/lib/blob" → write capsule
  //      JSON to Vercel Blob with 7-day TTL
  //   5. return { decision, capsule_id, audit_event_id }

  return NextResponse.json({
    schema: "sbo3l.playground_api.placeholder.v1",
    status: "skeleton",
    todo: "wire WASM + Postgres + signer + Blob per DEPLOY.md",
    received: {
      aprp_keys: Object.keys(body.aprp as Record<string, unknown>),
      policy_bytes: (body.policy as string).length,
    },
    github: "https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/tree/main/apps/sbo3l-playground-api",
  });
}
