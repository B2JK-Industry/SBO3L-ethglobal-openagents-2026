// Long-form articles for /learn. Each article is a self-contained
// MDX-shaped string here so we can build them at static-site-generation
// time without an MDX integration. (Adding @astrojs/mdx for 4 articles
// is the wrong tradeoff — 28 KB of React + scheduler hydration just
// to render markdown is more bundle than the pages themselves.)
//
// When the article count grows past ~10, switch to MDX.

export interface Article {
  slug: string;
  title: string;
  description: string;
  reading_min: number;
  audience: string;
  body_html: string;
}

const tierArchHtml = `
<p>
  SBO3L's playground gives every audience their own proof-level. A judge
  glancing for 60 seconds sees motion graphics; a tech sceptic edits the
  policy and re-runs in WASM; a sponsor reviewer submits a real APRP and
  watches the capsule appear on Sepolia. Three tiers, one product.
</p>

<h2>Tier 1 — Mock cinematic</h2>
<p>
  No daemon, no WASM, no network. Pre-rendered animated SVG sequence
  showing what a decision looks like. Auto-loops, ~17 seconds end-to-end,
  ships zero KB of JavaScript. The cost of being wrong here is "the
  judge thinks the demo is cute" — not "the judge thinks the cryptography
  is broken." Cheap to ship, fast to load, accessible (reduced-motion
  pins to scene 3 with the deny code visible).
</p>

<h2>Tier 2 — WASM playground</h2>
<p>
  <code>sbo3l-core</code> compiled to <code>wasm32-unknown-unknown</code>
  via <code>wasm-bindgen</code>. Loads in the browser, runs the same
  decision pipeline as the daemon, signs receipts with a deterministic
  mock key derived from <code>sha256("playground.sbo3l.dev/mock-key-v1")</code>.
  The mock-signing is the catch — capsules from Tier 2 are not
  cryptographically distinguishable from a real attacker who knew the
  same derivation. Tier 2 is for <em>education</em>, not auditable
  evidence.
</p>
<p>
  Bundle weight is the constraint: ~200 KB gzipped target. Anything
  more and the playground becomes annoying to load on mobile, defeating
  the "edit and re-decide in real time" experience.
</p>

<h2>Tier 3 — Hosted live daemon</h2>
<p>
  A real <code>sbo3l-server</code> runs as a Vercel Function (Fluid
  Compute, Node 24 LTS), backed by Vercel Postgres for the audit chain
  and Vercel KV for per-IP rate limiting. Every 6 hours a cron publishes
  an anchor of the audit-chain root to the Sepolia AnchorRegistry
  contract — so any visitor's capsule can be verified against an
  on-chain timestamp.
</p>
<p>
  Tier 3 capsules carry a real Ed25519 signature from a key generated
  at deploy time, stored sealed in Vercel env. The capsule's
  <code>verifier_pubkey</code> field points to that key, registered
  under <code>playground.sbo3l.dev</code> in ENS. A skeptic can verify
  the capsule offline against the public key + verify the public key
  is what ENS says — full chain of custody.
</p>

<h2>Why split into three?</h2>
<table>
  <thead><tr><th>Audience</th><th>Path</th><th>Time</th></tr></thead>
  <tbody>
    <tr><td>Judge in 60s</td><td>Lands → cinematic auto-plays</td><td>~30s</td></tr>
    <tr><td>Tech sceptic</td><td>Tier 2 → edits scenario → verifies WASM source</td><td>3-5 min</td></tr>
    <tr><td>Sponsor reviewer</td><td>Tier 3 → submits APRP → on-chain Etherscan link</td><td>2-3 min</td></tr>
  </tbody>
</table>

<h2>What this is NOT</h2>
<ul>
  <li>Tier 1 is not "the product." It's a teaser for the product.</li>
  <li>Tier 2 capsules are mock-signed — DO NOT use them as real audit
    evidence. The bundle clearly labels every output capsule with a
    <code>"mock_signed": true</code> field.</li>
  <li>Tier 3 is rate-limited (10 req/min/IP) and the audit chain is
    public — don't put real secrets in your APRP. The page banner says
    so explicitly.</li>
</ul>
`;

