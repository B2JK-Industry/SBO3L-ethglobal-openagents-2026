# Knowledge Base - Mandate

> **Účel:** Kompendium hard-to-find informácií, version pinov, library quirks, gas costov a gotchas. Čerpa sa z 5 paralelných deep-research stretov (Apr 2026). Implementačný agent (alebo ja v loope) si tu nájde odpoveď, kým ju musí googliť.
>
> **Skratky:**
> - **GO** = "Go ahead, this is the recommended choice"
> - **DON'T** = "Verified anti-pattern, will burn you"
> - **PIN** = "Lock this version exactly; don't auto-upgrade"
> - **WATCH** = "Active issue / drift / changing"

---

## §1 TEE Attestation

### §1.1 Intel TDX (Trust Domain Extensions)

**Hardware availability (April 2026):**
- TDX is GA on **5th-gen Xeon (Emerald Rapids)** and **6th-gen Xeon (Granite Rapids P-core)**, late 2024–2025.
- Sapphire Rapids (4th-gen) had it limited / late-enabled via microcode.
- **Granite Rapids-WS (workstation)** announced Feb 2026 on LGA 4710 — *first time TDX is reachable outside DC SKUs*.
- **Consumer Core CPUs do NOT have TDX.**
- Cloud SKUs: Azure DCesv5/ECesv5, GCP `c3-standard --confidential-instance-type=TDX`. **AWS does NOT offer TDX** (uses Nitro instead).

**Quote format:**
- Quote v4 (legacy) and **Quote v5 (current, supports TD10/TD15 module reports + multi-cert)**.
- Header (48B) + body (TDREPORT-derived, 584B for v4) + ECDSA-P256 sig + cert chain (PCK leaf + Intel SGX Root CA).
- Body contains: `MRTD` (build-time, hash of TDVF/OVMF + initial TD memory), `MRCONFIGID`, `MROWNER`, `MROWNERCONFIG`, plus **RTMR[0..3]** (runtime TPM-PCR-like extension registers).
- Convention: RTMR0 = firmware events, RTMR1 = kernel, RTMR2 = kernel cmdline+initrd, RTMR3 = application/orchestrator-defined (dstack uses it for compose-hash/app-id/instance-id).

**DCAP flow:**
- TD calls `TDG.VP.VMCALL<GetQuote>` → hypervisor forwards to **QGS** (Quote Generation Service, runs on host, talks to SGX Quoting Enclave because TDX leverages SGX QE for quote signing).
- PCK cert fetched from **PCCS** (caches Intel PCS).
- Verifier needs: PCK cert chain + TCB Info (signed by Intel TCB Signing CA) + QE Identity + CRLs.

**PCCS gotchas:**
- Default PCCS at `localhost:8081` requires Intel API key (free, registration at api.portal.trustedservices.intel.com).
- Must be reachable from **QGS host (NOT the TD guest)**.
- On Azure/GCP, use cloud-provided PCCS (`global.acccache.azure.net`, GCP equivalent).
- Retrieval: **`dcap-artifact-retrieval`** (Rust crate) handles both.

**Quote-from-TD pitfalls:**
- **GO**: Use **configfs-tsm (kernel ≥ 6.7)**, not the legacy vsock path. Path: `/sys/kernel/config/tsm/report/<name>/`. Write 64 bytes to `inblob` (report data), read `outblob` for the quote. Set `DCAP_TDX_QUOTE_CONFIGFS_PATH` for libs that look it up.
- **DON'T**: Vsock path requires QEMU `-object tdx-guest,quote-generation-service=vsock:2:4050` and host-side `qgsd` running — common error: `qgsd` cannot create `/var/opt/qgsd/.dcap-qcnl/` (perms) → "No certificate data for this platform."
- **DON'T**: Ship TDREPORT (raw, MAC'd) — local-only. **Ship the Quote.**
- The 64-byte report-data field is your only attestation-bound free-form payload. **Bind your TLS pubkey or contract address hash here.**

---

### §1.2 AMD SEV-SNP

**Hardware:** SEV-SNP since Milan (Zen 3, 2021), mature on Genoa/Bergamo (Zen 4) and Turin (Zen 5, 2024).

**Differences vs TDX:**
- Both VM-level CC.
- SEV-SNP memory encryption: AES-XEX (per-page tweak); TDX uses AES-XTS-128 with MKTME.
- SEV-SNP attestation simpler (no enclave-style QE), but cert distribution per-chip (VCEK).

**Report format:**
- 1184-byte `ATTESTATION_REPORT` struct (AMD SEV-SNP ABI v1.55+).
- Signed ECDSA-P384 by VCEK.
- Contains `MEASUREMENT` (launch digest), `HOST_DATA`, `REPORT_DATA` (64B), `ID_KEY_DIGEST`, platform info, TCB versions.

**Cert chain:**
- ARK (root, self-signed, AMD per-product-line) → ASK (signing) → VCEK (per-chip, derived from chip ID + TCB version).
- Fetch from KDS: `https://kdsintf.amd.com/vcek/v1/{Milan|Genoa|Turin}/<hwid>?...`.
- **VCEK rotates on TCB update** — cache invalidation is the #1 ops issue.
- **GO**: Use **VLEK alternative** if you want cloud-managed certs (Azure does this).

**Operational issues:**
- **CVE-2024-56161** — admin-privileged microcode patch loader signature bypass. Patch via SEV firmware ≥1.55.21 + AGESA update.
- **Heracles (CCS 2025)** — chosen-plaintext on AES-XEX. Mitigation: Zen 5 hypervisor-ciphertext-hiding feature (`SNP_FEATURES_ENABLED.CIPHERTEXT_HIDING_EN`).
- KDS rate limits and outages — always cache and configure fallback (Azure THIM mirror).

---

### §1.3 On-chain DCAP verification (the ETHPrague differentiator)

**Automata stack (de-facto):**
- `automata-network/automata-on-chain-pccs` — Solidity PCCS, permissionless cert/CRL/TCB-info upload.
- `automata-network/automata-dcap-attestation` — entrypoint contract, dispatches to v3/v4/v5 verifier.
- **PIN**: `automata-dcap-attestation` v1.1 (2025-2026), audited by Trail of Bits (March 2025).
- Deployed on **20+ chains** incl. Optimism, World Chain, HyperEVM, Base.

**Gas costs (DCAP v1.1, 2026):**
- Full on-chain verify of a TDX quote: **~4M gas with RIP-7212 precompile**, **~5M without**.
- ZK route (RISC Zero or SP1): **~250–400k gas** to verify SNARK + ~$0.05–0.20 in proof generation off-chain.
- On Base (post-Ecotone, ~0.05 gwei base fee typical): 4M-gas verify costs **$0.05–0.30** depending on blob market.
- On Arbitrum: similar (~$0.10–0.40).
- On Ethereum L1: **$15–60** at typical 10–30 gwei — **DON'T** verify DCAP on L1 directly, use ZK path or L2.

**RIP-7212 (P-256 precompile):**
- Address `0x0000…0100` does P-256 verify in **3,450 gas** vs ~330k for Solidity polyfill.
- DCAP cert chain is P-384 (not 7212), but QE signature over quote is P-256.
- **Live on:** Base (post-Ecotone), Polygon zkEVM/PoS, Arbitrum (ArbOS 31 "Bianca"), Optimism, zkSync.
- **NOT on Ethereum L1** yet (EIP-7951 pending).

**Contract pattern:**
```solidity
(bool ok, bytes memory output) = AutomataDcapAttestation
    .verifyAndAttestOnChain(bytes calldata quote);
// output contains: mrSigner, mrEnclave/mrTd, reportData
require(allowedMrTd[parsed.mrTd]);
require(parsed.reportData[0:32] == sha256(agentPubkey));
```

**Common mistakes:**
- **DON'T**: Trust quote without checking PCCS freshness. TCB info has expiry; old PCCS snapshot lets revoked enclaves pass. (January 2026 dstack disclosure by Saxena hit exactly this.)
- **DON'T**: Pin only `mrSigner` — pinning only `mrSigner` lets attacker run *any* code Intel signed. **Pin BOTH `mrSigner` AND `mrEnclave/mrTd`.**

