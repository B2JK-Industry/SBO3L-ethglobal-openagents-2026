# Implementation Safeguards (Anti-Bug Compendium)

> **Účel:** Kompendium *implementačných hrabiek*, ktoré sa pri tomto projekte ľahko prešliapne. Pred štartom akéhokoľvek slotu (per `18_agent_assignment.md`) si tu agent prečíta relevantnú sekciu.
>
> **Filozofia:** Každá položka je *konkrétna chyba*, ktorú treba neurobiť. Žiadne abstraktné "best practices" — iba konkrétne mistakes + konkrétne fixes.

---

## §0 Universal safeguards (platí pre všetky sloty)

### U-01 — Pred začatím: prečítaj contracts FIRST
Než spustíš editor, prečítaj:
- `17_interface_contracts.md` — *celé*. Bez výnimky.
- `19_knowledge_base.md` — sekcie pre tvoj scope.
- `16_demo_acceptance.md` — D-PN-* pre tvoju story.

Čas strávený čítaním < čas strávený debugovaním cross-module konfliktov.

### U-02 — Žiadne `unwrap()` v src/, žiadne `println!`
- Použi `?` operator alebo explicit `.expect("invariant: <reason>")` s rationale.
- Použi `tracing::*` macros, nie `println!`/`eprintln!`.
- CI clippy lint blokuje porušenia.

### U-03 — Žiadny float pre money / amount / cap
Akýkoľvek `f32` / `f64` v code, ktorý sa dotýka `amount` / `cap_usd` / `value` = bug. Použi `rust_decimal::Decimal`.

### U-04 — Wire types musia mať `#[serde(deny_unknown_fields)]`
Akýkoľvek struct deserializovaný z external input (request body, x402 challenge, decision token) MUSÍ deny unknown fields. Inak útočník môže pridať invisible polia.

### U-05 — Logy nikdy neobsahujú raw payload bytes alebo key material
Pred `tracing::info!(...)` payloadu, **hashuj** ho (sha256 hex prefix). CI test `D-P0-03` to verifikuje.

### U-06 — Žiadne `BEGIN DEFERRED` v SQLite write transactions
Pre budget, audit, policy mutations — vždy `BEGIN IMMEDIATE`. Inak race condition pri concurrent writes.

### U-07 — Žiadne tokio task bez mena
Použi `tokio::task::Builder::new().name("vault.<scope>.<task>").spawn(fut)`. Bez mena nesleduješ task v dumps.

### U-08 — Test má `is_test: true` v config
Žiadny dev key v test environment bez explicit `is_test: true` flagu — defense proti accident production deployment.

### U-09 — Cross-restart test pre persistent state
Akýkoľvek state, ktorý ukladáš (nonce, budget, audit, attestation cache) — test musí stop+start vault a overiť, že state survived correctly.

### U-10 — Žiadny network call v `Drop` impl
Drop nemôže byť async; networking v drop = blok / panic. Resource cleanup robí explicit `.shutdown().await`.

---

## §1 Phase 0 — Foundations safeguards

### P0-01 — Repo bootstrap
- **Bug:** `Cargo.toml` `[workspace]` bez `resolver = "2"` → cargo používa starú resolution, nesplít features. Always set `resolver = "2"` v workspace root.
- **Bug:** `rust-toolchain.toml` bez `profile = "minimal"` → CI sťahuje rust-docs zbytočne, spomaľuje. Add `profile = "minimal"`.
- **Bug:** GitHub Actions `actions/checkout@v3` (deprecated) — use v4 minimum.
- **Bug:** `cargo audit` bez `--ignore RUSTSEC-...` whitelist → false positives na unused features blokujú CI. Documentuj allowlist.

### P0-03 — Telemetry
- **Bug:** `tracing-subscriber::fmt().init()` bez `with_writer(io::stderr)` — logy idú do stdout, miešajú sa s gRPC binary output. Send logs to stderr.
- **Bug:** OpenTelemetry version skew: `tracing-opentelemetry` vyžaduje **rovnaký minor** ako `opentelemetry`/`opentelemetry_sdk`/`opentelemetry-otlp`. CI fails on Cargo.lock conflict.
- **Bug:** Zabudnúť `global::shutdown_tracer_provider()` v `main` → 5-second buffer of spans lost on exit.
- **Bug:** Sensitive field v span (e.g. `tracing::info!(passphrase = %p, ...)`). Solution: custom `Layer` ktorý filtruje by field name (`passphrase`, `key`, `secret`).

