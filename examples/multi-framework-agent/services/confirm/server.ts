/**
 * Confirm service — AutoGen framework boundary.
 *
 * Receives a `next_action` APRP from the execute step, gates it through
 * SBO3L via @sbo3l/autogen's function descriptor, and returns the signed
 * receipt as the final demo step. Different framework, same SBO3L audit
 * chain.
 */

import http from "node:http";
import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lFunction, type SBO3LClientLike } from "@sbo3l/autogen";

const ENDPOINT = process.env["SBO3L_ENDPOINT"] ?? "http://sbo3l-server:8730";
const KH_WORKFLOW_ID = "m4t4cnpmhv8qquce3bv3c";

const client = new SBO3LClient({ endpoint: ENDPOINT });
const sbo3lPay = sbo3lFunction({ client: client as unknown as SBO3LClientLike });

async function readBody(req: http.IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (c: Buffer) => chunks.push(c));
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf-8")));
    req.on("error", reject);
  });
}

const server = http.createServer(async (req, res) => {
  res.setHeader("Content-Type", "application/json");

  if (req.url === "/health") {
    res.statusCode = 200;
    res.end(JSON.stringify({ status: "ok", framework: "autogen" }));
    return;
  }

  if (req.method === "POST" && req.url === "/confirm") {
    try {
      const body = await readBody(req);
      const { aprp } = JSON.parse(body) as { aprp?: Record<string, unknown> };
      if (typeof aprp !== "object" || aprp === null) {
        res.statusCode = 400;
        res.end(JSON.stringify({ decision: "error", deny_code: "input.no_aprp", step: "confirm" }));
        return;
      }
      const result = await sbo3lPay.call(aprp);
      res.statusCode = 200;
      res.end(
        JSON.stringify({
          ...result,
          step: "confirm",
          framework: "autogen",
          kh_workflow_id: KH_WORKFLOW_ID,
        }),
      );
    } catch (e) {
      res.statusCode = 500;
      res.end(
        JSON.stringify({
          decision: "error",
          error: e instanceof Error ? e.message : String(e),
          step: "confirm",
          framework: "autogen",
        }),
      );
    }
    return;
  }

  res.statusCode = 404;
  res.end(JSON.stringify({ error: "not found" }));
});

const port = Number(process.env["PORT"] ?? 8003);
server.listen(port, "0.0.0.0", () => {
  console.log(`▶ confirm (autogen) listening on :${port} → SBO3L=${ENDPOINT}`);
});
