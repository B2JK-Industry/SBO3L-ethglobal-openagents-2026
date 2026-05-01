# SBO3L × ENS — submission one-pager (v1.0.1)

> **ENS becomes the agent trust DNS.**
> **Audience:** ENS team + ETHGlobal judges (Most Creative, AI Agents).
> Engineering deep-dive at [`docs/spec/`](../../spec/) (ENS records spec).

## Try it now (60 seconds)

```bash
cargo install sbo3l-cli --version 1.0.1

# Resolve the mainnet apex — 7 sbo3l:* records returned
sbo3l passport resolve sbo3lagent.eth

# Resolve a Sepolia subname (after T-3-3 fleet lands)
SBO3L_ENS_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com \
sbo3l passport resolve research-agent.sbo3lagent.eth
```

## What we put on chain

| Record | Meaning |
|---|---|
| `sbo3l:agent_id` | Stable agent identifier |
| `sbo3l:endpoint` | HTTPS endpoint for the SBO3L daemon serving this agent |
| `sbo3l:policy_hash` | Canonical hash of the agent's active policy snapshot |
| `sbo3l:audit_root` | Latest audit-chain head (rolled forward via checkpoints) |
| `sbo3l:proof_uri` | Stable URL where the latest Passport capsule for this agent lives |
| `sbo3l:capability` | Comma-separated capability tags (`x402-purchase`, `uniswap-swap`, `delegation-target`) |
| `sbo3l:reputation` | `<score>/100` computed from the audit chain (4-criteria scoring) |

Reading these seven records gives you a complete trust profile for an agent — *without trusting any single party*.

## Cross-agent verification

Agent A delegating to agent B signs an Ed25519 attestation pinning B's expected `policy_hash` and `expires_at`. The attestation flows through B's APRP to B's SBO3L; the daemon verifies the chain (A's pubkey → B's published policy → B's actual decision) before allowing the delegated action.

Tampered → `cross_agent.attestation_invalid`. Expired → `cross_agent.attestation_expired`. Both rejection paths are tested in the demo gate.

## Trust-DNS visualization

[`https://app.sbo3l.dev/trust-dns`](https://app.sbo3l.dev/trust-dns) — D3 force-directed graph of the Sepolia agent fleet discovering each other in real time. WebSocket-driven; allow/deny pulses on each node; signed attestation animates each edge.

This is the demo-video centerpiece for ENS Most Creative.

## ENSIP-25 CCIP-Read gateway

[`https://ccip.sbo3l.dev`](https://ccip.sbo3l.dev) serves dynamic `sbo3l:*` records (computed reputation, current audit-root, fresh capability set) without burning gas on every update. Uptime probe at `.github/workflows/ccip-gateway-uptime.yml`.

## ERC-8004 Identity Registry

T-4-2: SBO3L agents register their `agent_id` + canonical pubkey in the ERC-8004 Identity Registry on mainnet, anchoring the ENS subname to a verifiable global identity.

## Why we're going for "Most Creative"

We didn't add ENS as a feature; we used ENS as the *backbone* of the trust model. Every agent identity lookup, every cross-agent attestation, every reputation update flows through ENS. The hosted trust-DNS visualization makes that backbone visible — and verifiable in real time.

## Crates / packages

| Surface | Install | Verify |
|---|---|---|
| Identity crate | `cargo add sbo3l-identity@1.0.1` | https://crates.io/crates/sbo3l-identity |
| CLI | `cargo install sbo3l-cli --version 1.0.1` | `sbo3l --version` |
| TS SDK | `npm install @sbo3l/sdk` | https://www.npmjs.com/package/@sbo3l/sdk |
