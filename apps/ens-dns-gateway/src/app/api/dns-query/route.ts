/**
 * `/dns-query` — DNS-over-HTTPS endpoint (RFC 8484).
 *
 * Accepts both DoH transport modes:
 *
 * 1. `GET /dns-query?dns=<base64url-binary-message>` (RFC 8484 §4.1.1).
 * 2. `POST /dns-query` with `Content-Type: application/dns-message`
 *    and the binary message as the body (RFC 8484 §4.1.2).
 *
 * Responses are binary `application/dns-message` per the spec.
 *
 * Scaffold: the actual DNS-message parser/serializer (`dns-packet`
 * or hand-rolled) wires here. The resolution *logic* lives in
 * `src/lib/dns-resolve.ts`; the HTTP-binary glue lives in this
 * route. The scaffold returns a 501 Not Implemented for now with
 * a JSON body explaining the missing pieces — operators get a
 * clear error rather than a silent failure when they point a DoH
 * client at the deploy.
 *
 * To finish the scaffold:
 *
 * 1. `npm install dns-packet` (Node.js DNS message codec).
 * 2. Implement `decodeDnsMessage(body)` → `{ name, type }`.
 * 3. Route to `resolveTxt` / `resolveAddress` from
 *    `src/lib/dns-resolve.ts` based on `type`.
 * 4. Implement `encodeDnsResponse(question, answers)` → Buffer.
 * 5. Return the Buffer with `Content-Type: application/dns-message`.
 *
 * Until then: GET returns 501 + a usage hint; POST returns 501 +
 * the same hint. The route exists so a Vercel deploy of this app
 * has a real path under `/dns-query`.
 */

import { NextResponse } from 'next/server';

const SCAFFOLD_NOTICE = {
  status: 'not_implemented',
  reason:
    'ENS DNS gateway scaffold. The DNS-over-HTTPS wire codec is not yet wired. See apps/ens-dns-gateway/DEPLOY.md for the finish-the-scaffold checklist.',
  upstream:
    'For ENS resolution today, use sbo3l agent verify-ens or the CCIP-Read gateway at sbo3l-ccip.vercel.app.',
};

export async function GET() {
  return NextResponse.json(SCAFFOLD_NOTICE, { status: 501 });
}

export async function POST() {
  return NextResponse.json(SCAFFOLD_NOTICE, { status: 501 });
}