const auditChainHtml = `
<p>
  The SBO3L audit chain is a hash-chained log: every decision the
  daemon makes appends one row, and each row's hash includes the
  previous row's hash. A single byte-flip anywhere in history breaks
  the chain, and the strict-mode verifier rejects the capsule.
</p>

<h2>The chain rule</h2>
<pre><code>event_n.hash = sha256( event_{n-1}.hash || jcs(event_n.content) )</code></pre>
<p>
  <code>jcs</code> is RFC 8785 canonical JSON — same input bytes on
  every implementation regardless of map ordering or whitespace.
  <code>||</code> is byte concatenation. The first event uses a
  fixed genesis hash (<code>0x000...</code>) so the chain has no
  bootstrap hole.
</p>

<h2>What a tamper looks like</h2>
<p>
  Suppose an attacker edits event N's <code>amount</code> from
  $1000 to $10000 in the daemon's SQLite file. The hash of event N
  changes. Event N+1's hash was computed using the OLD event N hash,
  so the chain link breaks at N+1. The verifier walks the chain from
  the latest event backward, recomputing hashes; the first mismatch
  raises <code>strict_mode_violation</code>. Returns rc=1 (chain
  broken), not rc=0 (clean) or rc=2 (other capsule check failed).
</p>

<h2>Why not Merkle trees?</h2>
<p>
  Append-only chains are simpler than Merkle trees for the SBO3L use
  case: we always read the chain forward (from a known anchor) and
  we never need to prove non-membership. Merkle would add log(N)
  proof size for a property we don't need. The on-chain anchor
  (separate article) is what lets us truncate the chain locally
  without losing audit-grade trust.
</p>

<h2>Performance</h2>
<p>
  Append: one SHA-256 + one INSERT, ~3µs on commodity hardware.
  Verify: SHA-256 the entire chain top-down, ~1ms per 1000 events.
  At 1 million events the verify pass takes ~1 second; for chains
  larger than that, switch to incremental verification using the
  on-chain anchor as the trusted starting point.
</p>
`;

const onchainAnchorHtml = `
<p>
  Hash-chained audit logs are tamper-evident <em>locally</em>: a
  third party who has the chain bytes can detect any byte-flip. But
  they can't prove the chain wasn't replaced wholesale by the
  attacker. SBO3L's on-chain anchor closes that gap — the audit
  chain root is committed to a public blockchain on a regular
  interval.
</p>

<h2>The contract</h2>
<p>
  <code>SBO3LAnchorRegistry</code> on Sepolia at
  <code>0x4C302ba8…E8f4Ac</code>. One function:
</p>
<pre><code>function publish(bytes32 root, uint64 chain_length) external;</code></pre>
<p>
  Each call costs ~24K gas. The contract emits an event with
  <code>(publisher, root, chain_length, timestamp)</code>; we never
  store anything in contract storage beyond a moving "latest"
  pointer per publisher.
</p>

<h2>Cron + key management</h2>
<p>
  A 6-hour cron job on the daemon (or Vercel cron for the playground)
  computes the chain root, packs it into a transaction signed with the
  publisher's wallet, and broadcasts to Sepolia. The publisher key is
  separate from the daemon's signing key — compromise of one doesn't
  unlock the other.
</p>

<h2>Verifying with the anchor</h2>
<p>
  A skeptic given a capsule can:
</p>
<ol>
  <li>Verify the capsule's audit-chain proof goes back to a chain root R.</li>
  <li>Query Etherscan for the AnchorRegistry's <code>publish(R, ...)</code>
    event — confirms R existed on-chain at timestamp T.</li>
  <li>Compare T with the capsule's claimed event timestamp — must be
    in the past.</li>
</ol>
<p>
  Result: a 24K-gas check (one Etherscan API call) gives you proof
  that the agent took the action no later than the on-chain anchor
  block — even if the daemon's whole filesystem is later replaced.
</p>
`;

