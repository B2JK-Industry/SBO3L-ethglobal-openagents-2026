import { createServer } from "node:http";

import linearWebhook from "../api/linear-webhook.js";
import health from "../api/health.js";

/**
 * Local dev shim — mounts the Vercel-style handlers on a plain Node http
 * server so `npm run dev` exposes them at http://localhost:3000.
 *
 * Production deployment runs the same handlers on Vercel Functions; this
 * file is not packaged for prod (excluded from tsconfig.build.json).
 */
const PORT = Number.parseInt(process.env["PORT"] ?? "3000", 10);

const server = createServer((req, res) => {
  void (async () => {
    try {
      if (req.url === "/api/linear-webhook") {
        await linearWebhook(req, res);
      } else if (req.url === "/api/health" || req.url === "/") {
        health(req, res);
      } else {
        res.statusCode = 404;
        res.end("not found");
      }
    } catch (err) {
      res.statusCode = 500;
      res.end(`error: ${(err as Error).message}`);
    }
  })();
});

server.listen(PORT, () => {
  // eslint-disable-next-line no-console
  console.log(`orchestrator listening on http://localhost:${PORT}`);
});
