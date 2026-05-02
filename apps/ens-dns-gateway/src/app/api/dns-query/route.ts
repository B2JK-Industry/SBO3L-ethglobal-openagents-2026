/**
 * `/dns-query` — DNS-over-HTTPS endpoint (RFC 8484).
 *
 * Accepts both DoH transport modes:
 *
 * 1. `GET /dns-query?dns=<base64url-binary-message>` (RFC 8484 §4.1.1).
 * 2. `POST /dns-query` with `Content-Type: application/dns-message`
 *    and the binary message as the body (RFC 8484 §4.1.2).
 *
 * Responses are binary `application/dns-message` per the spec, with
 * `Cache-Control: public, max-age=300, stale-while-revalidate=3600`
 * — 5 minutes fresh, 1 hour stale (per round 11 P3 spec).
 *
 * Resolution flow per question:
 *
 * - **TXT** for an SBO3L name (`*.sbo3lagent.eth` etc.) → returns
 *   one TXT answer per non-empty `sbo3l:*` text record on the
 *   agent's ENS resolver, formatted as `key=value` tokens.
 * - **A / AAAA** for an SBO3L name → resolves the agent's
 *   `sbo3l:endpoint` URL host through the upstream DoH-JSON
 *   resolver (Cloudflare 1.1.1.1 by default) and surfaces those
 *   answers under the SBO3L name.
 * - **Anything else** → no answers section, RCODE NOERROR. The
 *   gateway doesn't proxy generic DNS queries; clients should
 *   keep their regular DoH/DNS upstream alongside this gateway.
 *
 * Errors are surfaced in the DNS response code:
 * - SERVFAIL (RCODE 2) on RPC failure / network errors.
 * - NOERROR (RCODE 0) with empty answer when the resolver is set
 *   but the requested record isn't present (matches `dig` semantics).
 */

import dnsPacket from 'dns-packet';
import { NextResponse } from 'next/server';

import { isEnsName, resolveAddress, resolveTxt } from '@/lib/dns-resolve';

const DEFAULT_RPC_URL =
  process.env.SBO3L_ENS_RPC_URL ?? 'https://ethereum-rpc.publicnode.com';
const DEFAULT_NETWORK: 'mainnet' | 'sepolia' =
  (process.env.SBO3L_ENS_NETWORK as 'mainnet' | 'sepolia' | undefined) ??
  'mainnet';
const DEFAULT_UPSTREAM =
  process.env.SBO3L_DNS_UPSTREAM_DOH_URL ??
  'https://cloudflare-dns.com/dns-query';

const RESOLVER_OPTS = {
  rpcUrl: DEFAULT_RPC_URL,
  network: DEFAULT_NETWORK,
  upstreamDohUrl: DEFAULT_UPSTREAM,
};

const DOH_HEADERS = {
  'Content-Type': 'application/dns-message',
  'Cache-Control': 'public, max-age=300, stale-while-revalidate=3600',
};

const RCODE_NOERROR = 0;
const RCODE_SERVFAIL = 2;

export async function GET(request: Request) {
  const url = new URL(request.url);
  const dnsParam = url.searchParams.get('dns');
  if (!dnsParam) {
    return NextResponse.json(
      { error: 'missing `dns` query param (RFC 8484 §4.1.1 GET form)' },
      { status: 400 },
    );
  }
  let body: Buffer;
  try {
    body = Buffer.from(dnsParam, 'base64url');
  } catch (e) {
    return NextResponse.json(
      {
        error: `failed to base64url-decode dns param: ${(e as Error).message}`,
      },
      { status: 400 },
    );
  }
  return handleQuery(body);
}

export async function POST(request: Request) {
  const ct = request.headers.get('content-type') ?? '';
  if (!ct.toLowerCase().includes('application/dns-message')) {
    return NextResponse.json(
      {
        error:
          'POST /dns-query requires Content-Type: application/dns-message (RFC 8484 §4.1.2)',
      },
      { status: 415 },
    );
  }
  const body = Buffer.from(await request.arrayBuffer());
  return handleQuery(body);
}

async function handleQuery(body: Buffer): Promise<Response> {
  let query: dnsPacket.Packet;
  try {
    query = dnsPacket.decode(body);
  } catch (e) {
    return NextResponse.json(
      { error: `dns-packet decode failed: ${(e as Error).message}` },
      { status: 400 },
    );
  }

  const question = query.questions?.[0];
  if (!question) {
    const empty = dnsPacket.encode({
      type: 'response',
      id: query.id ?? 0,
      flags:
        dnsPacket.RECURSION_DESIRED |
        dnsPacket.RECURSION_AVAILABLE |
        RCODE_NOERROR,
      questions: [],
      answers: [],
    });
    return new Response(new Uint8Array(empty), { headers: DOH_HEADERS });
  }

  const name = question.name;
  const recordType = question.type;

  if (!isEnsName(name)) {
    // Out of scope. Return NOERROR + empty answers (NOT NXDOMAIN —
    // we don't claim the name doesn't exist; we just don't resolve
    // it). Clients should use their regular DoH upstream for
    // non-`.eth` names.
    const empty = dnsPacket.encode({
      type: 'response',
      id: query.id ?? 0,
      flags:
        dnsPacket.RECURSION_DESIRED |
        dnsPacket.RECURSION_AVAILABLE |
        RCODE_NOERROR,
      questions: query.questions,
      answers: [],
    });
    return new Response(new Uint8Array(empty), { headers: DOH_HEADERS });
  }

  let answers: dnsPacket.Answer[] = [];
  let rcodeFlag = RCODE_NOERROR;

  try {
    if (recordType === 'TXT') {
      const records: import('@/lib/dns-resolve').DnsAnswer[] = await resolveTxt(
        name,
        RESOLVER_OPTS,
      );
      answers = records.map((r) => ({
        name: r.name,
        type: 'TXT' as const,
        ttl: r.ttl,
        // dns-packet TXT data accepts string | string[] | Buffer | Buffer[].
        data: r.value,
      }));
    } else if (recordType === 'A' || recordType === 'AAAA') {
      const records: import('@/lib/dns-resolve').DnsAnswer[] =
        await resolveAddress(name, recordType, RESOLVER_OPTS);
      answers = records.map((r) => ({
        name: r.name,
        type: recordType,
        ttl: r.ttl,
        data: r.value,
      }));
    } else {
      // Other types (MX, CNAME, NS) not implemented yet. NOERROR +
      // empty answers — same shape as a DNS server with no record
      // for the type.
      answers = [];
    }
  } catch (e) {
    // RPC failure / ENS lookup error → SERVFAIL.
    console.error('dns-query resolve error:', (e as Error).message);
    rcodeFlag = RCODE_SERVFAIL;
    answers = [];
  }

  const response = dnsPacket.encode({
    type: 'response',
    id: query.id ?? 0,
    flags:
      dnsPacket.RECURSION_DESIRED | dnsPacket.RECURSION_AVAILABLE | rcodeFlag,
    questions: query.questions,
    answers,
  });
  return new Response(new Uint8Array(response), { headers: DOH_HEADERS });
}