const mevGuardHtml = `
<p>
  MEV (Maximal Extractable Value) is the silent tax on every on-chain
  swap. A trader-MEV-bot pair spots your transaction in the public
  mempool, front-runs it, lets your slippage execute against their
  position, and skims the difference. SBO3L's MEV guard is a policy
  rule that denies any swap intent whose declared slippage exceeds
  the configured budget.
</p>

<h2>The rule</h2>
<pre><code>[[intents]]
kind = "uniswap.swap"
where.slippage_bps = { lte = 50 }     # 0.5% max
where.recipient = { allowlist = [...] }
require = [{ private_mempool = true, when = { amount_usd = { gt = 5000 } } }]</code></pre>
<p>
  Three layers stack:
</p>
<ol>
  <li><strong>Slippage cap</strong> — denies anything above 0.5% by
    default. Most legitimate swaps fit.</li>
  <li><strong>Recipient allowlist</strong> — denies swaps to
    addresses outside the agent's mandate. MEV exfiltration usually
    targets attacker-controlled addresses.</li>
  <li><strong>Private mempool requirement</strong> — for
    higher-value swaps, demand the agent route through Flashbots
    Protect or similar private RPC. The flag is part of the APRP
    envelope.</li>
</ol>

<h2>What gets denied</h2>
<p>
  Real cases from the test suite:
</p>
<ul>
  <li><code>swap @ 25% slippage</code> → <code>policy.deny_mev_slippage</code></li>
  <li><code>swap to 0xbeef…</code> (not in allowlist) →
    <code>policy.deny_unknown_recipient</code></li>
  <li><code>$50K swap on public mempool</code> →
    <code>policy.deny_requires_private_mempool</code></li>
</ul>

<h2>What this doesn't catch</h2>
<p>
  Sandwich attacks where the bot front-runs <em>and</em> back-runs
  inside your slippage budget. Mitigation: use private mempools for
  anything &gt;$5K, which the policy can require. SBO3L can't reach
  inside the bot's transaction; it can only refuse to sign a
  receipt for an intent that's structurally vulnerable.
</p>
`;

const langChainHtml = `
<p>
  You wrap your agent in LangChain. LangChain wraps your agent's
  <em>reasoning</em>: which tool to call, with which arguments, in
  what order. That's load-bearing. But LangChain doesn't wrap the
  <em>boundary</em> between "the agent decided" and "the action
  executed." That's the gap SBO3L closes.
</p>

<h2>The five-line wire</h2>

<p>Vanilla LangChain — the agent reasons, the tool fires, no audit:</p>

<pre><code>const chain = createOpenAIToolsAgent({ llm, tools, prompt });
const result = await chain.invoke({ input });
// → tool calls happened. Where's the receipt?</code></pre>

<p>With <code>@sbo3l/langchain</code> — same chain, plus a callback handler:</p>

<pre><code>import { Sbo3lCallbackHandler } from "@sbo3l/langchain";

const sbo3l = new Sbo3lCallbackHandler({
  url: "http://localhost:8730",
  agentId: "research-01",
  onDeny: (reason) =&gt; logger.warn("policy deny:", reason),
});

const chain = createOpenAIToolsAgent({ llm, tools, prompt });
const result = await chain.invoke({ input }, { callbacks: [sbo3l] });
// → every tool call now produces a signed receipt
// → policy denies surface as a tool_result with deny_code
// → handler.receipts contains the full audit trail</code></pre>

<h2>What changes</h2>

<ul>
  <li><strong>Every tool call has a signed receipt.</strong> Not a
    callback log. Not a database row your daemon writes after the
    fact. A cryptographic receipt produced <em>before</em> the
    tool executes, signed with the daemon's Ed25519 key, with the
    request hash and policy hash baked in.</li>
  <li><strong>Policy denies are part of the chain output.</strong>
    LangChain's standard tool-call result protocol carries the
    deny code through to the LLM, so the agent can reason about
    rejection ("I can't transfer that much; ask the user to lower
    the amount") instead of crashing.</li>
  <li><strong>The audit chain is queryable.</strong>
    <code>handler.receipts</code> is a list of all the receipts
    from this chain run. Hash-chained, exportable, replayable. Your
    SOC 2 auditor doesn't ask "did the agent do something it
    shouldn't?" — they ask "show me the receipts" and you ship the
    list.</li>
</ul>

<h2>Why your CFO wants this</h2>

<p>
  Imagine the conversation when the LangChain-driven agent does
  something expensive (or wrong, or both):
</p>

<table>
  <thead><tr><th>Without SBO3L</th><th>With SBO3L</th></tr></thead>
  <tbody>
    <tr>
      <td>"What happened?" — engineer reconstructs from logs, OpenAI
        traces, blockchain explorer, maybe LangSmith if they paid
        for it. Hours of forensic work.</td>
      <td>"What happened?" — engineer pulls capsule by request_hash,
        verifies offline against the daemon's published Ed25519
        pubkey, has cryptographic proof of what the agent decided
        and what policy was in force. Minutes.</td>
    </tr>
    <tr>
      <td>"Could it have been worse?" — uncertain. Logs might be
        incomplete; the agent's reasoning chain is ephemeral.</td>
      <td>"Could it have been worse?" — query the policy snapshot
        referenced by <code>policy_hash</code>; show the rules that
        prevented worse outcomes from firing.</td>
    </tr>
    <tr>
      <td>"Who approved this?" — depends on which review you logged.
        Not always available.</td>
      <td>"Who approved this?" — the receipt's
        <code>matched_rule_id</code> points at the exact policy
        rule. The rule's git history shows who shipped it.</td>
    </tr>
  </tbody>
</table>

<h2>What this isn't</h2>

<ul>
  <li><strong>Not a LangChain replacement.</strong> The handler is
    additive. Same agent, same tools, same prompts. SBO3L doesn't
    second-guess the LLM's reasoning — it just enforces the boundary
    around what the LLM can actually <em>do</em>.</li>
  <li><strong>Not a free latency lunch.</strong> Each tool call
    adds one round-trip to the daemon (typically &lt;1ms over Unix
    socket, ~5ms over HTTP). For human-perceived latency this is
    invisible; for high-frequency batch agents you can run the
    daemon in-process via the Rust crate directly.</li>
  <li><strong>Not a substitute for prompt engineering.</strong>
    SBO3L can't stop the LLM from <em>trying</em> to call the wrong
    tool — but it can stop the call from succeeding. The LLM gets
    a deny code; your prompt should teach it to handle that
    gracefully.</li>
</ul>

<h2>Try it</h2>

<p>
  The Node.js + Python LangChain adapter quickstart is at
  <a href="/quickstart/langchain"><code>/quickstart/langchain</code></a>
  — five minutes from <code>npm install</code> to your first
  signed receipt against a local daemon.
</p>
`;

