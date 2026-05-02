/**
 * ENS-backed DNS resolver — production logic + injectable deps for tests.
 *
 * Two abstraction seams keep this module testable without a live
 * Ethereum RPC or a live upstream DoH server:
 *
 *  1. `EnsTextReader` — reads ENS text records / discovers resolver
 *     addresses for a given name. Default impl uses viem against a
 *     public RPC; tests inject a fixture-backed reader.
 *
 *  2. `UpstreamDohClient` — issues a DoH-JSON query for a host
 *     against the upstream resolver (Cloudflare 1.1.1.1 by default).
 *     Default impl uses `fetch`; tests inject canned responses.
 *
 * The two `resolveTxt` / `resolveAddress` entry points accept
 * optional `deps` to override these for tests; production callers
 * pass `ResolveOptions` only and get the live deps automatically.
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

export const SBO3L_TEXT_KEYS = [
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
const ZERO_ADDRESS = '0x0000000000000000000000000000000000000000';
const DEFAULT_TTL_SECONDS = 300;

export interface ResolveOptions {
  rpcUrl: string;
  network: 'mainnet' | 'sepolia';
  upstreamDohUrl: string;
}

/** Reads ENS data for a single name. Tests inject fixtures here. */
export interface EnsTextReader {
  /** Returns the resolver address registered for `name`, or null
   *  if the name has no resolver / doesn't exist. */
  getResolver(name: string): Promise<string | null>;
  /** Returns the text record value, or empty string if absent. */
  readText(name: string, key: string): Promise<string>;
}

/** Issues a DoH-JSON query against the upstream. Tests inject
 *  canned responses here. */
export interface UpstreamDohClient {
  query(host: string, type: 'A' | 'AAAA'): Promise<DnsAnswer[]>;
}

export interface ResolveDeps {
  ens: EnsTextReader;
  upstream: UpstreamDohClient;
}

export function isEnsName(name: string): boolean {
  const lower = name.toLowerCase();
  return lower.endsWith('.eth') || lower.endsWith('.eth.');
}

export function normaliseEnsName(name: string): string {
  let n = name.toLowerCase();
  if (n.endsWith('.')) n = n.slice(0, -1);
  return n;
}

/**
 * Resolve TXT records for an SBO3L name. Returns one DnsAnswer per
 * non-empty `sbo3l:*` text record, formatted as `key=value`.
 */
export async function resolveTxt(
  name: string,
  opts: ResolveOptions,
  deps?: Partial<ResolveDeps>,
): Promise<DnsAnswer[]> {
  const ensName = normaliseEnsName(name);
  if (!isEnsName(ensName)) {
    throw new Error(`resolveTxt: ${name} is not an ENS name`);
  }
  const ens = deps?.ens ?? buildLiveEnsReader(opts);

  const resolver = await ens.getResolver(ensName);
  if (!resolver) return [];

  const out: DnsAnswer[] = [];
  for (const key of SBO3L_TEXT_KEYS) {
    let value = '';
    try {
      value = await ens.readText(ensName, key);
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
 * `sbo3l:endpoint` text record, extracts the host, and forwards a
 * DNS query for that host to the upstream resolver.
 */
export async function resolveAddress(
  name: string,
  type: 'A' | 'AAAA',
  opts: ResolveOptions,
  deps?: Partial<ResolveDeps>,
): Promise<DnsAnswer[]> {
  const ensName = normaliseEnsName(name);
  if (!isEnsName(ensName)) {
    throw new Error(`resolveAddress: ${name} is not an ENS name`);
  }
  const ens = deps?.ens ?? buildLiveEnsReader(opts);
  const upstream = deps?.upstream ?? buildLiveUpstreamClient(opts);

  const resolver = await ens.getResolver(ensName);
  if (!resolver) return [];

  let endpoint = '';
  try {
    endpoint = await ens.readText(ensName, 'sbo3l:endpoint');
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

  const upstreamAnswers = await upstream.query(host, type);
  return upstreamAnswers.map((a) => ({
    name: ensName,
    type,
    ttl: a.ttl,
    value: a.value,
  }));
}

// ============================================================
// Live deps — viem + fetch. Used by production routes; tests
// inject fixtures via the `deps` parameter on `resolveTxt` /
// `resolveAddress`.
// ============================================================

function buildLiveEnsReader(opts: ResolveOptions): EnsTextReader {
  const client = createPublicClient({
    chain: opts.network === 'mainnet' ? mainnet : sepolia,
    transport: http(opts.rpcUrl),
  });

  return {
    async getResolver(name: string): Promise<string | null> {
      const node = namehash(name);
      const addr = (await client.readContract({
        address: ENS_REGISTRY,
        abi: REGISTRY_ABI,
        functionName: 'resolver',
        args: [node],
      })) as `0x${string}`;
      if (addr.toLowerCase() === ZERO_ADDRESS) return null;
      return addr;
    },
    async readText(name: string, key: string): Promise<string> {
      const node = namehash(name);
      const resolver = (await client.readContract({
        address: ENS_REGISTRY,
        abi: REGISTRY_ABI,
        functionName: 'resolver',
        args: [node],
      })) as `0x${string}`;
      if (resolver.toLowerCase() === ZERO_ADDRESS) return '';
      return (await client.readContract({
        address: resolver,
        abi: RESOLVER_ABI,
        functionName: 'text',
        args: [node, key],
      })) as string;
    },
  };
}

function buildLiveUpstreamClient(opts: ResolveOptions): UpstreamDohClient {
  return {
    async query(host: string, type: 'A' | 'AAAA'): Promise<DnsAnswer[]> {
      const url = new URL(opts.upstreamDohUrl);
      url.searchParams.set('name', host);
      url.searchParams.set('type', type);
      const resp = await fetch(url, {
        headers: { Accept: 'application/dns-json' },
      });
      if (!resp.ok) return [];
      const body = (await resp.json()) as {
        Answer?: Array<{
          name: string;
          type: number;
          TTL: number;
          data: string;
        }>;
      };
      const wantedType = type === 'A' ? 1 : 28;
      return (body.Answer ?? [])
        .filter((a) => a.type === wantedType)
        .map((a) => ({
          name: host,
          type,
          ttl: a.TTL,
          value: a.data,
        }));
    },
  };
}