**Marlin Oyster (for AWS Nitro on-chain, alternative path):**
- Specializes in AWS Nitro on-chain verification (not DCAP).
- Two-stage on-chain pure verify: **~70M gas total** (cert chain stage ≤50M, attestation stage ≤20M) — **only practical via wrap-and-resign approach**.

**Production users:** Flashbots (TDX builders), Espresso (sequencer attestations), Phala, Marlin, Unichain.

---

### §1.4 Phala dstack

- Repo: `Dstack-TEE/dstack`. Apache-2.
- Includes **dstack-KMS (a TApp itself)** deriving app-bound keys from MRTD+RTMR3.
- `Phala-Network/dcap-qvl` — **pure-Rust DCAP QVL** (SGX+TDX quotes). Used by dstack-verifier and compiled to RISC0/SP1 for ZK path.
- **proof.t16z.com** — Phala-hosted public TEE Attestation Explorer. Paste a quote, get parsed report + verification status. **Use for debugging.**
- **WATCH (Jan 2026):** dstack attestation pipeline hardened after researcher Rahul Saxena reported issues — moved to "Secure-by-Default" mandatory infra-level checks. **If you fork dstack older than that, upgrade.**

---

### §1.5 Intel SGX status (2026)

- **Consumer CPUs:** Deprecated since 11th-gen Core (2021), removed from 12th-gen+. (Killed 4K UHD Blu-ray DRM as collateral damage.)
- **Server (Xeon):** SGX is **still shipped and supported on Xeon SP** through current Granite Rapids. Used because (a) TDX QE is itself an SGX enclave — TDX quote signing depends on SGX, (b) install base of SGX-native apps.
- **For new builds:** **GO** with TDX. **DON'T** start new SGX projects unless you need minimal TCB (just your enclave, not whole guest kernel).

---

### §1.6 AWS Nitro Enclaves

**Format:** CBOR-encoded **COSE_Sign1** (RFC 8152). Signature **ECDSA-P384 / SHA-384**.

**Payload contains:** `module_id`, `digest`, `timestamp`, `pcrs` (map of PCR0–PCR15: PCR0=enclave image, PCR1=kernel+bootstrap, PCR2=app, PCR3=IAM role, PCR4=instance ID, PCR8=signing cert), `certificate` (leaf), `cabundle` (chain to AWS Nitro root), `public_key`, `user_data`, `nonce`.

**vs Intel/AMD:** No external CA-as-a-Service like Intel PCS — AWS publishes single root cert (pinned SHA-256, validity 2019–2049). No TCB recovery dance. Simpler but you trust AWS entirely.

**KMS integration pattern:** Enclave calls `kms-decrypt`/`kms-generate-data-key`/`kms-generate-random` via vsock proxy. SDK auto-attaches attestation document. KMS key policy gates with `kms:RecipientAttestation:ImageSha384`, `kms:RecipientAttestation:PCR0/1/2/8`. KMS encrypts response to enclave's ephemeral pubkey from attestation doc → only that enclave can decrypt.

- **DON'T**: skip `nitro-cli build-enclave --signing-certificate`. Otherwise PCR8 is zero and you can't enforce code identity properly.

**Verifier libs:** `aws-nitro-enclaves-cose` (Rust), `aws-nitro-enclaves-attestation` (Rust — see Trail of Bits Feb 2024 critique on PCR0 naïveté), `aws-nitro-enclaves-nsm-api`.

---

### §1.7 TEE side-channels (relevant)

| Attack | Status | Mitigation |
|---|---|---|
| Spectre/Meltdown (2018) | Mitigated (microcode + L1D flush) | Baked in |
| Foreshadow/L1TF (2018) | Mitigated | Baked in |
| ÆPIC Leak (CVE-2022-21233) | Mitigated (microcode) | — |
| TDXdown (2024) — single-step + instr counting | Mitigated (Intel µcode randomized exit timing) | — |
| Heckler / Ahoi (2024) — interrupt injection | Mitigated (RFLAGS.IF + IRQ vector restriction; TDX 1.5+, SEV-SNP firmware) | — |
| Heracles (CCS 2025) — AES-XEX side channel | Mitigated by Zen 5 ciphertext-hiding | — |
| **TEE.fail (Oct 2025)** — sub-$1000 DDR5 memory-bus interposer | **Physical attack only.** Intel/AMD released µcode equalizing DDR5 timing but class remains for physically-present adversaries. | Defense in depth: rate-limit signing + per-tx policy + on-chain spending limits |

**Implication for vault:** Cloud TEEs trustworthy against remote attackers. Assume attacker with physical DC access can extract keys (TEE.fail). **For sovereign home server: physical theft is the bigger risk than side-channel.**

---

### §1.8 TEE Measurement-bound Key Sealing

**SGX legacy:** `EGETKEY` with `KEYREQUEST.KEYPOLICY = MRENCLAVE` derives sealing key bound to enclave measurement.

**TDX 1.x (current):** No native sealing instruction. Workaround stack:
1. Run SGX QE on same host that wraps TDX measurements into key derivation.
2. Or use **KMS-as-TApp pattern (dstack-KMS):** separate TD acts as KMS, holds long-lived seed in own sealed storage (host-side), derives keys via `HKDF(seed, MRTD || RTMR3 || requesting_app_id)`. KMS only releases derived key after verifying requester's attestation quote.
3. dstack records `compose-hash || instance-id || app-id || key-provider` into RTMR3 specifically for this.

**TDX 2.0:** Adds `TDG.MR.KEY.GET` for native sealed storage (analogous to SGX `EGETKEY`). Same security properties, no SGX dependency. **Not yet on Granite Rapids generally**; check TDX module version.

**SEV-SNP equivalent:** `MSG_KEY_REQ` to PSP returns a key derived from chip secrets + caller's `MEASUREMENT` + optional `GUEST_FIELD_SELECT`.

**Pattern for our vault:** **DON'T** seal the signing key directly. **GO**: Seal a **wrapping key**, store the encrypted signing key off-TEE (e.g., in a database). On boot: attest, derive wrapping key from measurements, decrypt signing key into TD memory only. Lets you rotate TDs without re-keying Ethereum address.

---

### §1.9 Attestation verifier libraries

**Rust (most mature ecosystem):**
- **`dcap-qvl`** (Phala) — pure-Rust SGX+TDX verification, no OpenSSL. **GO** for ZK-compilable verifier.
- **`sev`** (virtee/sev) — AMD SEV/SEV-SNP ABI + attestation. Features `openssl` or `crypto_nossl` (pure Rust: p384, rsa).
- **`tdx-quote`** — parses TDX v4/v5 quotes + PCK chain verification.
- **`dcap-artifact-retrieval`** — fetches PCK certs/TCB info/CRLs from Intel PCS or Azure cache.
- **`automata-network/tdx-attestation-sdk`, `amd-sev-snp-attestation-sdk`** — generation + RISC0/SP1 ZK proof creation.
- **`aws-nitro-enclaves-cose`, `aws-nitro-enclaves-attestation`** — Nitro side.
- **`entropyxyz/configfs-tsm`** — clean kernel-configfs binding for quote generation.

**Go:** `google/go-tdx-guest`, `google/go-sev-guest`, `google/go-eventlog` — Google's clean reimplementations.

**TS/JS:** `@phala/dstack-sdk` — client SDK incl. quote parse/verify.

**"If you need X check Y" cheatsheet:**
| Need | Use |
|---|---|
| Pure-Rust offline TDX/SGX verify | `dcap-qvl` |
| Same logic compiled to ZK | `automata dcap-rs` in `tdx-attestation-sdk` |
| Production EVM on-chain DCAP | `automata-dcap-attestation` v1.1 (use SP1 path) |
| Nitro on-chain | Marlin `oyster-attestation-verifier` (wrap-and-resign) |
| Quote debugging UI | `proof.t16z.com` |
| AMD SEV-SNP from Rust | `virtee/sev` with `crypto_nossl` |
| Measurement-bound keys without writing own KMS | `dstack-KMS` (run as TApp) |
| Native TDX sealing | Wait for TDX 2.0 + `TDG.MR.KEY.GET`, or use SGX QE bridge today |

