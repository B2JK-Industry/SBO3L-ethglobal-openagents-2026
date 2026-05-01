# SBO3L for ENS — judges' pitch

**For:** Dhaiwat (DM contact) and Simon `ses.eth` (technical reviewer).
**Length:** 300 words.

---

ENS is usually treated as agent **identity** — "here is the name."
SBO3L treats ENS as agent **commitment**: the resolver pointer is
load-bearing for runtime authentication, policy enforcement, and
audit-chain integrity.

`sbo3lagent.eth` (mainnet, owned) carries seven `sbo3l:*` text
records: `agent_id`, `endpoint`, `pubkey_ed25519`, `policy_url`,
`policy_hash`, `audit_root`, `proof_uri`, `capabilities`. `policy_hash`
is the JCS+SHA-256 of the agent's active policy snapshot; any judge
holding the ENS record and the daemon's `/v1/policy` endpoint can
re-hash and prove non-drift independently. `audit_root` pins the
cumulative digest of the agent's audit chain — silent retroactive
tampering with history shifts the digest and breaks every previously-
issued trust receipt.

The cross-agent verification protocol
(`crates/sbo3l-identity/src/cross_agent.rs`) is the load-bearing
claim: **two SBO3L agents need ZERO out-of-band setup to authenticate
each other.** Agent A signs a challenge containing its
`audit_chain_head + nonce + ts`; Agent B resolves A's
`sbo3l:pubkey_ed25519` via `getEnsText`, verifies the Ed25519
signature, emits a JCS-canonical `CrossAgentTrust` receipt. No CA, no
session, no enrolment server.

Two manifests ship live at submission: 5 named-role agents and a
60-agent constellation, every keypair deterministically re-derived
from a public seed-doc via SHA-256. Reputation, capabilities, and
audit head are served via an ENSIP-25 / EIP-3668 CCIP-Read gateway
(`apps/ccip-gateway/`, `OffchainResolver.sol`) — viem's
`getEnsText` round-trips with no SBO3L-specific client code.

We're targeting both bounty tracks: AI Agents framing (every agent
has a verifiable ENS identity) for the technical track, Most
Creative for the policy-commitment-as-text-record framing. Three
minutes of wallet ops light up the full demo. Happy to walk you
through the cross-agent protocol on Telegram (ENSxAI / ENS Devs).

— Dev 4, SBO3L
