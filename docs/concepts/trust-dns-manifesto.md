---
title: "Trust DNS Manifesto — naming as authentication for autonomous agents"
audience: "Engineers, ENS standards reviewers (ENSIP track), ERC-8004 implementers, agent-platform architects"
outcome: "By the end you can state, in one sentence, why an autonomous-agent ecosystem needs ENS-as-identity (not just ENS-as-naming) and how a seven-record opinionated commitment set generalises into an interoperable agent-identity convention."
length: "~5000 words"
status: "Phase 2 closeout — Trust DNS amplifier (ENS-MC-A2)"
keywords: "ENS, ENSIP, agent identity, autonomous agents, CCIP-Read, ERC-8004, EIP-3668, OffchainResolver, RFC2119"
related: "docs/ENSIP-N-DRAFT.md (the conventions this manifesto promotes)"
---

# Trust DNS Manifesto — naming as authentication for autonomous agents

> **Audience:** engineers and standards-track readers (ENS judges, ERC-8004 reviewers, ENSIP-aware integrators) who want the load-bearing claim before the implementation.
>
> **Outcome in 90 seconds:** by the end you can state, in one sentence, why an autonomous-agent ecosystem needs ENS-as-identity and not just ENS-as-naming. The TL;DR: *DNS resolves names to machines; SBO3L resolves names to **trust commitments** — and that's a primitive no existing naming system gives you for free.*
>
> **Normative language:** This manifesto uses MUST / SHOULD / MAY / RECOMMENDED in the sense of [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119) for the convention it proposes; the prose around the convention is descriptive.

## 1. Hero claim — the substitution that changes everything

Two autonomous AI agents are about to coordinate on a real action: A delegates a swap to B, A pays B for a result, B attests on A's behalf. The first question every protocol step asks is the same: *who is the other side, and what can they actually do?*

In a human-driven web2 system the answer is "you logged in." In a centralised ML platform the answer is "you and the other agent are both inside our trust boundary." In a multi-tenant agent ecosystem the answer is *nothing in the box yet*. There is no agent CA. There is no enrolment server. There is no shared session token that two SBO3L instances bootstrap before they trust each other. We had to pick an answer.

The answer we picked is ENS — but not in the way most projects mean when they say *we're using ENS*. Most uses of ENS reduce to **naming**: a friendlier label for a wallet address. What an autonomous-agent ecosystem actually needs is **authentication**: a name that lets a remote verifier reconstruct everything they need to know about the named entity, with no shared secrets and no trusted intermediary. That is a stronger claim than "naming," and ENS — precisely because of how it was already built — turns out to be the cleanest substrate to make it on.

This manifesto explains the substitution and walks through its consequences. None of those consequences are subtle once you see the substitution; the substitution itself is the load-bearing claim.

## 2. Why ENS, not a custom registry

The first instinct of every team building an agent-identity layer is to ship a registry. A registry is easy: stand up a service, define a JSON schema, hand out names. SBO3L deliberately did not ship one. The reasoning is practical, not aesthetic.

**Censorship resistance.** A custom registry has an owner. The owner can be subpoenaed, deplatformed, acquired, or simply walk away. The set of agents whose identity is "trustable" then becomes a function of *that owner's continued cooperation* — a single point of policy. ENS, by contrast, is a smart-contract registry on a public chain. Resolution does not depend on any one party's continued willingness to host the data; the records exist on chain, and any RPC client can read them. For an ecosystem whose entire selling point is *removing trusted intermediaries from agent action*, anchoring the identity layer to one would be incoherent.

**Global namespace.** A custom registry creates a private namespace ("agents.acme.com/research-01") that another platform's clients have no reason to recognise. Two agents on different platforms cannot authenticate each other without first agreeing on which registry to consult. ENS gives us a namespace every web3 wallet, indexer, and explorer already understands. `research-01.sbo3lagent.eth` resolves the same way under viem, ethers.js, the ENS App, raw `cast text`, and a dozen other tools. The ecosystem effect is structural: the moment we publish records under that name, every existing ENS consumer can read them with zero new code.

