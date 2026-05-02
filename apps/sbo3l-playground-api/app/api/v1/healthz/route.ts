import { NextResponse } from "next/server";

// Health probe. Wired even when the rest of the API is in skeleton
// mode so Vercel's edge can serve traffic + Daniel can verify the
// deploy succeeded before provisioning the data plane.

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET(): Promise<NextResponse> {
  const env = {
    has_postgres: typeof process.env.POSTGRES_URL === "string" && process.env.POSTGRES_URL.length > 0,
    has_kv: typeof process.env.KV_REST_API_URL === "string" && process.env.KV_REST_API_URL.length > 0,
    has_blob: typeof process.env.BLOB_READ_WRITE_TOKEN === "string" && process.env.BLOB_READ_WRITE_TOKEN.length > 0,
    has_signing_key: typeof process.env.SBO3L_PLAYGROUND_SIGNING_KEY === "string" && process.env.SBO3L_PLAYGROUND_SIGNING_KEY.length > 0,
  };
  const provisioned = env.has_postgres && env.has_kv && env.has_blob && env.has_signing_key;
  return NextResponse.json({
    status: provisioned ? "ok" : "skeleton",
    version: process.env.VERCEL_GIT_COMMIT_SHA?.slice(0, 7) ?? "dev",
    env,
    wasm_loaded: false,
    note: provisioned
      ? "All env vars present; route handlers will switch to live mode once their TODOs are wired."
      : "Skeleton mode — provision Vercel Postgres + KV + Blob + signing key per DEPLOY.md to activate.",
  });
}