// The Trust DNS Manifesto is a ~5000-word RFC-style document — too large
// to embed inline as HTML in this data file. The /learn card surfaces a
// synopsis and points readers at the canonical source in the repo, where
// the manifesto is rendered with all the cross-references intact.
const trustDnsManifestoHtml = `
<p>
  This is a synopsis. The full manifesto (~5000 words, RFC-style with
  normative MUST/SHOULD/MAY language, eight sections, comparison tables,
  worked rotation examples, and an attack-resistance argument) lives at
  <a href="https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/concepts/trust-dns-manifesto.md"><code>docs/concepts/trust-dns-manifesto.md</code></a>.
</p>

<h2>The substitution that changes everything</h2>
<p>
  Most uses of ENS reduce to <em>naming</em>: a friendlier label for a
  wallet address. What an autonomous-agent ecosystem actually needs is
  <em>authentication</em>: a name that lets a remote verifier reconstruct
  everything they need to know about the named entity, with no shared
  secrets and no trusted intermediary. ENS — precisely because of how
  it was already built — turns out to be the cleanest substrate to make
  that substitution on.
</p>

<h2>The trust profile in seven records</h2>
<p>
  ENS gives us <code>text(node, key)</code>. SBO3L proposes seven keys.
  Each answers a question a remote verifier needs answered.
  <code>agent_id</code> stable identifier · <code>endpoint</code> daemon
  URL · <code>pubkey_ed25519</code> receipt verifying key ·
  <code>policy_hash</code> commitment to the active policy ·
  <code>audit_root</code> anchor to the audit chain ·
  <code>capability</code> sponsor-surface tags ·
  <code>reputation_score</code> portable signal via CCIP-Read.
</p>
<p>
  The seven keys are also the body of <a href="https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/ENSIP-N-DRAFT.md"><code>docs/ENSIP-N-DRAFT.md</code></a>,
  the standardisation companion to this manifesto. Treating them as a
  profile rather than a free-form schema is what lets the convention
  generalise across platforms.
</p>

<h2>Resolver rotation as identity key-rotation</h2>
<p>
  ENS names point to a resolver contract; the owner can change which
  resolver the name points to at any time. If the agent's signing key is
  compromised, the operator runs a four-line runbook: generate the new
  key, update <code>pubkey_ed25519</code> in one transaction, resume
  signing, append a key.rotated entry to the audit chain. There is no
  CRL, no OCSP, no third-party signal — the chain itself is the
  truststore.
</p>

<h2>Cross-agent reputation through reverse records</h2>
<p>
  An action's signature recovers the public key; the public key hashes
  to a synthetic identifier; the synthetic identifier reverse-resolves to
  the agent's forward ENS name; the forward name reads back the seven
  records. The chain is permissionless and injective by construction —
  an attempt to launder bad reputation by registering a parallel name
  with the same key is detectable in the resolver, not in application
  code.
</p>

<p>
  <strong>
    <a href="https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/concepts/trust-dns-manifesto.md">Read the full manifesto →</a>
  </strong>
</p>
`;