---

## §2 x402 Protocol

### §2.1 Current state (April 2026)

- **Spec repo:** `github.com/coinbase/x402`. Spec lives in `/specs/x402-specification.md` and `/specs/transports-v2/http.md`.
- Now governed by the **x402 Foundation** (under Linux Foundation since April 2, 2026; founding members: Coinbase, Google, AWS, Microsoft, Stripe, Visa, Mastercard, Cloudflare, KakaoPay).
- **PIN to v2** (launched December 11, 2025). v2 is non-breaking per maintainers but cleans up types and adds Extensions.

### §2.2 Headers (transport v2 — exact names)

- `PAYMENT-REQUIRED` (server → client, 402 response): base64-encoded JSON `PaymentRequired` schema.
- `PAYMENT-SIGNATURE` (client → server, retry): base64-encoded JSON `PaymentPayload`.
- `PAYMENT-RESPONSE` (server → client, 200): base64-encoded JSON `SettlementResponse` (contains tx hash, network, payer).
- **WATCH:** Coinbase docs/SDK still also reference `X-PAYMENT` / `X-PAYMENT-RESPONSE` (legacy v1) — **gotcha:** SDKs in the wild mix both casings. **Implement both, prefer v2.**

### §2.3 Signature scheme — `exact` scheme (EVM)

**EIP-3009 `transferWithAuthorization` typed-data signature on USDC contract.**

Fields:
```
from, to, value, validAfter, validBefore, nonce
```

- Facilitator submits the authorization on-chain.
- **Solana scheme:** signed transaction (different envelope).
- **Algorand scheme:** Ed25519 / msig / lsig with msgpack-encoded atomic groups.

### §2.4 Network IDs (CAIP-style in payload)

| Chain | CAIP ID | USDC address |
|---|---|---|
| Base mainnet | `eip155:8453` | `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` |
| Base Sepolia | `eip155:84532` | (testnet USDC) |
| Polygon | `137` | `0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359` |
| Arbitrum One | `42161` | `0xaf88d065e77c8cC2239327C5EDb3A432268e5831` |
| World | `480/4801` | `0x79A02482A880bCE3F13e09Da970dC34db4CD24d1` |
| Solana mainnet/devnet | (SPL) | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` |

### §2.5 Adoption (March 2026)

- 75M+ tx, 94k unique buyers, 22k sellers, ~$28k/day on-chain volume.
- **49% of facilitator volume now flows through non-Coinbase facilitators** (e.g. ChaosChain, Cronos, MultiversX) — **important: you can avoid Coinbase as facilitator.**

### §2.6 Common gotchas

- `validAfter`/`validBefore` are **seconds, not ms**. Clock skew between vault and facilitator silently fails settlement.
- The `nonce` in EIP-3009 is a **random bytes32, not a sequence**. Agents that reuse nonces will get silently rejected by the on-chain authorization.

### §2.7 l402 (Lightning) alternative

- **Spec:** `github.com/lightninglabs/L402/blob/master/protocol-specification.md`. Site `l402.org`.
- Lightning Labs published "Lightning Agent Tools" (Feb 2026) — 7 composable skills, includes `lnget` (L402-aware HTTP client).
- **Mechanism:** macaroon (root key + caveats) + Lightning preimage. Server emits 402 with `WWW-Authenticate: L402 macaroon="...", invoice="lnbc..."`. Client pays Lightning invoice, presents `Authorization: L402 <macaroon>:<preimage>`.
- **Key differences vs x402:**
  - Verification *stateless* (macaroon = self-contained proof, no chain query).
  - Settles in **<1 s end-to-end** vs 1–3 s on Base.
  - Routing fees often <1 sat (sub-cent).
  - True floor microsats.
- **Prefer L402 when:** sub-cent micropayments, no facilitator dependency, BTC-native, sub-second latency, stateless verification.
- **Prefer x402 when:** USDC stablecoin denomination, EVM smart-contract composability, or customer's agent only carries stable balance.

---

## §3 Competitor Landscape (March 2026)

### §3.1 Coinbase CDP / AgentKit

- Repo: `github.com/coinbase/agentkit`. Docs: `docs.cdp.coinbase.com/agent-kit`.
- **Agentic Wallets GA Feb 2026.**
- Custodial: keys live in Coinbase's TEE infra, never in agent's prompt/LLM.
- Built-in **KYT** (Know Your Transaction) screening blocks "high-risk" interactions — **non-overridable, real friction point.**
- **Controls:** session caps, per-tx caps, allowlists. Stablecoin-only rails (USDC).
- No native session keys; **no built-in spending limits in AgentKit core** (those live at app layer).
- Notable launch partner: **World** integrated AgentKit + x402 with World ID human-verification (Mar 17, 2026).
- **Complaints:** custodial; KYC obligations on entity owning wallet; KYT can't be disabled; wallet creation API-rate-limited.

### §3.2 Turnkey

- **Custodial trust model:** keys generated and used inside **AWS Nitro Enclaves**; remote-attestation pipeline.
- **Policy engine:** JSON-based, two fields per policy — `consensus` (who can authorize) and `condition` (when policy applies). DSL expressions evaluate to bool.
- Recent additions: **Solana Policy Engine** and **TRON Policy Engine** (2025–2026). EIP-712 typed-data parsing supported.
- **Raw signing:** `SIGN_RAW_PAYLOAD` activity signs opaque hex blob — **policy engine bypass risk** if you let agent reach this primitive. Best practice: gate behind `SIGN_TRANSACTION` with parsed condition checks.
- Public incidents: none publicly disclosed as of April 2026.
- **Gotcha:** policies evaluated *before* signing only on parsed primitives; raw payload signing skips them.

### §3.3 Privy

- TEE-based + key sharding, SOC 2.
- Two agent models: (1) developer-owned agent wallet — backend has authorization key; (2) user-owned wallet with delegated permissions to agent.
- **Why bad fit for autonomous home agents:** ~175 ms signing latency (each sign is network call). **Proprietary TEE — not self-hostable.** No native smart-account support. Trust model: must trust Privy's TEE images.

### §3.4 Fireblocks / Fordefi / Copper (MPC custody)

- **Fireblocks:** in-house MPC-CMP, key shares in TEEs. SaaS-only policy engine. Pricing 5-6 figures/yr.
- **Fordefi:** self-custodial MPC, deep DeFi. Server shares in enclaves. SaaS.
- **Copper:** **2-of-3 MPC** (client / Copper / nominated 3rd party). Client shard is plain `.copper` file on local FS — actual key escrow primitive. Custody under English Law Trust.
- **All three:** require multi-employee operational governance, KYC/onboarding, enterprise contracts. **Bad fit for sovereign agents.**

### §3.5 Other agentic economy players (excluding Coinbase)

- **Skyfire:** **KYAPay protocol** — signed JWTs for verified agent identity, "Agent Checkout" thousands tx/day. KYA = Know Your Agent. Visa Intelligent Commerce integration (Dec 2025).
- **Catena Labs (Sean Neville, Circle co-founder):** $18M seed (a16z). **Agent Commerce Kit (ACK)** — open source MIT, repo `github.com/catena-labs/ack`. Two protocols: **ACK-ID** (W3C DIDs + VCs for agent identity) and **ACK-Pay** (payment + receipt + verification, payment-rail-agnostic).
- **Nevermined:** "Payment-for-AI" stack. Agents gate outputs behind paywalls. Now supports real credit-card rails.
- **Payman:** spend management / budget layer; LangChain integration that pauses execution when funds run low.
- **Other protocols:** Google's **Agent Payments Protocol (AP2)**, Stripe + Tempo's **Machine Payments Protocol (MPP)**, Mastercard's "Agent Pay" — all three are direct x402 competitors. Market projection: $135B (2025) → $1.7T (2030).

---

## §4 ERC-4337 + Smart Accounts

### §4.1 EntryPoint addresses (deterministic across chains)

| Version | Address |
|---|---|
| v0.6.0 | `0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789` |
| v0.7.0 | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| v0.8.0 | (per-chain, check `eth-infinitism/account-abstraction` releases) |

**v0.8 changes that matter:**
- UserOpHash now true EIP-712 hash → signatures display structured data in wallets.
- **Native EIP-7702 support** — upgrade EOA to smart account in one tx.
- CREATE/CREATE2 allowed during deployment.
- Unused-gas penalty no longer fires below 40k.

**Common mistake:** Hardcoding v0.6 EntryPoint. v0.7 changed `UserOperation` struct (packed gas fields, `accountGasLimits` is `bytes32`).

### §4.2 Bundlers (2026)

- **Pimlico (`Alto`, TS, ~30 chains)** — de-facto choice for multi-chain. EP6+EP7+EP8.
- Alchemy (Rundler, Rust, EP6/7).
- Stackup (Go).
- Biconomy.
- Etherspot Skandha (first to ship EP0.8).

### §4.3 ERC-7579 vs ERC-6900 (validator standard)

**ERC-7579 has effectively won** (Rhinestone, Biconomy Nexus, ZeroDev Kernel v3, Safe via `Safe7579` adapter, OKX wallet). ERC-6900 still ships but lost mindshare.

**Key repos:**
- `erc7579/erc7579-implementation`
- `rhinestonewtf/modulekit`
- `rhinestonewtf/safe7579`
- `rhinestonewtf/module-sdk`

**Module types per ERC-7579:** Validator (1), Executor (2), Fallback (3), Hook (4).

### §4.4 TEE-attestation Validator pattern

```solidity
// SimplifiedTeeAttestedValidator
contract TeeAttestedValidator {
    mapping(bytes32 => bool) public allowedMrTd;

    function validateUserOp(PackedUserOperation calldata op, bytes32 userOpHash)
        external returns (uint256 validationData)
    {
        (bytes memory quote, bytes memory ecdsaSig) =
            abi.decode(op.signature, (bytes, bytes));

        // 1. Verify TEE quote via Automata DCAP
        (bool ok, bytes memory output) =
            AUTOMATA_DCAP.verifyAndAttestOnChain(quote);
        require(ok, "DCAP fail");

        ParsedQuote memory parsed = parseQuoteOutput(output);

        // 2. Check measurement allowlist
        require(allowedMrTd[parsed.mrTd], "Unknown enclave");

        // 3. Recover signer from ECDSA
        address signer = ECDSA.recover(userOpHash, ecdsaSig);

        // 4. Quote's reportData must commit to that pubkey
        bytes32 expectedReportData = keccak256(abi.encodePacked(signer));
        require(bytes32(parsed.reportData[0:32]) == expectedReportData, "PK mismatch");

        // 5. Pack validity window into validationData
        uint48 validUntil = uint48(parsed.expiry);
        return _packValidationData(false, validUntil, 0);
    }

    function isValidSignatureWithSender(address sender, bytes32 hash, bytes calldata sig)
        external view returns (bytes4) { /* EIP-1271 path */ }

    function onInstall(bytes calldata data) external { /* allowlist */ }
    function onUninstall(bytes calldata data) external { /* clear */ }
    function isModuleType(uint256 typeID) external pure returns (bool) {
        return typeID == 1; // VALIDATOR
    }
}
```

**Common mistake:** Forgetting EIP-1271 path. Many 4337 flows route EIP-1271 through validator; if you only implement `validateUserOp`, you'll break Permit2/CowSwap-style sigs.

### §4.5 Session keys for AI agents

**State of the art (2026):**
- ZeroDev permissions/session keys (Kernel v3 + permissions framework with policies and signers).
- Biconomy `SessionKeyManager`/`SmartSessions` (also a 7579 module by Rhinestone).
- Privy (delegates via ZeroDev under the hood).
- Safe via Zodiac `RolesV2` module.

**Best repo:** `rhinestonewtf/sessions` ("Smart Sessions" — ERC-7579 validator, used in production by Biconomy, Pimlico, ZeroDev).

**Composes:**
- **Policies:** `SpendingLimitPolicy`, `TimeFramePolicy`, `SudoPolicy`, `ERC20SpendingLimitPolicy`.
- **Signers:** ECDSA, WebAuthn, multisig.

**Pattern for our vault:**
```
root key (cold/owner)
  └─ installs Smart Sessions module
      └─ for each agent: enableSession({
           signer: TeeAttestedSigner(enclavePubkey),
           policies: [
             SpendingLimitPolicy(USDC, 100e6 per day),
             AllowedTargetsPolicy([uniRouter, paymentEscrow]),
             ValidUntil(now+7d)
           ]
         })