**No infrastructure to run.** The custom-registry path comes with a tail of operational obligations: certificate management, DDoS mitigation, redundancy, schema evolution, audit logging, terms of service, GDPR data-subject access, abuse handling. ENS hands us all of that for free in exchange for a few text records and one resolver contract. The hosted-app surface SBO3L *does* run — the CCIP-Read gateway at `ccip.sbo3l.dev` — is intentionally minimal: a thin signer that computes dynamic values from the audit chain. If it falls over, static records continue to resolve directly from mainnet and the read primitive still works. A custom-registry outage is total; an ENS-CCIP outage degrades gracefully to "static-only," and even that degradation is documented at the protocol level (EIP-3668 §6.2 on offchain failures).

**Existing client tooling.** Years of work went into making ENS easy to read. Consumers don't need to write our SDK; they need to know one function, `getEnsText(name, key)`, that already ships in every Ethereum library. A LangChain integration that wants to authenticate the agent it's about to call doesn't reach for `@sbo3l/sdk`; it reaches for `viem`. The convention rides on top of an existing 7+ year deployment of resolution infrastructure.

**Cryptographic anchoring.** The mapping `name → records` is enforced by smart contracts on a public chain. Any tampering with what an agent's name resolves to is detectable, not policy-trusted. A custom registry can promise this, but the promise is only as strong as the registry operator. ENS records have on-chain provenance; we read them and we can prove what they were at block N for the auditor's later inspection.

The choice was therefore not *should we use ENS or a custom registry* — it was *should we accept that an opinionated commitment set on top of ENS is the lower-cost, higher-leverage path*. We accepted. The consequences of that choice define the rest of this manifesto.

## 3. The trust profile in seven records

ENS gives us `text(node, key)`. SBO3L proposes seven keys. Each key answers a question a remote verifier needs an answer to. Each key has a normative format. Each key has a normative interpretation. Together they form a self-contained agent-identity profile.

The seven keys are also the body of [`docs/ENSIP-N-DRAFT.md`](../ENSIP-N-DRAFT.md), the standardisation track companion to this manifesto. Treating them as a profile rather than a free-form schema is what lets the convention generalise across platforms.

### 3.1 `agent_id` — stable identifier

The agent's identity surface MUST have a stable identifier that survives resolver rotation and record rotation. `agent_id` is that identifier: a UTF-8 string, ≤ 64 characters, conventionally a hyphenated lowercase slug. It is what a *policy* binds to. The receipt the daemon signs after every decision binds to `agent_id`, not to the ENS name itself; this matters because the ENS name's resolver pointer can change (key rotation), the apex name can be transferred between owners, and the underlying records can be re-keyed — but the `agent_id` the policy was minted under does not change.

A consumer reads `agent_id` first. If the value differs from what the consumer expects, no further check matters; identity is pseudonymous, and a different `agent_id` is a different agent. Implementations MUST NOT match agents by ENS name; they MUST match by `agent_id`.

### 3.2 `endpoint` — daemon URL

The agent's `endpoint` MUST be an absolute URL parseable per RFC 3986. The schemes `http` and `https` are REQUIRED to be supported by consumers; other schemes are RESERVED for future variants (e.g. an `onion://` deployment for tor-routed agents). The endpoint is the URL where the daemon's `/v1/payment-requests` is served — the action verb of the trust layer.

`endpoint` is *liveness-bearing*, not *trust-bearing*. A reachable URL doesn't authenticate anything by itself; the receipts the URL serves do. A consumer that wants to verify "is this agent currently up" hits `/v1/healthz` at the endpoint; a consumer that wants to verify "what does this agent commit to" reads the other six records.

### 3.3 `pubkey_ed25519` — receipt verifying key

The Ed25519 verifying key the agent's daemon signs receipts with. 64-character lowercase hex, no `0x` prefix. The on-chain commitment to this key is what makes capsule verification possible: a consumer with a Passport capsule can verify the Ed25519 signature against this pubkey *without contacting the agent's daemon at all*. The capsule plus this one record is a complete chain of cryptographic evidence.

Implementations MUST verify the format strictly. A 65-character value (the common mistake of leaving a leading `0x`) MUST be rejected, not silently coerced; silent coercion creates a class of subtle interop bug where two implementations disagree about which 32-byte key the record commits to. The strict-mode WASM verifier in `apps/marketing/public/wasm/sbo3l_core_bg.wasm` rejects on this.

### 3.4 `policy_hash` — commitment to the active policy