### P0-04 — Error catalog
- **Bug:** `#[from]` `serde_json::Error` to top-level Error → leaks parsed-but-malformed input v error message. Wrap to `SchemaError::WrongType` bez raw input.
- **Bug:** `Display` impl for Error obsahuje raw input. Solution: `Debug` keep details, `Display` keep brief.
- **Bug:** `error_code` mismatch between Rust enum and `17_interface_contracts.md §3.1` table → test rejects. CI test enumerates and compares.

### P0-05 — Config
- **Bug:** TOML deserialization pri missing required field bez clear error → user sees "invalid config" without saying which key. Use `serde::Deserialize` with `#[serde(default)]` + explicit validation step.
- **Bug:** Env override `MANDATE__SECTION__KEY` collides with vendor envs. Use longer prefix `MANDATE__` with double underscores (well-known by `config` crate).
- **Bug:** Validate config at load AND on reload (SIGHUP) — production-lint must run both times.

---

## §2 Phase 1 — Happy-path safeguards

### P1 — APRP (E2-S1, S2)
- **Bug:** `serde_jcs` deprecated (per CONF-01). **Use `serde_json_canonicalizer`.**
- **Bug:** ULID regex permissive (per CONF-02). Use Crockford base32 strict regex.
- **Bug:** `chrono::DateTime<Utc>` parsing accepts non-RFC3339 strings (e.g. "2026-04-25 10:30:42"). Use `time::OffsetDateTime::parse(s, &format_description::well_known::Rfc3339)`.
- **Bug:** Schema validator allows `expiry` 100 years in future → accept everything. Cap at `now + 10 minutes`.
- **Bug:** `Money.value` parsed as `Decimal::from_str` without `_exact` → silent rounding of `"0.123456789012345678901234567890"` (>28 digits). Use `from_str_exact` to reject.
- **Bug:** `nonce` uniqueness check is per-process (HashMap). Survives restart? Per `D-P2-18` test must pass. Use persistent SQLite table.
- **Bug:** Schema `additionalProperties: false` not on nested objects. Recursively check every nested struct.
- **Bug:** Fuzz target generates valid request via `Arbitrary`, not mutation — no real fuzzing benefit. Use `arbitrary` for structure-aware AND raw `bytes` mutation.

### P1 — Gateway (E3-S1, S2, S4)
- **Bug:** Unix socket `bind()` after server already running → stale socket file blocks. Use `if path.exists() { fs::remove_file(path)?; }` pre-bind.
- **Bug:** Unix socket SO_PEERCRED — extracts client UID/GID, but mTLS layer needs to override (peer creds = whatever process; cert says who).
- **Bug:** TCP listener binds to `0.0.0.0` despite config saying `127.0.0.1` → typo in `parse::<SocketAddr>` doesn't validate. Add explicit assertion.
- **Bug:** mTLS server cert = self-signed per-deploy → agent SDK can't verify without explicit pin. Either ship CA cert with agent provisioning OR use SPKI pinning.
- **Bug:** Agent CN includes whitespace / special chars → `agent_id` injection. Restrict `agent_id` to `^[a-z0-9][a-z0-9_-]{2,63}$` everywhere.
- **Bug:** Cert revocation list (CRL) reload on file change — but if `inotify` misses event (busy filesystem), revoked agent persists. Add periodic re-read every 60s.
- **Bug:** Rate limit token bucket per agent shared across vault restarts? No — burst tolerance fails. Document: token bucket is in-memory, restarts reset.

