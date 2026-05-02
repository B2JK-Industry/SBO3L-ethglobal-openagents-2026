// Vercel KV — used as a token bucket for per-IP rate limiting.
//
// SKELETON. Add @vercel/kv once Daniel runs
// `vercel kv create sbo3l-playground-kv` (DEPLOY.md step 3).
//
// import { kv } from "@vercel/kv";

const RATE_LIMIT_PER_MIN = 10;

export interface RateLimitResult {
  allowed: boolean;
  remaining: number;
  reset_in_seconds: number;
}

export async function checkRateLimit(_ip: string): Promise<RateLimitResult> {
  // TODO: INCR token bucket key + EXPIRE if first request in window.
  // Sliding window counter: key = `rl:<ip>:<minute_bucket>`
  // Returns { allowed: count <= RATE_LIMIT_PER_MIN, ... }
  return { allowed: true, remaining: RATE_LIMIT_PER_MIN, reset_in_seconds: 60 };
}