JCS-canonical SHA-256 of the policy snapshot the agent currently runs. 32 bytes, hex-encoded, `0x` prefix optional. The on-chain value MUST byte-match the hash the daemon publishes at runtime; drift is a tampering signal, not a routine update. A policy change ships as a coordinated triple — a fresh ENS update, a fresh signed snapshot pinned at the agent's `policy_url` (sibling record outside the seven), and a fresh receipt-signing rotation if the change is policy-wide. A consumer who finds the daemon claiming a different `policy_hash` than ENS records MUST treat the agent as untrusted until reconciled.

This is the record that does the load-bearing work for "what is this agent allowed to do." The policy is not embedded in ENS — that would be expensive, and the policy is structured (rules, allowlists, budgets, attestation requirements). What ENS commits to is the *hash* of the canonical serialisation. A consumer that wants to read the policy itself fetches it from `policy_url` and recomputes the hash, comparing against `policy_hash` byte-for-byte.

### 3.5 `audit_root` — anchor to the audit chain

Cumulative digest of the agent's audit chain at the most recent on-chain checkpoint. 32 bytes, hex-encoded. Updated via the daemon's anchor cron (every 6 hours by default, see `Sbo3lAuditAnchor.sol` on Sepolia). Anchors the off-chain hash chain to a public timeline so a verifier can prove "the agent's audit chain at time T contained at least N events ending at this hash."

`audit_root` is updated less frequently than the audit chain itself grows — that is intentional. Per-event on-chain writes would be expensive (~24K gas per anchor on Sepolia, ~$2-5/anchor on mainnet at hackathon-time gas prices); we batch and post Merkle roots instead. A consumer who wants per-event verification reads the off-chain audit log or the Passport capsule that wraps a single event; `audit_root` is the *I-am-not-lying-about-history* commitment, not the per-event commitment.

### 3.6 `capability` — sponsor-surface tags

Comma-separated capability tags identifying which sponsor surfaces this agent can act on. The current tag set is open-membership but conventional: `x402-purchase`, `uniswap-swap`, `keeperhub-cron`, `delegation-target`, `attestation-issuer`. A consumer that wants to delegate a swap to an agent reads `capability`, confirms `uniswap-swap` is present, and only then proceeds.

`capability` is advisory — the policy bound to `policy_hash` is the actual enforcement. A misadvertised capability (the agent claims `uniswap-swap` but the policy denies it) results in a deny capsule on every attempt, not a security failure. Consumers SHOULD use `capability` as a discovery aid, not as a substitute for trying the action and reading the receipt.

### 3.7 `reputation_score` — portable signal

A 0-100 portable reputation signal, computed dynamically from the audit chain via a four-criterion formula (decision count, policy adherence, attestation history, time-in-network) and served via CCIP-Read so every read reflects current state. The score itself is signed by the SBO3L gateway; a consumer who doesn't trust the gateway can recompute from the audit chain directly.

`reputation_score` is the most experimental of the seven — it lives in CCIP-Read territory because it changes faster than blocks, and the scoring function will inevitably evolve. The convention is therefore that consumers SHOULD read `reputation_score` as a hint, not as a binding. A score of 92 doesn't mean "trusted"; it means "the gateway computed 92 right now, and here is the signature." Trust still bottoms out in the policy and the audit chain.

### 3.8 What the seven records do *not* commit to

Two negative claims worth surfacing. First, the seven records do not commit to the agent's *behavioural* characteristics — what tone of voice it uses, how it phrases tool calls, what models it relies on. Those are application-layer concerns; the trust profile is the cryptographic surface only. Second, the seven records do not replace the receipt the daemon emits per decision. The records authorise; the receipt witnesses. A consumer that has read all seven records still needs to see a signed capsule for a specific action before believing that action took place.

## 4. Resolver rotation as identity key-rotation

A subtle property falls out of the architecture once the seven records are in place: **resolver rotation gives us key-rotation without a Certificate Authority.**

ENS names point to a resolver contract, and the resolver is what serves the records. The owner of the name can change which resolver the name points to at any time. If we deploy a new OffchainResolver — for instance, after a signer-key compromise, or after upgrading the gateway protocol — the agent's name updates to point at it, and every consumer reading the records gets the new resolver's responses on the very next query. There is no propagation delay beyond the block in which the resolver-pointer change confirms; there is no certificate-revocation-list to update; there is no authority to coordinate with.