const keeperhubComposabilityHtml = `
<p>
  A research agent sees a paid API endpoint, decides "this is worth $0.05,"
  and reaches for a tool to fire the payment. Two things have to happen
  between intent and execution:
</p>
<ol>
  <li><strong>Decide:</strong> is this payment authorized? Within budget? Recipient allowlisted? Right chain? Right risk class?</li>
  <li><strong>Execute:</strong> translate the authorized intent into a signed transaction (or a webhook, or an x402 call) and dispatch it.</li>
</ol>
<p>
  Most "agent payment" stacks today try to do both in one product.
  SBO3L doesn't. KeeperHub doesn't. Together they form the cleanest
  composition we've shipped — a policy boundary that emits a signed
  receipt, and an execution layer that consumes the receipt and
  actually fires the call.
</p>

<h2>1. Why two products beat one monolith</h2>
<p>
  The decision layer is the boundary between "what the agent wants to
  do" and "what the operator allows the agent to do." Its audience is
  the operator: a CFO, a compliance officer, an SRE on call. They edit
  YAML to express budget caps, allowlist providers, set risk thresholds.
  They want the policy engine auditable, deterministic, slow-moving.
</p>
<p>
  The execution layer is the bridge between "an authorized payment
  intent" and "the rails it actually settles on." Its audience is the
  workflow developer wiring KeeperHub up to a Stripe webhook, an x402
  endpoint, a Uniswap router, a Discord bot. They want flexibility,
  fast-evolving, integration-rich.
</p>
<p>
  Bundle them and you compromise both. The decision engine starts
  shipping integrations to chase the latest payment rail. The execution
  layer starts adding policy DSLs to chase compliance reviews. Both get
  worse at their core jobs. Split them and each grows on its own clock —
  <em>if</em> the contract between them is right.
</p>

<h2>2. The signed receipt as the contract</h2>
<p>
  When SBO3L decides on an APRP (Agent Payment Request Protocol body),
  it emits a 14-field <code>PolicyReceipt</code>. Three things make it
  the right contract:
</p>
<ol>
  <li><strong>Content-addressable.</strong> The <code>request_hash</code> pins the exact APRP bytes that were decided on. KeeperHub can re-derive the hash from the body it receives and refuse to execute on mismatch. No "agent edited the request between decision and execution" attack.</li>
  <li><strong>Offline-verifiable.</strong> Anyone with the policy signer's public key can verify the Ed25519 signature without contacting SBO3L. KH verifies before executing. An auditor reading the audit log months later verifies too. No "trust SBO3L" step.</li>
  <li><strong>Carries the audit pointer.</strong> <code>audit_event_id</code> references a node in SBO3L's hash-chained audit log. KH echoes this ID back on its execution row, giving an auditor a single ID to walk both directions.</li>
</ol>
<p>
  This receipt is the only thing the two layers exchange. SBO3L doesn't
  know what KeeperHub's webhook URL is. KeeperHub doesn't know what
  policy YAML SBO3L is running. Each side evolves freely as long as
  the receipt schema holds.
</p>

<h2>3. Five integration paths (IP-1..IP-5)</h2>
<p>
  Once you have a receipt-as-contract, the next question is: how tight
  can the composition get? Five paths, each independently shippable:
</p>
<ul>
  <li><strong>IP-1 — sbo3l_* upstream-proof envelope fields.</strong> SBO3L's KH adapter posts the receipt's <code>request_hash</code> + <code>policy_hash</code> + <code>policy_version</code> + <code>audit_event_id</code> + <code>signature_hex</code> as five optional <code>sbo3l_*</code> fields alongside the workflow body. KH stores them. <strong>Shipped on the SBO3L side</strong>; pending KH-side schema adoption.</li>
  <li><strong>IP-2 — Public submission/result envelope JSON Schema.</strong> A Draft 2020-12 schema documenting the bidirectional wire shape. Adapter authors stop reverse-engineering responses from <code>curl -v</code>.</li>
  <li><strong>IP-3 — keeperhub.lookup_execution MCP tool.</strong> Symmetric MCP tool letting auditors query execution status + run-log + sbo3l_* fields without raw HTTP plumbing. <strong>Shipped on the SBO3L side</strong> as <code>sbo3l.audit_lookup</code>.</li>
  <li><strong>IP-4 — Standalone sbo3l-keeperhub-adapter Rust crate.</strong> Any third-party agent framework can <code>cargo add sbo3l-keeperhub-adapter</code> with no transitive dependency on the SBO3L policy engine. <strong>Shipped at v1.2.0 on crates.io.</strong></li>
  <li><strong>IP-5 — SBO3L Passport capsule URI on the execution row.</strong> A single optional string column — the URI to a self-contained verifiable bundle (APRP + receipt + audit segment + executor evidence + verification metadata). Capsule schema + verifier shipped; pending KH-side column adoption.</li>
</ul>
<p>
  Stacking all five gives end-to-end offline auditability of every
  KeeperHub execution that flowed through SBO3L. Two different products,
  one verifiable trail.
</p>

<h2>4. End-to-end demo</h2>
<p>Five lines in TypeScript:</p>
<pre><code>import { SBO3LClient } from "@sbo3l/sdk";
import { sbo3lKeeperHubTool } from "@sbo3l/langchain-keeperhub";

const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
const tool = sbo3lKeeperHubTool({ client });
// pass \`tool\` (or wrap as DynamicTool) into your LangChain agent's tool list</code></pre>
<p>What happens when the agent calls <code>tool.func(JSON.stringify(aprp))</code>:</p>
<ol>
  <li>POST to SBO3L daemon at <code>/v1/payment-requests</code> with the APRP body</li>
  <li>SBO3L decides allow / deny / requires_human against the policy + budget + nonce + provider trust list</li>
  <li>On allow: SBO3L's <code>executor_callback</code> hands the signed receipt to the daemon-side KeeperHub adapter</li>
  <li>KH adapter POSTs the IP-1 envelope to the workflow webhook, captures <code>executionId</code></li>
  <li>Tool returns <code>{decision, kh_workflow_id_advisory, kh_execution_ref, audit_event_id, request_hash, policy_hash, deny_code}</code></li>
</ol>

<h2>5. The 15 issues we filed on KeeperHub/cli</h2>
<p>
  Building the composition end-to-end surfaced 15 concrete, actionable
  asks. We filed them all on
  <a href="https://github.com/KeeperHub/cli/issues?q=is%3Aissue+author%3AB2JK-Industry">KeeperHub/cli</a>:
</p>
<ul>
  <li><strong>Round 1 (#47–#51)</strong> — couldn't-get-it-working frictions: token-prefix split, envelope schema, executionId lookup, sbo3l_* fields adoption, idempotency-key dedup.</li>
  <li><strong>Round 2 (#52–#56)</strong> — post-integration concerns: HTTP error code catalog, public mock fixture suite, webhook timeout SLO, schema versioning headers, max payload size.</li>
  <li><strong>Round 3 (#58–#62)</strong> — production-grade reliability: HMAC-SHA256 signature, workflow versioning + back-compat, response envelope JSON Schema, rate-limiting headers, delivery guarantees doc.</li>
</ul>
<p>
  Each issue carries a worked reproduction, a citation to the exact
  line in our adapter where the friction surfaces, and a proposed
  shape for the fix. Five have <strong>companion draft PRs</strong> on
  our repo showing the consumer-side adapter change ready to ship the
  day KH lands the upstream contract.
</p>

<h2>6. Looking forward — joint roadmap</h2>
<ul>
  <li><strong>Phase 1:</strong> IP-1 + IP-2 land on KeeperHub. A workflow author opts into <code>sbo3l_*</code> envelope fields with one checkbox. The submission/result schema is published. Adapters across the ecosystem standardise.</li>
  <li><strong>Phase 2:</strong> IP-3 + IP-5 ship. Vendor-neutral MCP tool surface for execution lookup. Capsule URI column on the execution row makes "show me the proof" a one-click download.</li>
  <li><strong>Phase 3:</strong> Multi-tenant trust DNS. Every agent gets its own ENS name; trust commitments behind each name (policy hash, signing key, deployer attestation) resolve from a single DNS-style query.</li>
</ul>
<p>
  This isn't a replacement for either product. It's a clean composition
  where each layer keeps doing what it's best at, and the contract
  between them carries enough cryptographic proof to satisfy the most
  paranoid auditor.
</p>
<p>
  If you're shipping an agent today, you already have an unsolved
  gate-then-execute problem. Start with the composition.
</p>

<p>
  <strong>
    <a href="https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/proof/blog-keeperhub-composability.md">Full long-form version →</a>
  </strong>
</p>
`;

