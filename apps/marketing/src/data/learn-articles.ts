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
];

export function getArticle(slug: string): Article | undefined {
  return ARTICLES.find((a) => a.slug === slug);
}