In a CA-rooted PKI, key rotation is a cooperative dance: the certificate-authority issues a new cert, the holder publishes it, the relying parties refresh their truststores, and OCSP-stapling carries the revocation hint forward. Each step has a known failure mode (CRL bloat, stale OCSP responses, heterogenous truststore behaviour). Resolver rotation under ENS has *one* failure mode: a consumer that cached the old resolver and ignores the on-chain pointer update will get stale data — and that's a consumer bug, not a protocol property.

The same mechanism handles **incremental key rotation** of the receipt-signing key. The flow is:

1. Daemon generates a new Ed25519 keypair, prepared for activation at block N.
2. Owner updates the agent's `pubkey_ed25519` text record to the new public key (one ENS transaction).
3. Daemon, at block N, switches to signing receipts with the new key. Capsules emitted before block N are still verifiable against the *old* public key, which is preserved in capsule headers.
4. Consumers reading the agent's name from block N onward see the new `pubkey_ed25519`. They verify recent capsules with that key, and historical capsules against the historical record (queryable via ENS event logs or a third-party indexer such as ENSIdeas).

The whole flow takes one transaction. There is no manual revocation, no list to maintain, no third party to notify. The chain *is* the truststore.

This is precisely the property a custom registry can't easily replicate. A custom-registry rotation requires the registry operator to coordinate the version bump and convince every consumer to refresh. ENS-rooted rotation is fire-and-forget: the records change, every reader sees the change on next read, end of story.

A worked example, end to end, makes the property concrete. Suppose `research-01.sbo3lagent.eth` has been issuing capsules signed by Ed25519 key `K_old`. The operator detects a leak risk in `K_old` (a developer left a console open; the underlying private-key file is exposed for an unbounded window). The operator's response, in five steps:

1. **Generate** `K_new` on a fresh hardware token. Compute `pubkey_ed25519_new` (the 32-byte public key, lowercase hex).
2. **Pause** the daemon's signing pipeline. Any in-flight APRP returns HTTP 503 with `Retry-After: 60`.
3. **Update ENS** — `setText('pubkey_ed25519', pubkey_ed25519_new)` against the agent's resolver. One transaction, one block. All seven other records remain unchanged. The `agent_id` in particular does not change, so policy bindings and historical capsules are unaffected.
4. **Resume signing** with `K_new`. Capsules from this point forward are signed by the new key.
5. **Document the rotation** — append an audit-chain entry of kind `key.rotated` carrying `(old_pubkey, new_pubkey, rotation_block, reason)`. The next on-chain `audit_root` checkpoint commits to a chain that records the rotation as a first-class event.

A consumer reading capsules issued before the rotation sees `pubkey_ed25519: K_old` in the capsule header and verifies against that. A consumer reading capsules issued after sees `K_new`. The same consumer reading the agent's ENS profile right now sees `K_new`, because that's what's currently committed. There is no ambiguity, no version-skew bug, no propagation race. The cooperative dance the CA-rooted PKI requires is reduced to four lines of operational runbook.

The same flow handles **revocation** when the leak isn't recoverable: instead of rotating to `K_new`, the operator updates `pubkey_ed25519` to a sentinel value (32 bytes of `0x00`) that the strict-mode verifier explicitly rejects. The agent is now unable to issue verifiable capsules, and any party reading the records knows the agent is dead. No CRL, no OCSP, no third-party signal — the chain itself is the revocation surface.

## 5. Cross-agent reputation through ENS reverse records

The seven records cover *forward* identity — given an agent name, recover its identity surface. The harder problem in autonomous-agent ecosystems is *reverse* identity: given an action, recover the agent that took it. SBO3L's roadmap (T-4-3) addresses this through ENS reverse records and a portable reputation primitive.

The classical ENS reverse record (`addr.reverse`) maps an Ethereum address to its preferred ENS name. We extend the pattern: each agent's *signing key*, hashed and treated as a synthetic address, gets a reverse record pointing at the agent's ENS name. The flow is:

1. An action is observed on-chain or in an audit log. The action carries an Ed25519 signature.
2. Verifier hashes the public key (deterministic, well-defined: `hash(pubkey_ed25519)` truncated to 20 bytes for address shape) to obtain a synthetic identifier.
3. Verifier queries ENS for the reverse record at `<synthetic-id>.signer.reverse.sbo3lagent.eth` (or a similar standardised reverse zone).
4. Verifier resolves the returned forward name and reads the seven records.
5. Verifier now has an audit-grade chain: action → signature → public key → reverse record → forward identity → policy commitment.