```

Agent only ever holds session key inside TEE; revocation is one tx.

**Common mistake:** Using 4337 paymaster *and* session key without giving session key permission to call paymaster's helpers — UserOp reverts in validation phase with "AA22 expired or not due."

### §4.6 Permit2 / EIP-2612 patterns

- **Permit2 address (same all chains):** `0x000000000022D473030F116dDEE9F6B43aC78BA3`.
- USDC v1 (Ethereum) does NOT have native EIP-2612; USDC v2.2+ on most L2s does.
- **GO**: For agent vault, use **SignatureTransfer** for one-shot agent payments (no on-chain state, sig is auth). For agent doing many txs, use **AllowanceTransfer** with short expiry (`uint48 expiration`).
- **DON'T**:
  - Use `permit()` directly on USDC L1 — not supported, burns gas.
  - Re-use Permit2 nonces — Permit2 uses *unordered* 256-bit nonces (bitmap), not counter. Generate randomly.
  - Forget Permit2 sig payload includes *spender* — if agent rotates session key, old sig dead (which is what you want).

### §4.7 EIP-712 replay protection for agent messages

- **Domain separator must include all 5 fields** `(name, version, chainId, verifyingContract, salt)`.
- Hash with `EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)` typehash.
- **Cache + recompute on chain-id change:** OpenZeppelin's `EIP712.sol` does this correctly.

**Nonce strategies:**
- Sequential — simple, blocks parallel agent ops.
- **2D nonce like ERC-4337 (`key << 64 | seq`)** — **GO**: lets multiple agents on same vault submit in parallel without head-of-line blocking.
- Bitmap (Permit2-style) — for one-shot vouchers.

**For TEE-attested messages, also bind:**
- `mrTd` / `mrEnclave` (so different enclave version can't replay)
- `keccak(agentPubkey)` in EIP-712 struct
- `expiry` (uint64 timestamp)

**Common mistakes from audits:**
- Signing the *digest* instead of typed-data hash (ECDSA on raw 32-byte digest is malleable, breaks ledger UX).
- `verifyingContract = address(0)` — collides across deployments.
- Reusing same domain separator across chains via proxy — chainId binding is your friend, don't strip it.

### §4.8 Safe modules (2026)

- **Current version:** Safe v1.4.1. Safe v1.5 in audit.
- **Singletons:** `0x41675C099F32341bf84BFc5382aF534df5C7461a` (L2), `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` (L1).
- **Factory `SafeProxyFactory`:** `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67`.
- EIP-7951 / RIP-7212 P-256 signer support landed in `safe-fndn/safe-smart-account`.

**Module pattern for attestation-bound execution:**
- Write Module (not fallback handler) exposing `executeAttested(target, value, data, quote, sig)`.
- Inside: verify `quote` via Automata, check it commits to `keccak(target,data,nonce)`, then call `safe.execTransactionFromModule(...)`.
- Modules bypass owner signatures entirely so TEE attestation *is* the authorization.

**Bridge to 4337:** Safe ships official 4337 module (`safe-modules/4337/Safe4337Module.sol`); for ERC-7579 use `rhinestonewtf/safe7579` adapter.

**DON'T**: implement own 4337 entrypoint hook. Validation-phase storage rules (ERC-7562) easy to violate.

**Common mistake:** Modules can do anything an owner can. Buggy attestation verifier == drained Safe. Add `TimelockModule` or per-token spending cap above attestation module.

### §4.9 ENS subnames for agents

- **NameWrapper (canonical, on L1):** `0xD4416b13d2b3a9aBae7AcD5D6C2BbDBE25686401`.
- Call `setSubnodeOwner(parentNode, label, owner, fuses, expiry)` from vault contract after `setApprovalForAll(vault, true)` from parent owner.
- **Burn fuses (`PARENT_CANNOT_CONTROL | CANNOT_UNWRAP`)** to make subname truly the agent's.
- **Off-chain (CCIP-Read) for free/cheap:** Namestone (managed API, $99/mo), Namespace, JustaName, ENSv2 L2 subnames. Reference: `gskril/ens-offchain-registrar`.
- **Vault pattern:** when minting agent #N, vault calls NameWrapper to mint `agent-N.team.eth` resolving to agent's session-key address, *also* writes text record `tee-attestation` containing EAS UID of enclave attestation.
- **DON'T**: `setSubnodeOwner` without fuses → parent can rugpull subname back. Always burn `PARENT_CANNOT_CONTROL`.

### §4.10 EAS / Verax addresses

| Chain | EAS | SchemaRegistry |
|---|---|---|
| Mainnet | `0xA1207F3BBa224E2c9c3c6D5aF63D0eb1582Ce587` | `0xA7b39296258348C78294F95B872b282326A97BDF` |
| Base | `0x4200000000000000000000000000000000000021` | `0x4200000000000000000000000000000000000020` |
| Optimism | `0x4200000000000000000000000000000000000021` | `0x4200000000000000000000000000000000000020` |
| Arbitrum One | `0xbD75f629A22Dc1ceD33dDA0b68c546A1c035c458` | (lookup) |

**Verax (Linea):** `Consensys/linea-attestation-registry`. Schema is Solidity tuple string, e.g. `"(bytes32 mrTd,address agentPubkey,uint64 validUntil,bytes32 policyHash)"`. Verax has Modules (validators that gate attestation creation) — you can require `TeeQuoteModule` runs Automata DCAP before attestation accepted.

**EAS gotcha:** Both "onchain" and "offchain" attestations exist. Offchain are EIP-712 sigs not stored on-chain. **For trust anchoring, use onchain attestations with `revocable=true`** so you can kill compromised enclaves.

### §4.11 Hackathon prize patterns (recurring 2024–2025)

- EIP-7702 wrappers (BeamPay, Zkipper).
- AI-agent autonomy + spending caps (Coinbase tracks).
- Passkey / WebAuthn smart accounts (RIP-7212, Daimo-style).
- Intent-based AA (Anoma, CoW).
- Session-key marketplaces.

**Differentiator for us:** *Cryptographic* (not just policy) proof that signing happened in TEE — almost no 2024/2025 winner offered this. Closest prior art: Flashbots SUAVE, Phala AI agents, t1 protocol.

**Common judge complaints:** "session keys without revocation UX" and "smart account that's just wrapper around EOA without added security property."

---

## §5 Rust Crate Stack — locked recommendations

### §5.1 Cryptography

| Need | Crate | Version (Apr 2026) | Note |
|---|---|---|---|
| secp256k1 | **`k256`** | 0.13.x | NCC Group audit 2023 (2 high-sev fixed). Pure Rust. Slower than C but reproducible. |
| Alternative secp256k1 | `secp256k1` (rust-bitcoin) | 0.29.x | C FFI, faster, Bitcoin-grade. Use only if reproducibility is OK with C. |
| ed25519 | `ed25519-dalek` | 2.x | RustCrypto. |
| Hashing | `sha2` | 0.10.x | Standard. |
| KDF | `hkdf` | 0.12.x | |
| ChaCha20-Poly1305 | `chacha20poly1305` | 0.10.x | |
| age encryption | `age` | 0.11.2 | Pre-1.0 but production by Mozilla SOPS, FluxCD, Hashicorp. **DON'T**: passphrase work factor default 18; raise for at-rest key. |
| **Canonical JSON (RFC 8785)** | **`serde_json_canonicalizer`** (evik42) | actively maintained | **DON'T**: use `serde_jcs` — abandoned, has UTF-16 / number edge cases. |

### §5.2 HSM / TPM

- **`cryptoki`** (parallaxsecond) v0.10.0 — PKCS#11. Requires vendor `.so` at runtime. Many vendors implement only subset of mechanisms; always probe `get_mechanism_info()`. YubiHSM2 needs `yubihsm-shell` PKCS#11 module + connector daemon. Nitrokey HSM 2 works via OpenSC's `opensc-pkcs11.so`.
- **`tss-esapi`** (parallaxsecond) v7.6.0 — TPM 2.0 ESAPI. Requires `libtss2-dev`/`tpm2-tss-devel` ≥ 3.2 at build. **Heavy ergonomics**: sealing to PCRs requires `PolicyPCR` → `TrialSession` → digest → `Create` with `policy_digest`. **Common mistake:** sealing under SRK without persistent handle survives reboot, but PCR values reset → **always pick stable PCR set (typically PCR 7 for SecureBoot state, NOT PCR 0/2 which change on firmware update).** Authorization sessions not `Send`; use one `Context` per thread.

### §5.3 Policy engine

- **`regorus`** (Microsoft) ~0.4.x — pure-Rust embedded Rego interpreter. **Not 100% OPA-compatible** — passes OPA v1.2 test-suite barring some builtins (notably `http.send`, some crypto/graph). Tree-walking interpreter — slower than OPA's Wasm target on hot policies. **GO**: every builtin gated by cargo feature so you can shrink TCB. **Production user:** Azure Container Instances confidential pod policy.
- **Alternatives:** `wasmtime` + OPA Wasm output (faster, larger TCB); **Cedar** (`cedar-policy` crate) if you can rewrite policies in Cedar — faster and formally analyzed but different language.

### §5.4 Ethereum stack

- **`alloy`** v1.7.3 (Apr 2026); `alloy-core` 1.5.2. Successor to deprecated ethers-rs. Used by Foundry, Reth, Revm, SP1 zkVM.
- **Stable surface:** `alloy-primitives` (U256/B256/Address), `alloy-sol-types` (sol! macro), `alloy-rpc-types-eth`, `alloy-provider`, `alloy-signer-local`, `alloy-network`. v1.0 (May 2025) declared API stability; minor versions additive.
- **Gotcha:** Crate explosion — 50+ sub-crates. **GO**: umbrella `alloy = "1"` with feature flags.
- `alloy-signer-aws`/`alloy-signer-gcp` for KMS; `alloy-signer-ledger`, `alloy-signer-trezor`. **No first-class PKCS#11 signer** — write thin `Signer` impl wrapping `cryptoki`.

### §5.5 Storage

- **`rusqlite`** — synchronous, mature, thinnest layer over libsqlite3. **GO** for security-critical local vault. Use `Connection::prepare_cached`. `OpenFlags::SQLITE_OPEN_CREATE | SQLITE_OPEN_READ_WRITE | SQLITE_OPEN_FULL_MUTEX`.
- **`sqlx`** — async, compile-time checked queries. **DON'T**: write transactions can deadlock/starve under SQLite + WAL ("Write Transactions are a Footgun" — emschwartz blog). Does NOT set journal mode by default.
- **`diesel`** — sync ORM, strongest type safety. Heavy macro overhead; overkill for vault schema.
- **WAL setup (mandatory):** `PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;`. Open one writer connection + N readers.
- **Encryption:** `bundled-sqlcipher` feature OR age-encrypted DB file OR filesystem-level FDE.

### §5.6 Numbers

- **`rust_decimal`** — 128-bit fixed (96-bit mantissa, 28 decimals max). Fast, no allocation, perfect for USD (2-4 decimals). **GO**. Use `Decimal::from_str_exact` to forbid silent rounding.
- **Avoid `bigdecimal`** unless you need precision >28 digits.
- For token amounts on Ethereum: use `alloy_primitives::U256`. Convert to `rust_decimal` only at USD boundary.
- **Newer:** `fastnum` — fixed-size `D128`/`D256`, faster. Worth evaluating but younger.

### §5.7 Transport

- **`tonic`** v0.14.5 (Feb 2026). 0.14 migrated to **hyper 1.0** + prost 0.14; prost separate crate (`tonic-prost`). TLS via features `tls-ring` or `tls-aws-lc`. **GO**: mTLS + Unix socket (`tonic` supports UDS via `tower::service_fn`).

### §5.8 Observability

**Standard 2026 stack:** `tracing` 0.1.x, `tracing-subscriber` 0.3.x, `tracing-opentelemetry` 0.27+, `opentelemetry` 0.26+, `opentelemetry_sdk` 0.26+, `opentelemetry-otlp` 0.26+ (OTLP/gRPC over tonic).

**Pattern:** `Registry::default().with(EnvFilter).with(fmt::layer()).with(OpenTelemetryLayer::new(tracer))`.

**DON'T**: version skew across `opentelemetry-*` crates is the #1 footgun — pin all four to same minor. **Always call `global::shutdown_tracer_provider()` on shutdown** or you'll lose buffered spans. **Scrub PII/keys before they hit spans** — custom `MakeWriter` or `Layer` filtering fields named `secret`, `key`, `passphrase`.

### §5.9 Testing

- **`cargo-fuzz`** — wrapper over libFuzzer; nightly + LLVM sanitizers. **GO** for in-tree per-format fuzz targets (one `fuzz_target!` per parser: APRP message, x402 payment header, JCS roundtrip).
- **`honggfuzz-rs`** — Google's honggfuzz; works on stable, supports persistent-mode + feedback-driven mutation. Good for long-running CI fuzz nodes.
- **`cargo-mutants`** — mutation testing. **High value for policy engine:** policy decision functions (allow/deny, threshold checks) are exactly where missing `!` or swapped comparison wouldn't fail tests but would break security. Use `--re 'policy::|verify::'` to scope; shard with `--shard k/N` in CI. Slow — restrict to security-critical, run weekly not per-PR.

### §5.10 Secret zeroization

**Stack:** `zeroize` (volatile-write + atomic fence; no FFI) + `secrecy` (`SecretBox<T>`, `Drop` calls zeroize, debug `[REDACTED]`) + `region::lock` on key buffers + `prctl(PR_SET_DUMPABLE, 0)` + systemd `MemoryDenyWriteExecute=yes`, `LockPersonality=yes`, `RLIMIT_MEMLOCK` raised.

**For production: disable swap entirely** on vault host — defense in depth beats per-allocation locking.

**Common mistakes that defeat zeroization:**
- `String`/`Vec` reallocation: pushing past capacity copies bytes, original buffer not zeroed. **Pre-allocate with `with_capacity`, never `push` past it.**
- `Clone` on a secret: copies escape. Use `#[derive(Zeroize)]` and never `Clone`.
- Stack copies via pass-by-value: pass `&Secret<T>`.
- `format!`, `Debug` impls leak; always wrap in `SecretBox`.
- Fork without pre-zero (CoW pages survive in child).
- Swap to disk: requires `mlock` *and* disable swap or use encrypted swap.
- LLVM optimizer eliding writes: `zeroize` uses `write_volatile` + fence to prevent. **DON'T** roll own with `memset`.