export const ARTICLES: Article[] = [
  {
    slug: "tier-architecture",
    title: "How the SBO3L playground works (3-tier architecture)",
    description: "Why the playground splits into mock cinematic, WASM client-side, and hosted live daemon — three tiers for three audiences.",
    reading_min: 6,
    audience: "Anyone trying to evaluate SBO3L from the playground",
    body_html: tierArchHtml.trim(),
  },
  {
    slug: "audit-chain",
    title: "How the audit chain prevents tampering",
    description: "SHA-256 + JCS canonical JSON + append-only — why a single byte-flip breaks the whole chain.",
    reading_min: 4,
    audience: "Engineers evaluating cryptographic audit guarantees",
    body_html: auditChainHtml.trim(),
  },
  {
    slug: "onchain-anchor",
    title: "On-chain anchoring: closing the local-tamper gap",
    description: "Hash-chained logs detect local tampers. On-chain anchors detect wholesale chain replacement. 24K gas per anchor; 6h cron.",
    reading_min: 4,
    audience: "Auditors + compliance teams + sponsor reviewers",
    body_html: onchainAnchorHtml.trim(),
  },
  {
    slug: "mev-guard",
    title: "MEV guard — three layers of slippage defense",
    description: "Slippage cap + recipient allowlist + private mempool requirement. What gets denied, what doesn't.",
    reading_min: 3,
    audience: "Treasury automation + DEX-trading agents",
    body_html: mevGuardHtml.trim(),
  },
  {
    slug: "why-langchain-needs-sbo3l",
    title: "Why LangChain needs SBO3L (and what changes when you wire them)",
    description: "LangChain wraps your agent's reasoning. SBO3L wraps LangChain's tool calls in policy + signed audit. Five lines of code, one boundary your CFO understands.",
    reading_min: 5,
    audience: "LangChain devs already shipping agents in production",
    body_html: langChainHtml.trim(),
  },
  {
    slug: "trust-dns-manifesto",
    title: "Trust DNS Manifesto — naming as authentication for autonomous agents",
    description: "RFC-style 5000-word manifesto. Why SBO3L resolves ENS names to trust commitments rather than wallet addresses; the seven-record opinionated profile; the standardisation path through ENSIP-N.",
    reading_min: 22,
    audience: "ENS standards reviewers, ERC-8004 implementers, agent-platform architects",
    body_html: trustDnsManifestoHtml.trim(),
  },
  {
    slug: "keeperhub-composability",
    title: "Don't give your agent a wallet. Don't make KeeperHub guess what's authorized either.",
    description: "Composing SBO3L's policy boundary with KeeperHub's execution layer. Why two products beat one monolith, the signed receipt as the contract, IP-1..IP-5, end-to-end demo, the 15 issues we filed, joint roadmap.",
    reading_min: 8,
    audience: "Agent-platform engineers + KeeperHub workflow authors evaluating end-to-end safety",
    body_html: keeperhubComposabilityHtml.trim(),
  },
];

export function getArticle(slug: string): Article | undefined {
  return ARTICLES.find((a) => a.slug === slug);
}