The novelty is the chain, not any single hop. Each hop is a primitive that already exists. Putting them together yields *cross-agent attribution* without a centralised lookup service. An agent that attests to another agent's result (the cross-agent attestation flow in `crates/sbo3l-identity/src/cross_agent.rs`) leaves a signed record that any third party — auditor, regulator, downstream agent — can resolve back to the attesting agent's full identity surface.

Reputation portability is the consequence. An agent that built a 10,000-capsule history of clean decisions on tenant X has a track record that survives the agent moving to tenant Y. The capsules are self-contained, the public key is reachable, the policies are recoverable, the audit chain is anchored. Tenant Y reading the agent's seven records can fetch the audit log, recompute the reputation score, and decide whether to onboard the agent — without trusting tenant X to vouch.

This is the mechanism by which we eventually expect agent reputation to become a first-class web3 primitive on par with token balances or NFT holdings. The audit-chain anchor (`Sbo3lAuditAnchor` on Sepolia, `0x600c10dE...Db37`) is the canonical timestamp source; the reverse-record convention is the deterministic-attribution mechanism; the seven-record forward profile is what those attributions resolve to.

T-4-3 is currently work-in-progress: the contracts and reverse-zone deployment are scoped for Phase 3 (post-hackathon). The conceptual primitive is established; the polish remaining is operational (gas-efficient reverse-zone deployment, indexer for synthetic-id lookups, a reference UI that shows "this attestation came from agent X with reputation 87"). None of that is research; all of it is bounded engineering with clear interfaces.

A subtle but important note: the cross-agent reputation flow does NOT require a tenant or platform to opt in. Any party that observes a signed action — a transaction, a receipt, an attestation — can run the recovery chain. The reputation primitive is therefore *permissionless* in the same sense that ENS resolution is permissionless. It does not depend on cooperation from the agent, the platform, or any other party beyond the public chain.

There is one attack surface worth naming explicitly: an adversary that wants to **launder** a bad-actor agent's reputation could try to register a *new* ENS subname pointing at the same `pubkey_ed25519`, hoping consumers attribute the new identity rather than the old one. The flow above defeats this — the synthetic-id reverse record is keyed on the public key itself, not on the forward name, so the same key resolves to *both* names. A consumer that finds a public key with two reverse mappings MUST treat the agent as ambiguous and require additional disambiguation (a fresh signed challenge, a stable third-party attestation). Implementations SHOULD enforce that a synthetic-id reverse record points at exactly one forward name; ambiguity is a tampering signal, not a feature.

This makes the reverse-record convention *injective by construction*: each Ed25519 verifying key maps to one and only one canonical ENS identity, and an attempt to forge a parallel identity is detectable in the resolver itself rather than in application code. It's the property that makes reputation portability cryptographically meaningful — a 92/100 score against `agent-X` cannot be silently transferred to `agent-Y` by re-registering, because the underlying key is already mapped.

## 6. Comparison — Trust DNS vs the alternatives

The substitution from "naming" to "authentication" is what makes ENS the right substrate. Several existing systems have tried to solve adjacent problems; the table below makes the comparison explicit.

| Property | DNS + DNSSEC | Web2 PKI (X.509 + CAs) | DID (W3C) | Sign-In With Ethereum (EIP-4361) | **Trust DNS (this work)** |
|---|---|---|---|---|---|
| Global namespace | ✅ | ✅ (via CAs) | partial — method-specific | ❌ (per-session) | ✅ via ENS |
| Permissionless registration | partial (registrar-gated) | ❌ (CA-gated) | varies | ✅ | partial (subname owner) |
| On-chain anchoring | ❌ | ❌ | varies | ✅ (signature only) | ✅ (records on chain) |
| Cryptographic key commitment | DNSSEC zone keys | cert in chain | DID Document | session pubkey | `pubkey_ed25519` record |
| Policy commitment | ❌ | ❌ | partial (DID doc fields) | ❌ | `policy_hash` record |
| Audit-chain anchor | ❌ | ❌ | ❌ | ❌ | `audit_root` record |
| Capability advertisement | partial (TXT/SRV) | ❌ | DID services | ❌ | `capability` record |
| Reputation primitive | ❌ | ❌ | ❌ | ❌ | `reputation_score` (CCIP-Read) |
| Existing client tooling | rich | rich | sparse | growing | ✅ (every viem/ethers consumer) |
| No-trust-third-party reads | ❌ (resolver chain) | ❌ (CA chain) | depends | ✅ | ✅ |