### P1 — Signing (E8-S1, S5)
- **Bug:** `local_dev_key` ephemeral key generated each startup → audit log references key id "X" but key X doesn't exist after restart. Either persist key (test mode) or refuse to start with persisted refs.
- **Bug:** age decryption: passphrase loaded into `String`, not zeroized. Use `secrecy::SecretString` with explicit `.expose_secret()`.
- **Bug:** Recovered ECDSA signature `v` value: 27/28 vs 0/1 — Ethereum expects 27/28. `k256::Signature::recoverable` returns 0/1 — add 27.
- **Bug:** Sign produces 64-byte signature without recovery byte — Ethereum needs 65 bytes (r || s || v). Append `v` correctly.
- **Bug:** Decision token signature scheme switches between Ed25519 (prod) and HMAC (dev) — but signer doesn't know which → reject. Make scheme part of token format, version-tagged.
- **Bug:** Decision token replay: same token re-used in N parallel sign requests within TTL. Add nonce store at signer side too.
- **Bug:** `mlock` fails silently on small RLIMIT_MEMLOCK → keys swap to disk. Check return value, log warning, prefer fail-closed.

### P1 — Mock x402 (E16-S0)
- **Bug:** Mock returns deterministic challenge → replay attack tests pass trivially. Add random nonce in challenge.
- **Bug:** Mock doesn't validate signature beyond presence → tests don't catch sig bugs. Verify signature properly using same EIP-3009 scheme.
- **Bug:** Mock TLS cert regenerated on every startup → cert pin tests fail. Use stable test cert from fixtures.

---

## §3 Phase 2 — Policy + Budget safeguards

### P2 — Policy engine (E4-S1)
- **Bug:** Rego compiler emits `time.now_ns()` builtin → non-deterministic eval. Whitelist allowed builtins explicitly; reject compile if disallowed.
- **Bug:** YAML order matters for hash, but YAML has multiple equivalent encodings. Canonicalize via `serde_yaml::to_value` → `serde_json_canonicalizer::to_vec` → hash.
- **Bug:** Empty `allowed.providers` → silently allows everything (no rules to deny). Fail-closed: empty allowlist = deny all.
- **Bug:** Policy version int overflow — use `u32` with rollover detection rather than `i64` silent.
- **Bug:** Rego eval P99 measured on 10-rule policy passes; production has 50 rules and fails. Test with worst-case policy size.
- **Bug:** `regorus` policy compile error swallowed → vault starts with broken policy. Compile at config load; fail-closed.
- **Bug:** Rego file paths in errors leak filesystem layout. Strip paths before user-facing error.

### P2 — Policy storage (E4-S2)
- **Bug:** SQLite `INSERT OR REPLACE` on policy table breaks immutability. Use plain INSERT, trigger to reject UPDATE.
- **Bug:** `parent_policy_id` cycles → infinite loop in lineage walker. Validate DAG at insert.
- **Bug:** Replay test passes for current policy version but breaks for archived (rego compile cached only for active). Recompile from stored YAML on demand.

### P2 — M-of-N admin sigs (E4-S3)
- **Bug:** Signature aggregation accepts duplicate signatures from same admin → counts as 2-of-3 with 1 admin signing twice. Track signer pubkeys, dedupe.
- **Bug:** Replay attack: admin sigs reused on new policy version. Bind sig to `policy_hash` AND `policy_version` AND `nonce`.
- **Bug:** M-of-N threshold = 0 accidentally → no signatures needed. Reject in config validation.

### P2 — Policy lint (E4-S4)
- **Bug:** Lint check sums `per_provider_daily` from map but doesn't include unbounded providers. Verify all providers covered or document.
- **Bug:** False positive on "duplicate provider" if same provider in different policy versions. Lint is per-version, dedupe scope is single policy.

### P2 — Budget (E5-S1)
- **Bug:** `BEGIN IMMEDIATE` not used → "database is locked" errors under concurrent reserves.
- **Bug:** Atomic reserve: SELECT then UPDATE in two queries → race condition. Use single UPDATE with WHERE clause: `UPDATE budget_accounts SET current_spent_usd = current_spent_usd + ? WHERE id = ? AND current_spent_usd + ? <= cap_usd RETURNING id`.
- **Bug:** Float in `cap_usd`: budget mismatch by sub-cent. Use `rust_decimal` everywhere.
- **Bug:** Reserve without commit window — TTL not enforced; reservation leaks. Add `reservation_expires_at` timestamp; GC task releases stale.
- **Bug:** Period reset uses local time → DST jumps create double-reset or no-reset hour. Use UTC + cron-style scheduling.
- **Bug:** `reset_period = "monthly"` reset at month boundary differs by length (28-31 days). Document as "30 days rolling" or "1st of month UTC".

