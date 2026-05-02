/**
 * ENS-backed DNS resolver — scaffold.
 *
 * Inputs an FQDN + record type (A / AAAA / TXT). For SBO3L
 * `*.sbo3lagent.eth` names we resolve the agent's `sbo3l:endpoint`
 * text record via viem, then resolve the host of that URL through
 * the public upstream resolver. For non-SBO3L names we forward
 * directly to the upstream.
 *
 * This is the *logic* layer. The HTTP-binary glue (decoding the
 * incoming RFC 8484 message, encoding the response) lives in the
 * `/dns-query` route handler.
 */

import { createPublicClient, http, namehash } from 'viem';
import { mainnet, sepolia } from 'viem/chains';

export type DnsRecordType = 'A' | 'AAAA' | 'TXT';

export interface DnsAnswer {
  name: string;
  type: DnsRecordType;
  ttl: number;
  value: string;
}

const SBO3L_TEXT_KEYS = [
  'sbo3l:agent_id',
  'sbo3l:endpoint',
  'sbo3l:policy_hash',
  'sbo3l:audit_root',
  'sbo3l:proof_uri',
] as const;

const RESOLVER_ABI = [
  {
    name: 'text',
    type: 'function',
    stateMutability: 'view',
    inputs: [
      { name: 'node', type: 'bytes32' },
      { name: 'key', type: 'string' },
    ],
    outputs: [{ name: '', type: 'string' }],
  },
] as const;

const REGISTRY_ABI = [
  {
    name: 'resolver',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: 'node', type: 'bytes32' }],
    outputs: [{ name: '', type: 'address' }],
  },
] as const;

const ENS_REGISTRY = '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e' as const;

const DEFAULT_TTL_SECONDS = 60;

export function isEnsName(name: string): boolean {
  return name.toLowerCase().endsWith('.eth') || name.toLowerCase().endsWith('.eth.');
}

function normaliseEnsName(name: string): string {
  let n = name.toLowerCase();
  if (n.endsWith('.')) n = n.slice(0, -1);
  return n;
}

export interface ResolveOptions {
  rpcUrl: string;
  network: 'mainnet' | 'sepolia';
  upstreamDohUrl: string;
}

/**
 * Resolve a TXT record for an SBO3L name. Returns one DnsAnswer per
 * non-empty `sbo3l:*` text record found; the value is formatted as
 * `k=v` so a `dig`-style consumer reads them as standard RFC-style
 * TXT tokens.
 */
export async function resolveTxt(
  name: string,
  opts: ResolveOptions
): Promise<DnsAnswer[]> {
  const ensName = normaliseEnsName(name);
  if (!isEnsName(ensName)) {
    throw new Error(`resolveTxt: ${name} is not an ENS name`);
  }

  const client = createPublicClient({
    chain: opts.network === 'mainnet' ? mainnet : sepolia,
    transport: http(opts.rpcUrl),
  });

  const node = namehash(ensName);
  const resolverAddress = (await client.readContract({
    address: ENS_REGISTRY,
    abi: REGISTRY_ABI,
    functionName: 'resolver',
    args: [node],
  })) as `0x${string}`;

  if (
    resolverAddress.toLowerCase() ===
    '0x0000000000000000000000000000000000000000'
  ) {
    return [];
  }

  const out: DnsAnswer[] = [];
  for (const key of SBO3L_TEXT_KEYS) {
    let value: string;
    try {
      value = (await client.readContract({
        address: resolverAddress,
        abi: RESOLVER_ABI,
        functionName: 'text',
        args: [node, key],
      })) as string;
    } catch {
      // Resolver doesn't expose this key — skip silently.
      continue;
    }
    if (value && value.length > 0) {
      out.push({
        name: ensName,
        type: 'TXT',
        ttl: DEFAULT_TTL_SECONDS,
        value: `${key}=${value}`,
      });
    }
  }
  return out;
}

/**
 * Resolve A/AAAA for an SBO3L name. Reads the agent's
 * `sbo3l:endpoint` text record, extracts the host portion of the
 * URL, and forwards a DNS query for that host to the upstream
 * resolver. Surfaces both A and AAAA answers as appropriate.
 */
export async function resolveAddress(
  name: string,
  type: 'A' | 'AAAA',
  opts: ResolveOptions
): Promise<DnsAnswer[]> {
  const ensName = normaliseEnsName(name);
  if (!isEnsName(ensName)) {
    throw new Error(`resolveAddress: ${name} is not an ENS name`);
  }

  const client = createPublicClient({
    chain: opts.network === 'mainnet' ? mainnet : sepolia,
    transport: http(opts.rpcUrl),
  });

  const node = namehash(ensName);
  const resolverAddress = (await client.readContract({
    address: ENS_REGISTRY,
    abi: REGISTRY_ABI,
    functionName: 'resolver',
    args: [node],
  })) as `0x${string}`;

  if (
    resolverAddress.toLowerCase() ===
    '0x0000000000000000000000000000000000000000'
  ) {
    return [];
  }

  let endpoint: string;
  try {
    endpoint = (await client.readContract({
      address: resolverAddress,
      abi: RESOLVER_ABI,
      functionName: 'text',
      args: [node, 'sbo3l:endpoint'],
    })) as string;
  } catch {
    return [];
  }

  if (!endpoint) return [];

  let host: string;
  try {
    host = new URL(endpoint).hostname;
  } catch {
    return [];
  }

  // Forward to upstream DoH. RFC 8484 DoH wire format is binary; we
  // delegate the full encode/decode to the upstream's JSON API
  // (Cloudflare 1.1.1.1 supports `application/dns-json`) for
  // simplicity in the scaffold.
  const url = new URL(opts.upstreamDohUrl);
  url.searchParams.set('name', host);
  url.searchParams.set('type', type);
  const resp = await fetch(url, {
    headers: { Accept: 'application/dns-json' },
  });
  if (!resp.ok) return [];

  const body = (await resp.json()) as {
    Answer?: Array<{ name: string; type: number; TTL: number; data: string }>;
  };
  const wantedType = type === 'A' ? 1 : 28;
  return (body.Answer ?? [])
    .filter((a) => a.type === wantedType)
    .map((a) => ({
      name: ensName,
      type,
      ttl: a.TTL,
      value: a.data,
    }));
}
