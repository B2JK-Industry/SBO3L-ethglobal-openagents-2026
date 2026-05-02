# Out-of-scope vulnerabilities

> See [`SECURITY.md`](../../SECURITY.md) for the in-scope policy. The list below collects categories that are **not eligible for bounty payout** with the reasoning. Reports in these categories may still be acknowledged in the Hall of Fame at our discretion.

## Documented dev-only modes

| Surface | Why excluded |
|---|---|
| `SBO3L_ALLOW_UNAUTHENTICATED=1` | The daemon prints `⚠ UNAUTHENTICATED MODE — DEV ONLY ⚠` at startup. This mode skips the `Authorization: Bearer` gate by design — exploits against it are exploits of a documented dev-only configuration. |
| `local_mock()` sponsor adapters in `crates/sbo3l-execution/` | Test fixtures returning canned responses; never wired to production daemon. |
| Dev signer (`SBO3L_SIGNER_BACKEND=dev` + `SBO3L_DEV_ONLY_SIGNER=1`) | Hard-coded test keys; banner printed at startup. |
| `examples/` apps | Demonstration code, not a deployed surface. |

## Non-production surfaces

| Surface | Why excluded |
|---|---|
| `*.vercel.app` preview URLs | Marketing/demo only; not the production trust boundary. Once `sbo3l.dev` DNS is pointed, only the canonical `https://sbo3l.dev` and subdomains are in scope. |
| Sepolia testnet ENS records | Test fixtures, not user-facing trust data. |
| Sepolia OffchainResolver `0x7c69…A8c3` | Demonstration deploy on testnet. Mainnet deploy will be in scope when it ships. |
| `examples/uniswap-agent-{ts,py}` | Demo agents pointing at Sepolia. |

## Categories that are not vulnerabilities

- **Rate-limit complaints** (e.g. "your daemon throttles after 1000 RPS") — by design.
- **Missing security headers on Vercel previews** — covered by Vercel's defaults; we ship strict CSP on `vercel.json` for marketing.
- **HTTPS configuration on third-party domains** (npm, crates.io, PyPI) — out of our control.
- **Reports that require physical access** to a victim's machine.
- **Reports that require social engineering** of SBO3L staff.
- **Reports against unsupported / archived branches** (anything but `main`).
- **Self-XSS** — requires the victim to paste attacker-supplied JS into their own console.
- **Outdated browser warnings** — we support recent versions; older browsers may render incompletely.

## Transitive dependencies

A vulnerability in a transitive dep (e.g. `serde`, `tokio`, `axum`) is **not** an SBO3L vulnerability. Report it directly to the affected project; we'll issue a SBO3L advisory after the upstream fix lands.

If you find a way to **reach** the transitive vulnerability through a SBO3L public surface (e.g. crafted APRP triggering a `serde_json` panic), that **is** in scope under the appropriate severity tier.

## DoS & resource exhaustion

- **Network-level DoS** (flooding our daemon with TCP connections) is out of scope.
- **Application-level DoS** (a single crafted APRP that exhausts memory / hangs the daemon for >30s) is in scope at Medium severity.
- **Storage DoS** (filling SQLite with a small number of crafted high-fanout APRPs) is in scope at Medium.

## Cryptographic assertions we do NOT make

These are **NOT** security claims; reports that "break" them are not bounty-eligible:

- **Quantum resistance** — we use Ed25519 + secp256k1 + SHA-256, all known to be vulnerable to a sufficiently-large quantum computer. A post-quantum migration is on the Phase 4+ roadmap.
- **Hardware-secured signing** in the dev signer path — the dev signer holds keys in process memory by design.
- **Side-channel resistance** at the host level — we depend on `dalek-cryptography` and `secp256k1` for constant-time primitives, but a TEE-grade attacker can still extract keys from a compromised host.

## See also

- [`../../SECURITY.md`](../../SECURITY.md) — top-level security policy.
- [`pgp-key.asc`](pgp-key.asc) — PGP key for `security@sbo3l.dev`.