### P2 — x402 verifier (E6-S1, S2, S3)
- **Bug:** Parser accepts `"amount": "0.05"` and `"amount": "5e-2"` differently. Normalize.
- **Bug:** Cert pin extraction: SPKI hash vs full cert hash vs leaf cert hash — all different. Document choice (default leaf SPKI).
- **Bug:** Hostname comparison case-sensitive (`API.example.com` vs `api.example.com`). Normalize lowercase before compare.
- **Bug:** Provider URL with trailing slash inconsistency. Normalize URL via `url::Url::parse`.
- **Bug:** Tolerance ±5% computed as `request - provider_amount` but should be `provider - request` (provider asks more than requested → reject). Document direction.
- **Bug:** Reputation score initialized at 0 for new providers → instant reject. Initialize at 50 (neutral).

---

## §4 Phase 3 — Audit + Emergency safeguards

### P3 — Audit hash chain (E10-S1)
- **Bug:** Concurrent `INSERT INTO audit_events` race → seq=N inserted twice with different prev_event_hash. Use SQLite `INSERT ... ON CONFLICT(seq) DO NOTHING RETURNING ...` pattern + atomic seq generation in same transaction.
- **Bug:** First event genesis prev_event_hash — what value? Document: `"0000...0000"` (64 zeros). Test it.
- **Bug:** Audit signer key not initialized before first event → bootstrap failure. Init signer key during `mandate init`, before any service starts.
- **Bug:** Tampering test: trigger DELETES the row instead of setting flag. SQLite triggers can't reject DELETE — they raise error, which client must handle correctly.
- **Bug:** Hash chain walker O(N²) for verification. Make O(N) with stream.

### P3 — Daily Merkle (E10-S2)
- **Bug:** Merkle tree from N events: pad to next power of 2 with zeros — but `H(0)` collides with empty leaf. Use `tagged_hash("leaf", x)` and `tagged_hash("node", x||y)` per Hashicorp transit pattern.
- **Bug:** Cron at UTC midnight skipped if vault was restarted at 23:59:30. Run on startup if last manifest > 24h old.
- **Bug:** Manifest filename `YYYY-MM-DD.json` — locale-dependent if not `chrono::format::strftime`. Force `%Y-%m-%d`.

### P3 — Emergency freeze (E12-S1)
- **Bug:** Singleton `EmergencyState` updated then audit event written → race with reader who checks state but processes request. Use single transaction: update state, insert audit, in same `BEGIN IMMEDIATE`.
- **Bug:** Latency target <100ms — middleware reads `EmergencyState` from DB on every request → adds 5-20ms. Cache in `Arc<RwLock<EmergencyState>>`, invalidate on update via channel.
- **Bug:** Resume during freeze: 1 admin signs → state stays frozen → second admin signs → state changes. But what if first admin signed, then revoked? Verify all sigs against current admin set, not historical.
- **Bug:** Freeze prevents new requests but in-flight requests proceed. Document semantics: "freeze prevents NEW requests; in-flight finish."

### P3 — Hardware kill switch (E12-S3)
- **Bug:** `evdev` crate requires read access to `/dev/input/event*` → permission denied as `mandate` user. udev rule needed (per `19_knowledge_base.md §6.3`).
- **Bug:** Listener thread crashes on device unplug → silent kill switch failure. Watchdog: re-open device on EBADF.
- **Bug:** Double-press window 1000ms — if user double-clicks at 1500ms → counted as 2 separate events → no freeze. Increase or document.
- **Bug:** Multiple kill switches connected → first event triggers freeze, subsequent ignored. Acceptable but log all events.

### P3 — Anomaly auto-freeze (E12-S4)
- **Bug:** Median calculation over rolling window not updated atomically with new event → off-by-one window. Use `BTreeMap` with timestamps for correct window slice.
- **Bug:** False positive rate >5% — model not calibrated against test corpus. Define corpus path, score against it in CI.
- **Bug:** Auto-freeze without notification to admin → admin doesn't know vault froze. Always notify when auto-freeze triggers.

### P3 — Audit coverage tests (E10-S5)
- **Bug:** Test in `core/tests/` doesn't have access to storage internals. Integration test, not unit. Move to `/tests/integration/`.
- **Bug:** Test enumerates known actions but new action added without test → silent no audit. Add CI lint: every `pub fn` in `mod actions` has a corresponding event type.

