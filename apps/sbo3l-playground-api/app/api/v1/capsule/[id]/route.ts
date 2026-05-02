import { NextResponse } from "next/server";

// Capsule retrieval by ID. Skeleton: validates the ID shape, returns
// placeholder until Vercel Blob is wired.

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

const ID_RE = /^cap_[A-Z0-9]{20,30}$/;

interface Params { params: Promise<{ id: string }> }

export async function GET(_: Request, { params }: Params): Promise<NextResponse> {
  const { id } = await params;
  if (!ID_RE.test(id)) {
    return NextResponse.json(
      { error: "invalid_id", detail: "capsule id must match cap_[A-Z0-9]{20,30}" },
      { status: 400 },
    );
  }

  // TODO: import { fetchCapsule } from "@/lib/blob" → fetch from
  // Vercel Blob; 404 if not found OR expired (7-day TTL).
  return NextResponse.json({
    schema: "sbo3l.playground_api.placeholder.v1",
    status: "skeleton",
    todo: "wire Vercel Blob fetch per DEPLOY.md",
    requested_id: id,
    github: "https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/tree/main/apps/sbo3l-playground-api",
  });
}