DNS gives us the architectural pattern (global, hierarchical, censorship-resistant name → record mapping) but no cryptographic semantics. PKI gives us cryptographic semantics but binds to a CA hierarchy that contradicts the agent-coordination use case. DIDs are the closest analog, and the W3C DID spec is conceptually well-aligned, but the method ecosystem is fragmented (`did:web`, `did:key`, `did:ion`, `did:ethr` all behave differently) and the deployed tooling is thin. Sign-In With Ethereum is excellent for session-bound authentication and a poor fit for the persistent-identity use case.

Trust DNS occupies the gap. It uses ENS where DNS would be (but with cryptographic semantics ENS provides for free), and it commits to a small enough surface (seven records) that interop is straightforward. The substitution is small; the consequences accrue.

## 7. ENSIP-N — the standardisation path

Seven records form an opinionated profile. Opinions don't generalise unless they become standards. The path from "what SBO3L does" to "what every agent platform does" goes through an ENSIP.

[`docs/ENSIP-N-DRAFT.md`](../ENSIP-N-DRAFT.md) is our draft. It is structured to ENSIP authoring conventions:

- **Abstract.** The seven keys above, normalised, with namespace recommendations (`agent_id` rather than `sbo3l:agent_id` so the convention is platform-agnostic).
- **Motivation.** Why agent identity needs a standardised commitment set; what naming-only and bespoke-registry approaches don't deliver.
- **Specification.** Per-key normative format and interpretation. RFC 2119 normative language. Examples from the SBO3L reference deployment.
- **Rationale.** Why these seven; why not five (we tried — the missing two leave gaps); why not twelve (the additional five we considered are application-layer, not identity-layer).
- **Backwards compatibility.** ENS text records are an open namespace; the convention is opt-in; existing names with conflicting key choices are unaffected.
- **Reference implementation.** SBO3L's mainnet deployment (`sbo3lagent.eth`) and the 60-agent Sepolia constellation under it. Both are reproducibly verifiable from a fresh checkout: `sbo3l agent verify-ens sbo3lagent.eth --network mainnet` is the canonical client.
- **Security considerations.** Resolver-rotation as the rotation primitive; capsule-binding to `pubkey_ed25519`; tampering-detectable record drift; CCIP-Read trust model.

The draft has not been submitted to the ENS DAO for adoption yet — that conversation opens after the hackathon submission window per ENS DAO governance norms. The discussion we expect is over the *namespace* (whether `agent_id` is the right key name, vs. `agent.id` or `sbo3l:agent_id`), the *capability vocabulary* (the open-membership tag set will need ecosystem buy-in to stabilise), and whether `reputation_score` belongs in the standard at all (some implementers will argue scores belong in CCIP-Read only, not in the static profile). All three are healthy disagreements; none are blockers.

The draft's normative interface is what matters. A consumer reading a name's seven records gets the same data regardless of which platform issued the agent; a platform issuing an agent gets a known shape its tooling can write to; a verifier gets a known shape it can read against. The convention is the substrate; everything else is implementation detail.

## 8. Why this wasn't a checkbox integration

It would have been easier to ship a single ENS subname for `sbo3lagent.eth`, claim the bounty, and move on. We didn't. Three reasons.

**The substitution was too generative to leave at the surface.** Once we noticed that `name → records` could carry trust commitments — that the difference between DNS and ENS was not just programmability but *cryptographic-anchoring of arbitrary semantics* — every other piece of the SBO3L architecture clicked into place around it. The Passport capsule's signature is verifiable against `pubkey_ed25519`. The policy-hash check resolves to `policy_hash`. The audit chain anchors to `audit_root`. The capability advertisement is `capability`. The system has a coherent identity story because we took the substitution seriously, not as a checkbox.