---

## §5 Phase 4 — Real x402 + Simulator safeguards

### P4 — Simulator (E7-S1)
- **Bug:** `eth_call` doesn't support state override on all RPC providers (public RPC = no). Detect at startup; refuse to start production if not supported.
- **Bug:** `debug_traceCall` permission required (premium tier on Alchemy). Document; provide fallback path.
- **Bug:** Block reorg between simulation pin block and broadcast → tx reverts. Check `block_number` at broadcast vs pin; if `>5` blocks drift, re-simulate.
- **Bug:** USDC contract address copy-paste error → wrong contract simulated. Use `19_knowledge_base.md §2.4` table; CI test cross-checks.

### P4 — Method whitelist (E7-S2)
- **Bug:** Selector decode treats `0x` prefix differently → false negative. Normalize input (strip `0x`, lowercase hex).
- **Bug:** ABI signature `transfer(address,uint256)` vs `transfer(address , uint256)` whitespace → different keccak256. Always strip whitespace before keccak.

### P4 — Multi-RPC quorum (E7-S3)
- **Bug:** Quorum compares responses but JSON has different field order → false disagreement. Compare on parsed struct, not raw JSON.
- **Bug:** Quorum on N=2 with M=2 = require both. Fault tolerance breaks. Document recommended config: `2-of-3`, not `2-of-2`.
- **Bug:** RPC at slightly different block height → state slightly differs → false disagreement. Pin block via earliest of N.

### P4 — Base Sepolia integration (E16-S1)
- **Bug:** Test wallet not pre-funded → integration test fails on first run. Document funding flow; provide faucet links.
- **Bug:** Base Sepolia RPC rate limits — quorum hits 429. Use multiple RPC URLs (per `19_knowledge_base.md §2.4`).
- **Bug:** Tx broadcast async, settlement watcher polls — race when broadcast tx not yet visible to RPC. Add retry with backoff.

### P4 — Settlement watcher (E2-S5)
- **Bug:** Confirmation depth=3 but RPC returns finality at depth=12 (Base finality). Document: "confirmation" means N blocks, not finality. Use both: log when finalized too.
- **Bug:** Watcher missed event during downtime → settlement_complete never written. Catch up on startup: replay txs in `pending` state.
- **Bug:** Failed settlement: tx revert reason hidden in logs (`status=0`). Decode revert reason via simulator's same RPC.

### P4 — Idempotency keys (E2-S6)
- **Bug:** Duplicate request with same key but different body → vault must reject (key collision). Hash body, store hash with key, compare on duplicate.
- **Bug:** TTL = 24h means same request can succeed today but fail tomorrow. Document.

---

## §6 Phase 5 — Hardware Isolation safeguards

### P5 — PKCS#11 (E8-S2)
- **Bug:** YubiHSM connector daemon not running → `cryptoki::Pkcs11::new("yubihsm_pkcs11.so")` succeeds but operations fail. Health check must do `get_session_info` to verify.
- **Bug:** Nitrokey OpenSC PKCS#11 module path varies (`/usr/lib/x86_64-linux-gnu/opensc-pkcs11.so` Ubuntu vs `/usr/lib/opensc-pkcs11.so` other). Auto-detect or document per-distro.
- **Bug:** PKCS#11 session handle leak under load. Wrap in connection pool.
- **Bug:** Sign with PKCS#11 returns DER-encoded signature; Ethereum needs raw `r || s || v`. Convert.
- **Bug:** Per-key constraints (sign-only, no extract) set at keygen; if not set initially, can't tighten later. Test that constraints actually enforced (test extract attempt).

### P5 — TPM (E8-S3)
- **Bug:** PCR sealing to PCR 0/2 (per CONF-05) — fixed, use 7+11+14.
- **Bug:** TPM busy: tpm2-abrmd not started or not connected. Service start order in systemd unit.
- **Bug:** TPM transient handle limit (3-4 typically) — leaks when not flushed. Flush context after every operation.
- **Bug:** SRK regenerated when TPM cleared → all sealed keys lost. Document; recovery procedure.

