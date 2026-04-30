import type { IncomingMessage, ServerResponse } from "node:http";

/** Liveness probe — returns 200 OK with version + commit. */
export default function handler(
  _req: IncomingMessage,
  res: ServerResponse,
): void {
  res.statusCode = 200;
  res.setHeader("Content-Type", "application/json");
  res.end(
    JSON.stringify({
      service: "sbo3l-orchestrator",
      version: "0.1.0",
      commit: process.env["VERCEL_GIT_COMMIT_SHA"] ?? "dev",
    }),
  );
}