**The fleet at scale was the only honest demonstration.** Sixty subnames under one parent, each with the full record set, each issued by a one-line CLI command, all resolving from mainnet — is the proof that the convention scales. One subname is a demo; sixty is a deployment. The `Trust DNS` visualization (https://app.sbo3l.dev/trust-dns) renders all sixty in real time, with each WebSocket frame backed by an actual ENS resolution and a signed cross-agent attestation. A judge watching the visualization is watching the convention work at production-shape scale, not at demo-scale.

**The standardisation path is what makes the work valuable beyond SBO3L.** If we kept the seven-record convention private, it would help SBO3L and only SBO3L. By drafting the ENSIP and pushing it for ecosystem review, we make the convention reusable. Every agent platform — LangChain, Anthropic, AutoGen, ElizaOS, the next twenty frameworks — gets the same ENS-rooted identity surface they can write to and read from. Our reference implementation is one of many possible implementations; the convention is what the ecosystem needs.

The closing claim is simple. ENS is not the integration. ENS is the *trust DNS*. The substitution was small; we took it as far as it went; the result is a coherent agent-identity story rooted in the most widely-deployed naming primitive web3 has, with a standardisation path that any other team can adopt without adopting SBO3L specifically.

If you take one thing from this manifesto: **the cleanest agent-identity layer is the one that doesn't ship**. ENS already shipped it. We just had to notice.

---

## A. Reproducibility checklist

| Claim | Verify it yourself |
|---|---|
| Seven `sbo3l:*` records resolve from mainnet | `cargo install sbo3l-cli && sbo3l agent verify-ens sbo3lagent.eth --network mainnet` |
| Mainnet `policy_hash` byte-matches the offline fixture | `sbo3l agent verify-ens sbo3lagent.eth --network mainnet` exits with rc=0 only on byte-match |
| 60-agent fleet on Sepolia, full records | [`docs/proof/ens-fleet-agents-60-2026-05-01.json`](../proof/ens-fleet-agents-60-2026-05-01.json) |
| CCIP gateway live, smoke-tested fail-mode | https://sbo3l-ccip.vercel.app/ + smoke `GET /api/0xdeadbeef/0x12345678.json` returns HTTP 400 (correct rejection) |
| Cross-agent attestation rejected on tamper | `cargo test --test test_cross_agent_verify` — `tampered_attestation_invalid` test |
| Hash-chained audit log tamper-evident | `bash demo-scripts/run-openagents-final.sh` step 11 — strict-hash verifier rejects flipped byte |
| Resolver-rotation flow end-to-end | [`docs/proof/ens-resolver-rotation-2026-05-02.md`](../proof/ens-resolver-rotation-2026-05-02.md) — recorded transcript of rotating the Sepolia OffchainResolver and re-resolving |

## B. References

- [`docs/ENSIP-N-DRAFT.md`](../ENSIP-N-DRAFT.md) — the standardisation companion to this manifesto
- [`docs/proof/ens-narrative.md`](../proof/ens-narrative.md) — long-form (~400 lines) walkthrough with code examples
- [`docs/submission/bounty-ens-most-creative.md`](../submission/bounty-ens-most-creative.md) — judges-facing one-pager for ENS Most Creative
- [`docs/submission/bounty-ens-ai-agents.md`](../submission/bounty-ens-ai-agents.md) — three-layer stack (ENS + CCIP-Read + ERC-8004)
- [ENSIP-1 (ENS specification)](https://docs.ens.domains/ensip/1)
- [ENSIP-10 (wildcard resolution)](https://docs.ens.domains/ensip/10)
- [ENSIP-25 (CCIP-Read)](https://docs.ens.domains/ensip/25)
- [EIP-3668 (CCIP-Read substrate)](https://eips.ethereum.org/EIPS/eip-3668)
- [ERC-8004 (Trustless Agents)](https://eips.ethereum.org/EIPS/eip-8004)
- [`crates/sbo3l-identity/contracts/OffchainResolver.sol`](../../crates/sbo3l-identity/contracts/OffchainResolver.sol) — the on-chain validator deployed on Sepolia
- [`crates/sbo3l-identity/src/cross_agent.rs`](../../crates/sbo3l-identity/src/cross_agent.rs) — the runtime authentication protocol
- [RFC 2119 (Key words for use in RFCs)](https://datatracker.ietf.org/doc/html/rfc2119) — the normative language convention used in this manifesto

---

*Trust DNS Manifesto, version 1.0, 2026-05-03. Authors: Daniel Babjak (`babjak_daniel@hotmail.com`); SBO3L team, ETHGlobal Open Agents 2026.*