### §5.11 Build & release

**Reproducible Rust builds (recipe 2026):**
```bash
SOURCE_DATE_EPOCH=$(git log -1 --pretty=%ct)
RUSTFLAGS="--remap-path-prefix=$HOME=/build --remap-path-prefix=$PWD=/src"
cargo build --release --locked --frozen --offline
```
- Pin via `Cargo.lock`, commit it. `[profile.release] codegen-units = 1, lto = "fat"`.
- Vendor: `cargo vendor-filterer` (reproducible tarballs).
- Build inside pinned container (digest-pinned `rust:1.83-slim@sha256:...`).
- Verify with `diffoscope` between two independent builds; rebuilderd (`reproduce.debian.net` builds amd64/arm64/riscv64 as of Jan 2025).

**Sigstore / cosign / SLSA L3:**
- **PIN**: cosign v3 (mandatory `--bundle` flag).
- ```
  cosign sign-blob --bundle mandate.bundle ./target/release/mandate
  cosign verify-blob --bundle mandate.bundle \
    --certificate-identity 'https://github.com/org/mandate/.github/workflows/release.yml@refs/tags/v*' \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com
  ```
- **SLSA v1.0 Build L3 on GitHub Actions** requires: (1) reusable workflow (isolation between caller and builder); (2) `actions/attest-build-provenance` (signed in-toto provenance via Sigstore Fulcio); (3) ephemeral runner; (4) network-restricted/hermetic build step. Use `slsa-framework/slsa-github-generator`.
- **For binary signing today:** call `cosign` CLI from build script. **DON'T** use `sigstore-rs` for signing — API unstable. **GO** for in-process *verification* of downloaded updates only.

