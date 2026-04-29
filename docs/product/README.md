# SBO3L Passport Product Plan

This folder is the product-planning layer for the next SBO3L phase.
It does not replace the implemented submission surface. It explains how
the already-built SBO3L primitives should compose into one coherent
hackathon-winning product:

> SBO3L Passport: proof-carrying execution for AI agents.

The current implementation already has the hard substrate: APRP,
policy decisions, signed receipts, persistent replay protection,
active policy lifecycle, mock KMS, doctor, audit checkpoints, audit
bundles, trust badge, operator console, and sponsor-facing adapters.
The next phase is packaging those pieces into a product that prize
judges can understand, use, and map directly to their own goals.

## Documents

| Document | Purpose |
|---|---|
| [`SBO3L_PASSPORT_SOURCE_OF_TRUTH.md`](SBO3L_PASSPORT_SOURCE_OF_TRUTH.md) | Single source of truth for the product vision, future production state, data model, user journeys, and non-goals. |
| [`SBO3L_PASSPORT_BACKLOG.md`](SBO3L_PASSPORT_BACKLOG.md) | Detailed two-developer execution backlog. PR sequence, owners, acceptance criteria, verification, and merge gates. |
| [`REWARD_STRATEGY.md`](REWARD_STRATEGY.md) | Prize-by-prize strategy for KeeperHub, ENS, Uniswap, Builder Feedback, and optional 0G/Gensyn reach. |
| [`TWO_DEVELOPER_EXECUTION_PROTOCOL.md`](TWO_DEVELOPER_EXECUTION_PROTOCOL.md) | Operating protocol for Developer A + Developer B working continuously without breaking the existing product. |

## Product Thesis

Most teams are building agents. SBO3L should be the infrastructure an
agent must pass through before it can move value:

1. ENS tells the world which agent this is and which policy/audit roots
   define its public mandate.
2. MCP lets any agent, IDE, or orchestration framework ask SBO3L for a
   decision.
3. SBO3L produces a signed policy receipt and a tamper-evident audit
   trail.
4. KeeperHub executes only after SBO3L has allowed the action.
5. Uniswap swaps are quote-checked, budget-checked, and recipient-checked
   before execution.
6. The whole thing is exported as a portable proof capsule a judge can
   inspect offline.

Short version:

> KeeperHub executes. ENS discovers. Uniswap settles. SBO3L proves the
> action was authorized.

## Discipline

This plan is additive. It must not weaken the current submission-grade
surface:

- The existing 13-gate demo remains green.
- The production-shaped mock runner remains deterministic by default.
- Offline static trust badge and operator console remain file-safe.
- Mocks stay labelled as mocks until a real network call exists.
- The APRP wire format, audit bundle, and receipt semantics stay stable
  unless a schema bump is explicit and tested in lockstep.