### P5 — Health monitor (E8-S6)
- **Bug:** Health check holds session → blocks real requests. Make health check non-blocking; separate session.
- **Bug:** Backend "degraded" state never recovers to "healthy" — only on restart. Add explicit recovery test.

### P5 — Production lint (E8-S7)
- **Bug:** Lint runs only at `mandate start` — config reload (SIGHUP) bypasses. Run lint on reload too.
- **Bug:** `attestation_required: false` allowed if backend is `mpc_remote` — not in spec. Add exception list explicitly.

### P5 — Admin enrollment (E13-S1)
- **Bug:** First admin enrolled via env var → leaks to logs / `/proc/<pid>/environ`. Use systemd-creds or stdin prompt.
- **Bug:** Subsequent admin enrollment requires existing admin sigs — but bootstrap admin alone can't add second admin (M-of-N=2 needed before second admin exists). Bootstrap mode: M-of-1 until N≥2.

### P5 — Multisig mutations (E13-S2)
- **Bug:** Partial signature state in `pending_mutations` table grows unbounded if mutations abandoned. GC task removes after TTL.
- **Bug:** Two admins simultaneously submit different mutations → both pending; first to reach M wins, other invalidated. Document and emit event.

### P5 — MCP server (E16-S5)
- **Bug:** MCP protocol versioning — vault must support multiple protocol versions per backward compat.
- **Bug:** Tool param validation: MCP doesn't enforce types beyond JSON schema; vault must validate.
- **Bug:** MCP doesn't natively support async approval — long-running tools fail timeout. Return "pending" + use poll.

---

## §7 Phase 6 — Approval + Governance UI safeguards

### P6 — Web UI (E11-S1)
- **Bug:** WebAuthn registration uses RPID = `localhost` → fails on `127.0.0.1`. Document hostname requirement; use `mandate.local` via `/etc/hosts` if needed.
- **Bug:** CSP allows inline styles → SvelteKit hydration injects them → blocked. Either nonce-based CSP or hash-based.
- **Bug:** Self-signed cert: every browser session prompts for trust. Document: instruction to add cert to system CA.
- **Bug:** WebSocket for live updates — auth not propagated from initial mTLS handshake. Re-auth on WS connect.

### P6 — Push relay (E11-S3)
- **Bug:** Relay deployed on same host as vault → SPOF. Document: separate VM/host.
- **Bug:** ntfy-style push tokens visible in subscriber URLs → leaked. Use opaque per-admin tokens.
- **Bug:** Mobile app receives push but signature verification key not bundled → must download. Bundle with app install.

### P6 — RBAC (E13-S3)
- **Bug:** Role check at endpoint dispatcher only → bypassed via internal API. Add at every layer.
- **Bug:** "Auditor" role can read audit log (sensitive provider names, recipient addresses). Acceptable but document.

### P6 — Webhooks (E10-S9)
- **Bug:** Subscriber URL pointing to vault itself → infinite loop on event. Reject loopback subscriber URLs.
- **Bug:** Webhook delivery sequential per-subscriber → slow subscriber blocks all. Per-subscriber queue.
- **Bug:** Retry exponential backoff overflow → schedules deliver years in future. Cap max retry interval.

### P6 — Approval TTL (E11-S5)
- **Bug:** TTL clock skew: client sees TTL expired but vault hasn't yet → confusing. Use vault clock authoritative; SDK polls.
- **Bug:** Long-poll holds connection > TTL → connection drops mid-poll. Set polling interval < TTL.

---

## §8 Phase 7 — TEE + Attestation safeguards

### P7 — Self-signed attestation (E9-S1)
- **Bug:** Composite measurement formula not stable: `H(binary || policy || config)` differs based on serialization order. Use canonical encoding (JCS for config).
- **Bug:** Attestation key can be rotated, but old attestations reference old key — verifier must support multiple key versions. Add `key_version` field.

### P7 — TDX (E9-S2)
- **Bug:** Use vsock path (deprecated) instead of configfs-tsm. Fix per `19_knowledge_base.md §1.1` — use `/sys/kernel/config/tsm/report/`.
- **Bug:** PCCS not reachable from QGS → attestation fails. Document PCCS configuration in install guide.
- **Bug:** Quote v4 vs v5 — verifier supports v4 only, runtime emits v5. Auto-detect; support both.
- **Bug:** `report_data` 64 bytes free-form — bind your data here. If left zero, attacker can replay quote with their own pubkey.
- **Bug:** Quote size ~5 KB → may exceed gRPC default message size. Configure max message size > 16 KB.

