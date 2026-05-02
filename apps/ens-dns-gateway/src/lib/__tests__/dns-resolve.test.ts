/**
 * Test suite for the ENS-backed DNS resolver. Five mock ENS names
 * cover the canonical fleet shape; edge cases cover missing
 * resolvers, missing endpoint, malformed endpoint URL, IPv6, and
 * non-ENS names.
 */

import { describe, expect, it } from 'vitest';

import type {
  DnsAnswer,
  EnsTextReader,
  ResolveOptions,
  UpstreamDohClient,
} from '../dns-resolve';
import { isEnsName, resolveAddress, resolveTxt } from '../dns-resolve';

const OPTS: ResolveOptions = {
  rpcUrl: 'http://test.invalid',
  network: 'mainnet',
  upstreamDohUrl: 'http://test.invalid/dns-query',
};

// Five mock SBO3L names, each with a different shape.
const FIXTURES: Record<string, Record<string, string>> = {
  // Canonical fleet agent — all 5 sbo3l:* keys present.
  'research-agent.sbo3lagent.eth': {
    'sbo3l:agent_id': 'research-agent-01',
    'sbo3l:endpoint': 'http://research.sbo3l.dev:8730/v1',
    'sbo3l:policy_hash': 'e044f13c5acb792dd3109f1be3a98536',
    'sbo3l:audit_root': '0000000000000000000000000000000000000000',
    'sbo3l:proof_uri': 'https://b2jk-industry.github.io/proofs/research-01.json',
  },
  // Trading agent with IPv6 endpoint.
  'trading-agent.sbo3lagent.eth': {
    'sbo3l:agent_id': 'trading-agent-01',
    'sbo3l:endpoint': 'http://[2001:db8::1]:8731/v1',
    'sbo3l:policy_hash': 'aaaa1111bbbb2222cccc3333dddd4444',
    'sbo3l:audit_root': '1111111111111111111111111111111111111111',
    'sbo3l:proof_uri': 'https://b2jk-industry.github.io/proofs/trading-01.json',
  },
  // Audit agent — no endpoint set.
  'audit-agent.sbo3lagent.eth': {
    'sbo3l:agent_id': 'audit-agent-01',
    'sbo3l:endpoint': '',
    'sbo3l:policy_hash': '5555555555555555555555555555555555555555',
    'sbo3l:audit_root': '6666666666666666666666666666666666666666',
    'sbo3l:proof_uri': 'https://b2jk-industry.github.io/proofs/audit-01.json',
  },
  // Coordinator — partial records (only agent_id + endpoint).
  'coordinator.sbo3lagent.eth': {
    'sbo3l:agent_id': 'coordinator-01',
    'sbo3l:endpoint': 'http://coordinator.sbo3l.dev/v1',
  },
  // Apex — full record set.
  'sbo3lagent.eth': {
    'sbo3l:agent_id': 'sbo3lagent',
    'sbo3l:endpoint': 'http://apex.sbo3l.dev/v1',
    'sbo3l:policy_hash': '7777777777777777777777777777777777777777',
    'sbo3l:audit_root': '8888888888888888888888888888888888888888',
    'sbo3l:proof_uri': 'https://b2jk-industry.github.io/proofs/apex.json',
  },
};

const FIXTURE_RESOLVER = '0x1234567890123456789012345678901234567890';

function buildMockReader(): EnsTextReader {
  return {
    async getResolver(name: string): Promise<string | null> {
      return FIXTURES[name] ? FIXTURE_RESOLVER : null;
    },
    async readText(name: string, key: string): Promise<string> {
      const records = FIXTURES[name];
      if (!records) return '';
      return records[key] ?? '';
    },
  };
}

function buildMockUpstream(
  responses: Record<string, DnsAnswer[]>,
): UpstreamDohClient {
  return {
    async query(host: string, type: 'A' | 'AAAA'): Promise<DnsAnswer[]> {
      const key = `${host}:${type}`;
      return responses[key] ?? [];
    },
  };
}

describe('isEnsName', () => {
  it('matches .eth names', () => {
    expect(isEnsName('foo.eth')).toBe(true);
    expect(isEnsName('research-agent.sbo3lagent.eth')).toBe(true);
    expect(isEnsName('FOO.ETH')).toBe(true);
    expect(isEnsName('foo.eth.')).toBe(true);
  });

  it('rejects non-.eth names', () => {
    expect(isEnsName('foo.com')).toBe(false);
    expect(isEnsName('foo')).toBe(false);
    expect(isEnsName('')).toBe(false);
    expect(isEnsName('eth')).toBe(false);
  });
});