### §5.12 systemd integration

- **`sd-notify`** — pure-Rust, zero deps. **GO**: `READY=1`, `STATUS=...`, `WATCHDOG=1`.
- **`libsystemd`** — pure-Rust. Socket activation (`receive_descriptors()`), journal logging, machined, credentials decoding (LoadCredentialEncrypted via `systemd-creds`).
- **DON'T**: use `systemd` (jmesmon) — FFI to libsystemd.so, hurts reproducibility.

**Recommended pattern:**
- `Type=notify`, `NotifyAccess=main`, `WatchdogSec=30s` → `sd-notify`.
- `ListenStream=/run/mandate/mandate.sock` + `Sockets=mandate.socket` → `libsystemd::activation::receive_descriptors(true)` → hand FDs to tonic via `UnixListener::from_std`.
- Passphrase: `LoadCredentialEncrypted=mandate-pass:/etc/credstore.encrypted/mandate-pass` → read `$CREDENTIALS_DIRECTORY/mandate-pass` (kernel TPM key unwraps; no decryption code in binary).

---

## §6 Linux Hardening

### §6.1 systemd unit hardening directives — full compendium

Target: `systemd-analyze security mandate.service` score < 1.0.

| Directive | Recommended Value | Why |
|---|---|---|
| `NoNewPrivileges=` | `yes` | Blocks setuid/fcaps gain |
| `ProtectSystem=` | `strict` | Read-only `/usr`, `/boot`, `/etc` |
| `ProtectHome=` | `yes` | Hide `/home`, `/root`, `/run/user` |
| `PrivateTmp=` | `yes` | Per-unit `/tmp` namespace |
| `PrivateDevices=` | **`no`** + `DevicePolicy=closed` + `DeviceAllow=` | **DON'T** `yes` — silently breaks PKCS#11/USB |
| `PrivateNetwork=` | `no` (need RPC) + `RestrictAddressFamilies=` + `IPAddressAllow=` | |
| `CapabilityBoundingSet=` | `` (empty — drop everything) | |
| `AmbientCapabilities=` | `` (empty) | |
| `SystemCallFilter=` | `@system-service` then `~@privileged @resources @mount @swap @reboot @debug @cpu-emulation @obsolete @raw-io @module` | **WATCH**: wrong filter kills service silently — test with `journalctl -u` |
| `SystemCallArchitectures=` | `native` | Block 32-bit/x32 |
| `ProtectKernelTunables=` | `yes` | RO `/proc/sys`, `/sys` |
| `ProtectKernelModules=` | `yes` | Block `init_module` |
| `ProtectKernelLogs=` | `yes` | Block `/dev/kmsg`, syslog syscall |
| `ProtectControlGroups=` | `yes` | RO `/sys/fs/cgroup` |
| `ProtectClock=` | `yes` | Block `settimeofday` |
| `ProtectHostname=` | `yes` | UTS namespace |
| `ProtectProc=` | `invisible` | Hide other PIDs |
| `ProcSubset=` | `pid` | Show only PID dirs |
| `RestrictNamespaces=` | `yes` | Deny `clone()` new ns |
| `RestrictRealtime=` | `yes` | Block `SCHED_FIFO/RR` |
| `RestrictSUIDSGID=` | `yes` | Block setuid bit |
| `LockPersonality=` | `yes` | Defeats ASLR-bypass tricks |
| `MemoryDenyWriteExecute=` | `yes` | No JIT in Rust |
| `RestrictAddressFamilies=` | `AF_UNIX AF_INET AF_INET6` | Strips `AF_NETLINK`, `AF_PACKET` |
| `IPAddressAllow=`/`Deny=` | Allow only RPC peers; `IPAddressDeny=any` first | eBPF L4 firewall |
| `User=`/`Group=` | `mandate:mandate` (uid < 1000, system) | |
| `UMask=` | `0077` | |
| `KeyringMode=` | `private` | |
| `RemoveIPC=` | `yes` | Drop SysV/POSIX IPC on stop |
| `DevicePolicy=` | `closed` | cgroup device whitelist |
| `DeviceAllow=` | `/dev/bus/usb/00X/00Y rw` (HSM) plus `/dev/tpmrm0 rw` if used | |
| `BindReadOnlyPaths=` | `/etc/mandate` | |
| `ReadWritePaths=` | `/var/lib/mandate /var/log/mandate` | |