### P7 — SEV-SNP (E9-S3)
- **Bug:** VCEK fetch from KDS rate-limited → cache. Cache invalidation on TCB update (per `19_knowledge_base.md §1.2`).
- **Bug:** ECDSA-P384 signature verification slower than P256 — measure throughput.

### P7 — Attestation drift (E9-S5)
- **Bug:** Drift check runs every hour but detects mid-flight: should rule running attestation be invalidated immediately? Document semantics — invalidate, mark all in-flight as `requires_re_attestation`.

### P7 — TEE-sealed key (E8-S4)
- **Bug:** Sealing requires KMS-as-TApp or SGX bridge (per `19_knowledge_base.md §1.8`). Document deployment dependency.
- **Bug:** Recovery if TEE dies and sealed key lost → use offline backup of unsealed key (HSM-stored).

### P7 — Static binary release (E14-S1)
- **Bug:** musl static binary on Linux works for x86_64 but cross-compile to ARM64 needs `cross` tool. Document.
- **Bug:** Cosign v3 mandates `--bundle` (per `19_knowledge_base.md §5.11`). Old verifier won't accept.

### P7 — `.deb` / `.rpm` (E14-S2)
- **Bug:** Postinst creates user but doesn't add to system groups (e.g. `dialout` for serial). Document deps.
- **Bug:** systemd unit installed but not enabled — user expects auto-start. Document.

### P7 — Reproducible build (E14-S5)
- **Bug:** Two CI runs differ due to build container caching layers. Use `--pull always` to force fresh.

---

## §9 Phase 8 — On-chain safeguards

### P8 — Safe attested module (E16-S3)
- **Bug:** Safe v1.4.x address differs from v1.5 (in audit). Pin Safe version per deploy.
- **Bug:** Module bypasses owner sigs entirely — buggy module = drained Safe. Add timelock module above attestation module (per `19_knowledge_base.md §4.8`).
- **Bug:** EIP-1271 path forgotten → Permit2 / CowSwap break (per `19_knowledge_base.md §4.4`).

### P8 — Custom 4337 validator (E16-S6)
- **Bug:** Hardcode v0.6 EntryPoint, deploy on chain with v0.7 only. UserOp struct different, validation fails.
- **Bug:** Pin only `mrSigner` (per CONF safeguard). Pin BOTH `mrSigner` AND `mrTd`.
- **Bug:** Gas estimate too low → tx OOG; gas too high → user pays too much. Test on testnet.
- **Bug:** PCCS freshness not checked → revoked enclave passes (Jan 2026 dstack disclosure pattern).
- **Bug:** RIP-7212 not available on Ethereum L1 → deploy fails. Skip L1.

### P8 — Audit anchor (E16-S7)
- **Bug:** Cost <$0.01 budget — gas spike to 100 gwei makes it $0.50+. Adaptive: skip anchor if gas > threshold, retry next period.
- **Bug:** Anchor tx fails (revert) → audit log integrity claim broken. Retry; alert if persistently fails.

### P8 — Sponsor demo scripts (E15-S5)
- **Bug:** Scripts hardcode environment-specific values (RPC URL, addresses) → fail in different env. Use env vars + `.env.example`.

### P8 — Live attack demos (E11-S8)
- **Bug:** Demo state not idempotent → second run fails. Add explicit reset script.
- **Bug:** Network jitter during demo → x402 challenge times out. Pre-warm provider connection.

---

## §10 Phase 9-10 safeguards

### P9 — Marketplace (E16-S4)
- **Bug:** Buyer-seller flow assumes sync HTTP; real marketplace has webhooks. Document support level.

### P9 — ZK proof (E16-S10)
- **Bug:** RISC Zero / SP1 prover requires significant CPU/RAM (8GB+). Document hardware.
- **Bug:** Proof generation 30s+ → user-visible latency. Use async proof generation, return "pending."

### P9 — Mobile PWA (E11-S9)
- **Bug:** WebAuthn on mobile: passkeys need iCloud/Google account. Document fallback.
- **Bug:** PWA install prompt in Safari requires manual "Add to Home Screen". Document.