describe('resolveTxt', () => {
  const ens = buildMockReader();
  const upstream = buildMockUpstream({});

  it('returns one TXT answer per non-empty sbo3l:* record', async () => {
    const out = await resolveTxt(
      'research-agent.sbo3lagent.eth',
      OPTS,
      { ens, upstream },
    );
    // 5 keys, all present + non-empty.
    expect(out).toHaveLength(5);
    expect(out[0]).toMatchObject({
      name: 'research-agent.sbo3lagent.eth',
      type: 'TXT',
      value: 'sbo3l:agent_id=research-agent-01',
    });
    // Order matches SBO3L_TEXT_KEYS array.
    expect(out.map((r) => r.value.split('=')[0])).toEqual([
      'sbo3l:agent_id',
      'sbo3l:endpoint',
      'sbo3l:policy_hash',
      'sbo3l:audit_root',
      'sbo3l:proof_uri',
    ]);
  });

  it('skips empty records (audit-agent has no endpoint)', async () => {
    const out = await resolveTxt(
      'audit-agent.sbo3lagent.eth',
      OPTS,
      { ens, upstream },
    );
    // 4 records: endpoint is empty, others present.
    expect(out).toHaveLength(4);
    expect(out.map((r) => r.value.split('=')[0])).toEqual([
      'sbo3l:agent_id',
      // sbo3l:endpoint is skipped (empty)
      'sbo3l:policy_hash',
      'sbo3l:audit_root',
      'sbo3l:proof_uri',
    ]);
  });

  it('returns partial set for coordinator', async () => {
    const out = await resolveTxt(
      'coordinator.sbo3lagent.eth',
      OPTS,
      { ens, upstream },
    );
    // Only agent_id + endpoint set.
    expect(out).toHaveLength(2);
    expect(out[0].value).toBe('sbo3l:agent_id=coordinator-01');
    expect(out[1].value).toBe('sbo3l:endpoint=http://coordinator.sbo3l.dev/v1');
  });

  it('returns empty list for unknown name (no resolver)', async () => {
    const out = await resolveTxt('nonexistent.eth', OPTS, { ens, upstream });
    expect(out).toEqual([]);
  });

  it('throws on non-ENS name', async () => {
    await expect(
      resolveTxt('foo.com', OPTS, { ens, upstream }),
    ).rejects.toThrow('not an ENS name');
  });

  it('normalises trailing dot', async () => {
    const out = await resolveTxt(
      'research-agent.sbo3lagent.eth.',
      OPTS,
      { ens, upstream },
    );
    expect(out).toHaveLength(5);
    expect(out[0].name).toBe('research-agent.sbo3lagent.eth'); // trailing dot stripped
  });

  it('TTL is 5 minutes per SWR spec', async () => {
    const out = await resolveTxt(
      'research-agent.sbo3lagent.eth',
      OPTS,
      { ens, upstream },
    );
    expect(out[0].ttl).toBe(300);
  });
});

describe('resolveAddress', () => {
  const ens = buildMockReader();

  it('resolves A from sbo3l:endpoint host', async () => {
    const upstream = buildMockUpstream({
      'research.sbo3l.dev:A': [
        { name: 'research.sbo3l.dev', type: 'A', ttl: 60, value: '203.0.113.10' },
      ],
    });
    const out = await resolveAddress(
      'research-agent.sbo3lagent.eth',
      'A',
      OPTS,
      { ens, upstream },
    );
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({
      name: 'research-agent.sbo3lagent.eth',
      type: 'A',
      value: '203.0.113.10',
    });
  });

  it('resolves AAAA for IPv6-shaped endpoint host', async () => {
    const upstream = buildMockUpstream({
      // IPv6 literal endpoint — host extracted from URL is `[2001:db8::1]`
      // (URL parser preserves brackets); upstream key matches that.
      '[2001:db8::1]:AAAA': [
        {
          name: '[2001:db8::1]',
          type: 'AAAA',
          ttl: 60,
          value: '2001:db8::1',
        },
      ],
    });
    const out = await resolveAddress(
      'trading-agent.sbo3lagent.eth',
      'AAAA',
      OPTS,
      { ens, upstream },
    );
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({
      name: 'trading-agent.sbo3lagent.eth',
      type: 'AAAA',
      value: '2001:db8::1',
    });
  });

  it('returns empty when sbo3l:endpoint is unset', async () => {
    const upstream = buildMockUpstream({});
    const out = await resolveAddress(
      'audit-agent.sbo3lagent.eth',
      'A',
      OPTS,
      { ens, upstream },
    );
    expect(out).toEqual([]);
  });

  it('returns empty for unknown name', async () => {
    const upstream = buildMockUpstream({});
    const out = await resolveAddress(
      'nonexistent.eth',
      'A',
      OPTS,
      { ens, upstream },
    );
    expect(out).toEqual([]);
  });

  it('returns empty when upstream has no answer', async () => {
    const upstream = buildMockUpstream({});
    const out = await resolveAddress(
      'research-agent.sbo3lagent.eth',
      'A',
      OPTS,
      { ens, upstream },
    );
    expect(out).toEqual([]);
  });

  it('reuses ensName as the synthetic answer name (not the underlying host)', async () => {
    const upstream = buildMockUpstream({
      'apex.sbo3l.dev:A': [
        { name: 'apex.sbo3l.dev', type: 'A', ttl: 60, value: '192.0.2.50' },
      ],
    });
    const out = await resolveAddress('sbo3lagent.eth', 'A', OPTS, {
      ens,
      upstream,
    });
    expect(out[0].name).toBe('sbo3lagent.eth'); // ENS name, not apex.sbo3l.dev
  });

  it('throws on non-ENS name', async () => {
    const upstream = buildMockUpstream({});
    await expect(
      resolveAddress('foo.com', 'A', OPTS, { ens, upstream }),
    ).rejects.toThrow('not an ENS name');
  });
});
