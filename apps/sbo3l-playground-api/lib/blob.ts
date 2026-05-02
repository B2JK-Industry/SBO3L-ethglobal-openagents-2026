// Vercel Blob — capsule storage with 7-day TTL.
//
// SKELETON. Add @vercel/blob once Daniel runs
// `vercel blob store create sbo3l-playground-blob` (DEPLOY.md
// step 4).
//
// import { put, head, del } from "@vercel/blob";

const TTL_SECONDS = 7 * 24 * 60 * 60;

export async function storeCapsule(capsuleId: string, _capsuleJson: string): Promise<{ url: string }> {
  // TODO: put(`capsules/${capsuleId}.json`, capsuleJson, {
  //   access: "public", contentType: "application/json", cacheControl: `public, max-age=${TTL_SECONDS}`
  // })
  throw new Error(`blob.storeCapsule(${capsuleId}): skeleton — wire @vercel/blob per DEPLOY.md`);
}

export async function fetchCapsule(_capsuleId: string): Promise<string | null> {
  // TODO: fetch the public URL produced by put() above; 404 if expired.
  return null;
}

export const _ttl_seconds = TTL_SECONDS; // exported for tests once they exist