### P10 — Appliance image (E14-S4)
- **Bug:** First-boot wizard requires user input but image is headless. Use cloud-init for unattended config.

### P10 — External audit (E17-S5)
- **Bug:** Audit firm proposes high-risk findings — release blocked. Plan triage process; not all "high" require blocking remediation.

### P10 — Open source release (E18-S1)
- **Bug:** HN post needs strict content moderation — controversial framing kills momentum. Pre-write, get external review.

---

## §11 Cross-cutting common bugs

### CC-01 — Tokio runtime nesting
- **Bug:** Calling `.block_on()` inside async context → panic.
- **Fix:** Always use `.await`; no `block_on()` in production code.

### CC-02 — `Arc<Mutex<T>>` lock contention
- **Bug:** Hot path takes lock for whole duration → serialized. Use `RwLock` for read-heavy or `dashmap` for sharded.

### CC-03 — `Vec::push` past capacity copies — defeats zeroize
- **Bug:** Documented in §5.10 of KB. Pre-allocate `with_capacity`.

### CC-04 — `Drop` order
- **Bug:** Field order in struct = drop order. If `signer` drops before `audit_writer`, signing of "shutdown" event fails. Manual `drop()` calls in shutdown handler.

### CC-05 — Time API non-monotonic
- **Bug:** `chrono::Utc::now()` can go backwards (NTP step). For TTL and rate-limit, use `Instant::now()` (monotonic).

### CC-06 — Async traits and `dyn`
- **Bug:** `async fn` in trait requires `#[async_trait]` or `Box<dyn Future>`. Use latest Rust async-trait.

### CC-07 — `Cow<str>` vs `String` allocation
- **Bug:** `to_string()` everywhere allocates. Use `Cow<str>` for input that may or may not need owned.

### CC-08 — `serde_json::Value` instead of typed
- **Bug:** Using `Value` loses type safety, allows malformed downstream. Always use typed structs.

### CC-09 — Logging in hot path
- **Bug:** `tracing::debug!` in tight loop → format strings allocated even if filter rejects. Use `tracing::event_enabled!` guard.

### CC-10 — Error type explosion
- **Bug:** Each crate defines own Error → boxing everywhere. Use `thiserror` per-crate + workspace `Error` for top-level mapping.

---

## §12 Pre-commit / pre-merge checklist

Before any agent's slot is "done":

- [ ] Read pre-listed files in §0
- [ ] Implementation matches `17_interface_contracts.md` exactly (no liberty)
- [ ] All `D-PN-NN` demos for slot pass
- [ ] No `unwrap()`, `println!`, raw payload logging, float for money
- [ ] CI green: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo audit`
- [ ] No new top-level dependencies without KB-justification
- [ ] If new schema field: `17_interface_contracts.md` updated FIRST (PR depends on contracts PR)
- [ ] If new error code: `17_interface_contracts.md §3.1` updated
- [ ] If new audit event type: `17_interface_contracts.md §5.4` updated
- [ ] If new metric: matches `mandate_*` naming pattern
- [ ] No third-party CDN, no Telemetry to external service unless OTLP (configurable)
- [ ] Cross-restart test for any persistent state
- [ ] Hand-back report to orchestrator: "implemented X, demos Y passed, deviations Z"

---

## §13 If a demo fails

If during loop run any `D-PN-NN` demo fails:

1. **Read `evidence.json` first** — it has structured failure context.
2. **Don't modify the demo** unless you're 100% sure the demo itself is wrong (then escalate to orchestrator before changing).
3. **Trace the failure** through telemetry: each demo emits a `tracing` span; capture.
4. **Re-read** the corresponding section in `17_interface_contracts.md` and `19_knowledge_base.md`. Likely the spec is more strict than the impl assumed.
5. **Fix at the right layer:** if validation, fix in validator; if storage, fix in storage. Don't paper over with try/catch in higher layer.
6. **Re-run only the affected demo** (`bash demo-scripts/run-single.sh D-PN-NN`).
7. **After fix, re-run whole phase** to ensure no regression.

If after 3 fix attempts demo still fails: stop, report context to orchestrator, do NOT continue.