### §6.2 AppArmor profile pattern

```
include <tunables/global>
profile mandate /usr/bin/mandate {
  include <abstractions/base>
  include <abstractions/nameservice>
  include <abstractions/openssl>

  capability,
  deny capability sys_module,
  deny capability sys_rawio,
  deny capability sys_ptrace,

  /usr/bin/mandate       mr,
  /etc/mandate/**        r,
  owner /var/lib/mandate/**  rwk,
  /var/log/mandate/**    w,
  /run/mandate/**        rwk,

  # PKCS#11 module
  /usr/lib/pkcs11/*.so       mr,
  /usr/lib/x86_64-linux-gnu/pkcs11/*.so mr,
  /etc/pkcs11/**             r,
  owner /run/user/*/p11-kit/** rw,

  # USB HSM (hidraw + raw bus)
  /dev/bus/usb/[0-9]*/[0-9]* rw,
  /sys/bus/usb/devices/**    r,
  /dev/hidraw[0-9]*          rw,

  # Network: outbound only to RPC
  network inet  stream,
  network inet6 stream,
  network unix  stream,
  deny network raw,
  deny network packet,

  deny /proc/*/mem   rwx,
  deny /proc/sys/kernel/** w,
  deny @{PROC}/sysrq-trigger w,
}
```

Notes: `owner` qualifier restricts to euid match (defense vs hardlink games); `audit deny` logs denials; use `aa-logprof` to refine after running in `complain` mode. Ubuntu 24.04+ uses `abi <abi/4.0>` header.

### §6.3 udev rules for HSM USB

`/etc/udev/rules.d/70-mandate-hsm.rules`:
```
# YubiHSM 2 (vid 1050, pid 0030)
SUBSYSTEM=="usb", ATTRS{idVendor}=="1050", ATTRS{idProduct}=="0030", \
  TAG+="uaccess", GROUP="mandate", MODE="0660"

# Nitrokey HSM 2 (vid 20a0, pid 4230)
SUBSYSTEM=="usb", ATTRS{idVendor}=="20a0", ATTRS{idProduct}=="4230", \
  GROUP="mandate", MODE="0660"

# hidraw children
KERNEL=="hidraw*", ATTRS{idVendor}=="1050", GROUP="mandate", MODE="0660"
```

Reload: `udevadm control --reload && udevadm trigger`. **DON'T**: `MODE="0666"` (world-writable). **DON'T**: `TAG+="uaccess"` alone for daemon — grants only to logged-in seat users.

### §6.4 LUKS2 + TPM2 unattended unlock

```bash
cryptsetup luksFormat --type luks2 --pbkdf argon2id /dev/nvme0n1p3
systemd-cryptenroll --tpm2-device=auto --tpm2-pcrs=7+11+14 --tpm2-with-pin=yes /dev/nvme0n1p3
```

PCR 7 = Secure Boot state, PCR 11 = unified kernel image, PCR 14 = MOK. **DON'T**: PCR 0 (firmware updates rebrick).

`/etc/crypttab`:
```
vault UUID=... none tpm2-device=auto,tpm2-pcrs=7+11+14,headless
```

Mount options: `/var/lib/mandate ext4 defaults,nodev,nosuid,noexec,noatime 0 2`. Plus optional `fs-verity` on binary.

---

## §7 Prompt Injection Defenses

### §7.1 OWASP LLM Top 10 (2025) — vault-relevant

- **LLM01 Prompt Injection** — direct + indirect. #1 risk.
- **LLM06 Excessive Agency** — financial-loss vector.
- **LLM07 System Prompt Leakage** (new in 2025).
- **LLM10 Unbounded Consumption**.

### §7.2 Defense layers (no single one reliable)

1. **Privilege isolation (most important).** LLM never holds signing keys; submits signing *request* to daemon, daemon independently re-validates intent against policy. **Only architectural defense that survives successful injection.**
2. **Structured outputs / function-calling schemas.** Reject any output not matching strict JSON schema for `{action, recipient, amount, asset, rationale}`. Validators in Rust: `jsonschema`, `valico`.
3. **Spotlighting (Microsoft Research, Hines et al. 2024).** Three modes: *delimiting*, *datamarking* (insert per-request token between every word of untrusted text), *encoding* (base64/ROT13). Encoding drops attack success rate to ~0% on summarization but adds tokens.
4. **Intent classification before action.** Small classifier (or LLM-as-judge) verifies proposed signed transaction matches natural-language intent before daemon signs.
5. **Canary tokens** in system prompts. If canary appears in any output or downstream request, prompt has leaked → alert + revoke. Used by Rebuff (`github.com/protectai/rebuff`).
6. **LLM-as-judge / Constitutional Classifiers.** Anthropic's constitutional classifiers (2025) trained on synthetic data from rule "constitution"; defended against universal jailbreaks in $15k bounty (no break in 2 months). Run separate judge model on every (input, proposed action) pair.
7. **Tool-use safety / RL-hardened models.** Anthropic reports Claude Opus 4.5 ~1.4% attack success rate vs 10.8% for Sonnet 4.5 on browser-agent injections — pick a hardened model.
8. **External guardrails:** NeMo Guardrails (NVIDIA, Colang DSL — only one with multi-turn dialogue control), Lakera Guard (managed), Rebuff (canary + vector store of past attacks), NVIDIA Garak (37+ probe modules — use as CI fuzzer, not runtime), Protect AI's `llm-guard`.
9. **Spend caps and on-chain co-signers.** Daemon enforces per-time-window value limits independent of LLM. Above threshold → require human or HSM-button approval.
10. **Adversarial CI.** Run Garak against every model/prompt change; track attack-success-rate as release gate.

---

## §8 MCP for Payment Tools

- **x402-mcp (Vercel):** `vercel.com/blog/introducing-x402-mcp`. Light wrapper around `mcp-handler` that adds `paidTools` — tool definition with `price` field.
- **Coinbase MCP server:** `docs.cdp.coinbase.com/x402/mcp-server` and mirror at `x402.gitbook.io/x402/guides/mcp-server-with-x402`.
- **Zuplo / GPU-Bridge:** `github.com/gpu-bridge/mcp-server` exposes 30 AI services as MCP tools, x402-native.

**Pattern for agent → vault MCP:** vault exposes MCP server with tools like `pay_invoice(invoice_id, max_amount, recipient_allowlist_id)`, `quote_payment`, `get_balance`. Agent's MCP client calls vault's tools; vault enforces policy + attestation locally, returns settlement receipt. **x402 `PaymentRequired` blob is natural input/output schema** since v2 puts everything in headers/JSON.

**Gotcha:** MCP doesn't have native "interactive auth approval" primitive — for human-in-the-loop confirms, implement out-of-band approval (push to phone) or design tool to return "pending" + require re-poll.

---

## §9 Audit Log Shipping

**Topology:** `mandate → journald → rsyslog → TLS → central SIEM`. journald structured/binary; rsyslog handles transport.

**Audit logs NOT captured by rsyslog default** — use `imfile` to tail `/var/log/audit/audit.log`, or `audisp-syslog` plugin.

