// Mock per-tenant policy YAML. The real source of truth is the daemon
// (P3.5: GET /v1/tenants/<slug>/policy returns signed YAML + version).
// This stub exists so the Monaco editor at /t/[slug]/admin/policy/edit
// has realistic content to render and round-trip during demos.
//
// Schema: sbo3l.policy.v1 (see crates/sbo3l-policy/src/schema.rs).
// _Tenant policies inherit from the tenant's root mandate_ — only the
// override surface is shown here, not the full effective set.

export interface TenantPolicy {
  yaml: string;
  version: number;
  updated_at: string;
  signed_by: string;
}

const ACME_POLICY = `# acme.sbo3lagent.eth — policy override v3
# Inherits: sbo3l.root@1
schema: sbo3l.policy.v1
tenant: acme
version: 3
updated_at: 2026-04-29T15:42:00Z

intents:
  - name: erc20.transfer
    where:
      to:    { allowlist: [acme-treasury, acme-payroll] }
      token: { allowlist: [USDC, USDT, DAI] }
      amount: { lte_per_24h: 50000 }
    require:
      - human_2fa: true
        when: { amount: { gt: 10000 } }

  - name: uniswap.swap
    where:
      tokenIn:  { allowlist: [USDC, WETH] }
      tokenOut: { allowlist: [USDC, WETH] }
      slippage_bps: { lte: 50 }

  - name: cron.payroll
    where:
      schedule: "0 9 1,15 * *"   # 1st + 15th, 09:00 UTC
    require:
      - tier: pro
`;

const CONTOSO_POLICY = `# contoso.sbo3lagent.eth — policy override v1
# Inherits: sbo3l.root@1
# (Free tier — most overrides locked; upgrade for full surface.)
schema: sbo3l.policy.v1
tenant: contoso
version: 1
updated_at: 2026-04-22T14:30:00Z

intents:
  - name: erc20.transfer
    where:
      amount: { lte_per_24h: 1000 }   # free-tier cap
    require:
      - human_2fa: true
`;

const FABRIKAM_POLICY = `# fabrikam.sbo3lagent.eth — policy override v7
# Inherits: sbo3l.root@1, fabrikam.compliance@2
schema: sbo3l.policy.v1
tenant: fabrikam
version: 7
updated_at: 2026-05-01T11:18:00Z

intents:
  - name: erc20.transfer
    where:
      to:    { allowlist_resolver: ens, suffix: ".fabrikam.eth" }
      token: { allowlist: [USDC, EURC] }
      amount: { lte_per_24h: 250000 }
    require:
      - quorum:    2_of_3
        signers:   [treasury-ops, treasury-cfo, treasury-cto]
      - audit_log: enterprise

  - name: opencompute.train
    where:
      model: { allowlist: [llama-3-8b, mistral-7b] }
      duration_minutes: { lte: 240 }
    require:
      - tier: enterprise
`;

const POLICIES: Record<string, TenantPolicy> = {
  acme: {
    yaml: ACME_POLICY,
    version: 3,
    updated_at: "2026-04-29T15:42:00Z",
    signed_by: "acme-admin@acme.sbo3lagent.eth",
  },
  contoso: {
    yaml: CONTOSO_POLICY,
    version: 1,
    updated_at: "2026-04-22T14:30:00Z",
    signed_by: "contoso-admin@contoso.sbo3lagent.eth",
  },
  fabrikam: {
    yaml: FABRIKAM_POLICY,
    version: 7,
    updated_at: "2026-05-01T11:18:00Z",
    signed_by: "fabrikam-cto@fabrikam.sbo3lagent.eth",
  },
};

export function policyForTenant(slug: string): TenantPolicy | undefined {
  return POLICIES[slug];
}