**Integrity controls:**
- **TLS with mutual auth:** `module(load="omrelp")` or `module(load="imtcp" StreamDriver.Mode="1" StreamDriver.AuthMode="x509/name")`. **GO**: RELP over plain TCP — RELP gives application-level acks (no message loss on TCP RST).
- **Disk-assisted action queue** so logs survive SIEM outages: `action(... queue.type="LinkedList" queue.fileName="vault" queue.maxDiskSpace="1g" queue.saveOnShutdown="on")`.
- **Forward-secure sealing** on journald: `journalctl --setup-keys` then ship `.journal` files to write-only sink.
- **Hash chain at rest:** `aide` over rotated audit logs, or journald FSS.

**Common mistakes:**
- Shipping logs over plain TCP/UDP.
- Storing local logs under same uid as daemon (compromise erases evidence — use `setfacl` so daemon can append but not modify).
- Not enabling `space_left_action = SYSLOG` in `auditd.conf` (full disk silently drops events — better is `space_left_action = HALT` for a vault).

---

## §10 References — quick directory

### TEE
- Automata DCAP attestation — https://github.com/automata-network/automata-dcap-attestation
- Automata DCAP v1.1 release — https://blog.ata.network/automatas-release-of-dcap-attestation-v1-1-for-agentic-systems-84ae98900370
- Phala dcap-qvl — https://github.com/Phala-Network/dcap-qvl
- Phala dstack — https://github.com/Dstack-TEE/dstack
- t16z TEE Attestation Explorer — https://proof.t16z.com/
- Marlin Oyster on-chain Nitro — https://blog.marlin.org/on-chain-verification-of-aws-nitro-enclave-attestations
- Intel TDX docs — https://docs.kernel.org/arch/x86/tdx.html
- AMD SEV-SNP whitepaper — https://www.amd.com/content/dam/amd/en/documents/developer/lss-snp-attestation.pdf

### x402
- coinbase/x402 — https://github.com/coinbase/x402
- x402 v2 launch — https://www.x402.org/writing/x402-v2-launch
- x402 network support — https://docs.cdp.coinbase.com/x402/network-support
- L402 spec — https://github.com/lightninglabs/L402/blob/master/protocol-specification.md

### ERC-4337 / Smart Accounts
- ERC-7579 — https://eips.ethereum.org/EIPS/eip-7579
- Rhinestone modulekit — https://github.com/rhinestonewtf/modulekit
- Safe7579 — https://github.com/rhinestonewtf/safe7579
- Smart Sessions — https://github.com/rhinestonewtf/sessions
- Safe modules — https://docs.safe.global/advanced/smart-account-modules
- Permit2 — https://github.com/Uniswap/permit2

### Rust crates
- alloy — https://docs.rs/alloy/latest/alloy/
- regorus — https://github.com/microsoft/regorus
- cryptoki — https://github.com/parallaxsecond/rust-cryptoki
- tss-esapi — https://github.com/parallaxsecond/rust-tss-esapi
- serde_json_canonicalizer — https://crates.io/crates/serde_json_canonicalizer

### Linux hardening
- systemd-analyze security — `man systemd-analyze`
- AppArmor — https://wiki.archlinux.org/title/AppArmor
- TPM2 + LUKS — https://gierdo.astounding.technology/blog/2025/07/05/tpm2-luks-systemd
- 0pointer.net LUKS2 unlock — https://0pointer.net/blog/unlocking-luks2-volumes-with-tpm2-fido2-pkcs11-security-hardware-on-systemd-248.html

### Prompt injection
- OWASP LLM Top 10 v2025 PDF — https://owasp.org/www-project-top-10-for-large-language-model-applications/assets/PDF/OWASP-Top-10-for-LLMs-v2025.pdf
- Microsoft Spotlighting paper — https://arxiv.org/html/2403.14720v1
- Anthropic Constitutional Classifiers — https://www.anthropic.com/research/constitutional-classifiers
- Rebuff — https://github.com/protectai/rebuff
- NeMo Guardrails — https://github.com/NVIDIA/NeMo-Guardrails
- NVIDIA Garak — https://github.com/leondz/garak

### Reproducible builds & signing
- SOURCE_DATE_EPOCH — https://reproducible-builds.org/docs/source-date-epoch/
- cosign — https://github.com/sigstore/cosign
- SLSA — https://slsa.dev
- slsa-github-generator — https://github.com/slsa-framework/slsa-github-generator

---

## §11 Decision points — locked

| Decision | Choice | Date | Source |
|---|---|---|---|
| Implementation language | Rust | 2026-04-25 | Memory safety + reproducibility |
| Policy DSL | Rego via `regorus` | 2026-04-25 | Embedded, audited, gated builtins |
| JSON canonicalization | `serde_json_canonicalizer` | 2026-04-25 | `serde_jcs` abandoned |
| secp256k1 | `k256` | 2026-04-25 | Audited, pure Rust, reproducible |
| Storage | `rusqlite` + WAL | 2026-04-25 | Single-writer pattern, predictable |
| Money type | `rust_decimal` | 2026-04-25 | 28 decimal places, fast |
| Ethereum stack | `alloy` v1.x | 2026-04-25 | Successor to ethers-rs |
| TEE platform (primary) | Intel TDX | 2026-04-25 | Better gas costs on-chain, growing HW availability |
| TEE platform (secondary) | AMD SEV-SNP | 2026-04-25 | Enterprise + Hetzner cloud |
| HSM (primary) | Nitrokey HSM 2 | 2026-04-25 | Open source firmware, ~€100 |
| HSM (secondary) | YubiHSM 2 | 2026-04-25 | Better attestation, $650 |
| TPM library | `tss-esapi` | 2026-04-25 | Only mature option |
| PKCS#11 library | `cryptoki` | 2026-04-25 | Idiomatic, used by Parsec |
| Payment protocol | x402 v2 (primary), l402 (secondary) | 2026-04-25 | Linux Foundation governance |
| Smart account standard | ERC-7579 | 2026-04-25 | Won over ERC-6900 |
| On-chain DCAP verifier | Automata DCAP v1.1 | 2026-04-25 | Audited, multi-chain |
| Chain (primary) | Base | 2026-04-25 | Coinbase x402 native |
| Chain (secondary) | Polygon, Arbitrum | 2026-04-25 | Cheap, RIP-7212 supported |
| Bundler (4337) | Pimlico Alto | 2026-04-25 | Best multi-chain coverage |
| Audit anchoring chain | Base | 2026-04-25 | Cheap, fast |
| ENS subname provider | Namestone (paid) or self-hosted CCIP-Read | 2026-04-25 | Both viable |
| Reproducible build verification | rebuilderd + diffoscope | 2026-04-25 | Industry standard |
| Code signing | cosign v3 + SLSA L3 | 2026-04-25 | Sigstore + GitHub Actions |
| Linux base | Ubuntu 24.04 LTS | 2026-04-25 | Long support, AppArmor default |
| **Public brand** | **Mandate** | 2026-04-27 | Hackathon/submission name. Stronger category frame: agents do not get wallets; they get spending mandates. Tagline: "Spending mandates for autonomous agents." |
| **Technical namespace** | **`mandate`** | 2026-04-27 | Same as public brand for daemon/crates/schema IDs/paths/CLI. Deep naming research: pôvodný "Agent Vault OS" mal hard blockers (`cloudweaver/agentvault`, `Infisical/agent-vault`, ThoughtMachine VaultOS). |

---

## §12 Open issues / WATCH list

- **TDX 2.0 rollout** — when `TDG.MR.KEY.GET` becomes broadly available, native sealing simplifies our key management significantly.
- **EIP-7702 maturity on Base** — affects M3 user onboarding flow.
- **Rekor v2 transition** — Sigstore transparency log; older sigstore-rs versions break when v1 decommissioned.
- **dstack hardening fix (Jan 2026)** — must be on post-Saxena disclosure version.
- **TEE.fail mitigation status** — physical attack class; vault deployment guides must be explicit about physical security assumptions for sovereign hosting.
- **Cargo 1.93 reproducibility improvements** — track upstream.
- **rustc bit-for-bit reproducibility** — active GSoC '25 work; track for Rust 1.84+ status.
