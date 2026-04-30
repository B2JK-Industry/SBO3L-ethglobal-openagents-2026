# Phase 1 — Foundation + Guaranteed Bounty #1 (Days 1-30)

> Goal: Production-ready core + adoption surface. Exit gate locks **KeeperHub Builder Feedback ($250)** submitted.

## Phase 1 ticket index

| ID | Title | Owner | Effort | Depends |
|---|---|---|---|---|
| F-1 | Real auth middleware (bearer + JWT) | 🦀 Alice | 8h | — |
| F-2 | Persistent budget store | 🦀 Alice | 6h | F-1 |
| F-3 | Idempotency atomicity (state machine) | 🦀 Alice | 6h | F-2 |
| F-4 | Public-bind safety gate | 🦀 Alice | 1h | — |
| F-5 | KMS abstraction with backends | 🦀 Alice | 12h | F-3 |
| F-6 | Self-contained Passport capsule v2 | 🛠️ Bob | 12h | F-2, F-3 |
| F-7 | Dockerfile (multi-stage, slim) | 🚢 Grace | 4h | F-1 |
| F-8 | docker-compose.yml | 🚢 Grace | 2h | F-7 |
| F-9 | TypeScript SDK on npm (`@sbo3l/sdk`) | 📘 Carol | 8h | F-1, F-6 |
| F-10 | Python SDK on PyPI (`sbo3l-sdk`) | 🐍 Dave | 8h | F-1, F-6 |
| F-11 | crates.io publishable chain (9 crates) | 🛠️ Bob | 6h | F-1..F-6 |
| F-12 | examples/typescript-agent/ | 📘 Carol | 4h | F-9 |
| F-13 | examples/python-agent/ | 🐍 Dave | 4h | F-10 |
| T-2-1 | KH Builder Feedback — file 5+ GitHub issues | Daniel | 2h | F-9, F-10 (must use) |
| T-2-2 | FEEDBACK.md expanded with concrete pain points | Daniel | 1h | T-2-1 |

**Total Phase 1 effort:** ~84h. With 5 agents in parallel + Daniel manual = ~30 days.

---

## [F-1] Real auth middleware (bearer + JWT)

**Owner:** 🦀 Alice
**Effort:** 8h
**Phase:** 1
**Depends:** none (start day 1)

**Files:**
- `crates/sbo3l-server/src/auth.rs` (new)
- `crates/sbo3l-server/src/lib.rs` (wire into router)
- `crates/sbo3l-server/Cargo.toml` (add `jsonwebtoken = "9"`, `bcrypt = "0.15"`)
- `tests/integration_auth.rs` (new)
- `SECURITY_NOTES.md` (remove "no auth" warning)

**What:**
Add bearer token + JWT validation middleware to `POST /v1/payment-requests`. JWT must be agent_id-bound (claim `sub` matches APRP `agent_id`). Bearer mode for service-to-service: env var `SBO3L_BEARER_TOKEN_HASH` stores bcrypt hash of expected token. Default-deny if no `Authorization` header, unless `SBO3L_ALLOW_UNAUTHENTICATED=1` is set.

**Acceptance criteria:**
- [ ] `Authorization: Bearer <valid-token>` → request flows
- [ ] `Authorization: Bearer <invalid-token>` → HTTP 401 + RFC 7807 body with `code: "auth.invalid_token"`
- [ ] `Authorization: Bearer eyJ...` (JWT) → validated against pubkey from env `SBO3L_JWT_PUBKEY_HEX`; claim `sub` must match APRP `agent_id`, else 401 + `auth.agent_id_mismatch`
- [ ] No Authorization header + `SBO3L_ALLOW_UNAUTHENTICATED=1` → request flows
- [ ] No Authorization header + env unset → HTTP 401 + `auth.required`
- [ ] Stderr warning at startup if dev flag set ("⚠ UNAUTHENTICATED MODE — DEV ONLY ⚠")
- [ ] No tokens or token hashes in logs (grep test)
- [ ] All errors RFC 7807-shaped (`type`, `title`, `status`, `detail`, `code`)

**QA Test Plan:**
```bash
# 1. Dev mode bypass
pkill -f sbo3l-server || true
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server > /tmp/sbo3l-auth-1.log 2>&1 &
sleep 2
curl -s :8730/v1/payment-requests -X POST -H "Content-Type: application/json" \
  -d @test-corpus/aprp/golden_001_minimal.json | jq -r .decision
# expect: "allow"
grep -q "UNAUTHENTICATED MODE — DEV ONLY" /tmp/sbo3l-auth-1.log
# expect: exit 0

# 2. Bearer required
pkill -f sbo3l-server
HASH=$(htpasswd -nbB testuser sekret123 | cut -d: -f2)
SBO3L_BEARER_TOKEN_HASH="$HASH" cargo run --bin sbo3l-server > /tmp/sbo3l-auth-2.log 2>&1 &
sleep 2

# 2a. No auth → 401
RC=$(curl -sw "%{http_code}" -o /dev/null :8730/v1/payment-requests -X POST \
  -H "Content-Type: application/json" -d @test-corpus/aprp/golden_001_minimal.json)
[ "$RC" = "401" ] || echo FAIL no-auth-rc=$RC

# 2b. Bearer correct → 200
RC=$(curl -sw "%{http_code}" -o /dev/null :8730/v1/payment-requests -X POST \
  -H "Authorization: Bearer sekret123" -H "Content-Type: application/json" \
  -d @test-corpus/aprp/golden_001_minimal.json)
[ "$RC" = "200" ] || echo FAIL bearer-rc=$RC

# 3. Token NOT in logs
grep -q "sekret123" /tmp/sbo3l-auth-2.log && echo FAIL token-leak

# 4. Test suite
cargo test --test integration_auth
# expect: all green
pkill -f sbo3l-server
```

**[D] Daniel review checklist:**
- [ ] No tokens leaked in logs (Heidi grep verifies)
- [ ] JWT signature validation rejects fake pubkey
- [ ] SECURITY_NOTES.md updated to remove "no auth" warning, replace with "auth required by default"
- [ ] Identity sub-claim "no-key boundary" preserved (server still verifies, agent still has no key)

---

## [F-2] Persistent budget store

**Owner:** 🦀 Alice | **Effort:** 6h | **Depends:** F-1

**Files:**
- `crates/sbo3l-storage/migrations/V008__budget_state.sql` (new)
- `crates/sbo3l-policy/src/budget.rs` (refactor `BudgetTracker` to use Storage)
- `crates/sbo3l-policy/src/lib.rs` (re-export)
- `crates/sbo3l-server/src/lib.rs` (AppState wires budget to Storage)
- `tests/test_budget_persistence.rs` (new)

**What:**
Replace in-memory `HashMap<BudgetKey, BudgetState>` with SQLite-backed `budget_state` table. ACID via transaction wrapping policy decision + budget commit + audit append. Multi-scope: `per_tx`, `daily`, `monthly`, `per_provider`.

Migration `V008__budget_state.sql` creates:
```sql
CREATE TABLE budget_state (
  agent_id TEXT NOT NULL,
  scope TEXT NOT NULL,         -- 'per_tx' | 'daily' | 'monthly' | 'per_provider'
  scope_key TEXT NOT NULL,     -- e.g. '2026-04-30' for daily, 'keeperhub' for per_provider
  spent_cents INTEGER NOT NULL DEFAULT 0,
  cap_cents INTEGER NOT NULL,
  reset_at_unix INTEGER,        -- when to reset (e.g. midnight UTC for daily)
  PRIMARY KEY (agent_id, scope, scope_key)
);
CREATE INDEX idx_budget_state_agent ON budget_state(agent_id, scope);
```

**Acceptance criteria:**
- [ ] Migration V008 applies cleanly on fresh DB; `schema_migrations.sha256` content invariant preserved
- [ ] `BudgetTracker::commit()` wraps policy + budget + audit in single transaction
- [ ] Daemon restart preserves budget state (test below)
- [ ] Concurrent same-key writes serialize correctly
- [ ] `bash demo-scripts/run-production-shaped-mock.sh` still passes

**QA Test Plan:**
```bash
rm -f /tmp/sbo3l-budget-test.db
SBO3L_DB=/tmp/sbo3l-budget-test.db SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
sleep 2

# Spend 5 cents (under 10-cent daily cap from default policy)
PAYLOAD=$(jq '.amount.value = "0.05" | .nonce = "01HTAWX5K3R8YV9NQB7C6P2D01"' test-corpus/aprp/golden_001_minimal.json)
curl -s :8730/v1/payment-requests -X POST -H "Content-Type: application/json" -d "$PAYLOAD" | jq -r .decision
# expect: "allow"

# Restart
pkill sbo3l-server
SBO3L_DB=/tmp/sbo3l-budget-test.db SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
sleep 2

# Spend 6 cents → should fail (5+6=11 > 10)
PAYLOAD=$(jq '.amount.value = "0.06" | .nonce = "01HTAWX5K3R8YV9NQB7C6P2D02"' test-corpus/aprp/golden_001_minimal.json)
curl -s :8730/v1/payment-requests -X POST -H "Content-Type: application/json" -d "$PAYLOAD" | jq -r .deny_code
# expect: "policy.budget_exceeded"

cargo test --test test_budget_persistence
pkill sbo3l-server
```

**[D] Daniel review:** Migration is idempotent + content-hashed; doesn't break existing DBs.

---

## [F-3] Idempotency atomicity (state machine)

**Owner:** 🦀 Alice | **Effort:** 6h | **Depends:** F-2

**Files:**
- `crates/sbo3l-storage/migrations/V009__idempotency_atomicity.sql` (alter table; add `state` column)
- `crates/sbo3l-server/src/idempotency.rs` (refactor)
- `tests/test_idempotency_race.rs` (new)

**What:**
Replace post-success cache write with `processing → succeeded | failed` state machine. First request atomically INSERTs with `state='processing'`. Concurrent same-key request finds `state='processing'` → returns 409 `protocol.idempotency_in_flight`. Pipeline completes → state moves to `succeeded`. Same-key+same-body after success → cached replay. Same-key+diff-body → 409 `protocol.idempotency_conflict`. Failed pipeline → `state='failed'`, retry possible after 60s grace.

**Acceptance criteria:**
- [ ] Migration V009 applies cleanly
- [ ] Concurrent same-key requests: one passes, others get 409 `protocol.idempotency_in_flight`
- [ ] Same-key + same-body after success → byte-identical cached replay
- [ ] Same-key + diff-body → 409 `protocol.idempotency_conflict`
- [ ] Failed pipeline → state=`failed`, retry possible after 60s
- [ ] No race condition under 50-concurrent-request stress test

**QA Test Plan:**
```bash
KEY="01TESTRACEKEY9UNIQUE16chars"
PAYLOAD=$(jq '.nonce = "01HTAWX5K3R8YV9NQB7C6P2D11"' test-corpus/aprp/golden_001_minimal.json)

# 50 concurrent same-key requests
for i in {1..50}; do
  curl -s -o /dev/null -w "%{http_code}\n" :8730/v1/payment-requests -X POST \
    -H "Authorization: Bearer test" -H "Idempotency-Key: $KEY" \
    -H "Content-Type: application/json" -d "$PAYLOAD" &
done | sort | uniq -c
# expect: exactly one 200, the rest 409

cargo test --test test_idempotency_race
```

**[D] Daniel review:** No double-spend possible; budget table consistent post-race.

---

## [F-4] Public-bind safety gate

**Owner:** 🦀 Alice | **Effort:** 1h | **Depends:** none (parallel with F-1, F-2, F-3)

**Files:** `crates/sbo3l-server/src/main.rs`

**What:**
Refuse to bind to non-loopback (`0.0.0.0`, `::`, public IP) unless `SBO3L_ALLOW_UNSAFE_PUBLIC_BIND=1` is set. Print red stderr warning if public bind enabled.

**Acceptance criteria:**
- [ ] `SBO3L_LISTEN=0.0.0.0:8730 sbo3l-server` → exits with error + exit code 2
- [ ] `SBO3L_LISTEN=0.0.0.0:8730 SBO3L_ALLOW_UNSAFE_PUBLIC_BIND=1 sbo3l-server` → starts + warning to stderr
- [ ] Default `SBO3L_LISTEN=127.0.0.1:8730` → starts normally, no warning

**QA Test Plan:**
```bash
# 1. Public bind without flag → fail
SBO3L_LISTEN=0.0.0.0:8730 ./target/debug/sbo3l-server 2>&1 | grep -q "unsafe public bind"
echo "test 1 rc=$?"  # expect 0

# 2. Public bind with flag → start
SBO3L_LISTEN=0.0.0.0:18730 SBO3L_ALLOW_UNSAFE_PUBLIC_BIND=1 ./target/debug/sbo3l-server > /tmp/sbo3l-pb.log 2>&1 &
PID=$!
sleep 2
grep -q "UNSAFE PUBLIC BIND" /tmp/sbo3l-pb.log
echo "test 2 rc=$?"
kill $PID

# 3. Default → start clean
./target/debug/sbo3l-server > /tmp/sbo3l-default.log 2>&1 &
PID=$!
sleep 2
grep -q "unsafe" /tmp/sbo3l-default.log && echo FAIL clean-default
kill $PID
```

---

## [F-5] KMS abstraction with backends

**Owner:** 🦀 Alice | **Effort:** 12h | **Depends:** F-3

**Files:**
- `crates/sbo3l-core/src/signer.rs` (new — `Signer` trait + `SignerError`)
- `crates/sbo3l-core/src/signers/mod.rs` (new)
- `crates/sbo3l-core/src/signers/dev.rs` (current dev seeds, gated `SBO3L_DEV_ONLY_SIGNER=1`)
- `crates/sbo3l-core/src/signers/aws_kms.rs` (new, behind `aws_kms` feature)
- `crates/sbo3l-core/src/signers/gcp_kms.rs` (new, behind `gcp_kms` feature)
- `crates/sbo3l-core/src/signers/phala_tee.rs` (new, behind `phala_tee` feature, stub)
- `crates/sbo3l-core/Cargo.toml` (feature flags)
- `crates/sbo3l-server/src/lib.rs` (signer factory from env)
- `tests/test_signers.rs` (new)
- `SECURITY_NOTES.md` (update signing seeds section)

**What:**
Define `Signer` trait:
```rust
pub trait Signer: Send + Sync {
    fn sign(&self, message: &[u8]) -> Result<Signature, SignerError>;
    fn pubkey(&self) -> PublicKey;
    fn key_id(&self) -> &str;
}
```

Backends (each behind feature flag):
- `DevSigner` — refuses unless `SBO3L_DEV_ONLY_SIGNER=1`; clear stderr warning at startup
- `AwsKmsSigner` — wraps `aws-sdk-kms`, signs via KMS API
- `GcpKmsSigner` — wraps `google-cloud-kms`, signs via KMS API
- `PhalaTeeSigner` — placeholder; Phase 3 wires real TEE

Selection: env `SBO3L_SIGNER_BACKEND` ∈ `{dev, aws_kms, gcp_kms, phala_tee}`.

**Acceptance criteria:**
- [ ] `Signer` trait + 4 impls compile (each behind feature flag)
- [ ] DevSigner refuses without `SBO3L_DEV_ONLY_SIGNER=1` (exit code 2 + stderr warning)
- [ ] AwsKmsSigner integration test passes against real AWS KMS test key (Daniel provides)
- [ ] GcpKmsSigner integration test passes against real GCP KMS test key (Daniel provides)
- [ ] `cargo test --workspace --all-targets` baseline 377/377 unchanged for non-feature path
- [ ] Receipt verification works interchangeably across signers (signature format identical)

**QA Test Plan:**
```bash
# 1. Dev signer without flag → fail
SBO3L_SIGNER_BACKEND=dev cargo run --bin sbo3l-server 2>&1 | grep -q "DEV signer requires SBO3L_DEV_ONLY_SIGNER=1"
echo $?  # expect 0

# 2. Dev signer with flag → starts
SBO3L_SIGNER_BACKEND=dev SBO3L_DEV_ONLY_SIGNER=1 cargo run --bin sbo3l-server > /tmp/sbo3l-dev.log 2>&1 &
sleep 2
grep -q "⚠ DEV ONLY SIGNER ⚠" /tmp/sbo3l-dev.log
pkill sbo3l-server

# 3. AWS KMS path (requires Daniel-provisioned key)
SBO3L_SIGNER_BACKEND=aws_kms SBO3L_AWS_KMS_KEY_ID=alias/sbo3l-test \
  cargo test --features aws_kms --test test_signers test_aws_kms_signer

# 4. Receipt verification interop (signed with AWS KMS, verified by sbo3l-cli)
RECEIPT=$(/tmp/get-receipt-aws.sh)
echo "$RECEIPT" | cargo run -p sbo3l-cli -- audit verify-bundle --stdin
# expect: rc=0
```

**[D] Daniel review:**
- [ ] Daniel provisions AWS KMS test key (one-time, ~30 min)
- [ ] AWS credentials in CI Secrets if running in CI
- [ ] Phala TEE stub clearly labelled "Phase 3 wires real TEE"
- [ ] Identity sub-claim "no-key boundary" preserved (signer is internal to SBO3L; agent never holds it)

---

## [F-6] Self-contained Passport capsule v2

**Owner:** 🛠️ Bob | **Effort:** 12h | **Depends:** F-2, F-3

**Files:**
- `schemas/sbo3l.passport_capsule.v2.json` (new schema, additive on v1)
- `crates/sbo3l-core/src/passport.rs` (verifier handles v1 + v2; emits v2 default)
- `crates/sbo3l-cli/src/passport.rs` (CLI emits v2 via `passport run`)
- `test-corpus/passport/v2_golden_001_minimal.json` and 4 more (5 golden v2 fixtures)
- `test-corpus/passport/v2_tampered_001..004.json` (4 tampered v2 fixtures)
- `docs/product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md` (update for v2)
- `docs/cli/passport.md` (update CLI docs)

**What:**
Capsule schema v2 adds two optional fields:
- `policy.policy_snapshot` — full canonical policy JSON (so `--strict` recomputes `policy_hash` without `--policy <path>`)
- `audit.audit_segment` — bundle-shaped segment of audit chain (so `--strict` walks chain without `--audit-bundle <path>`)

Both OPTIONAL for v2 backwards-compat with v1 callers. When present, `--strict` reads them instead of demanding aux inputs. v1 capsules still verify (compatibility table in source-of-truth doc).

**Acceptance criteria:**
- [ ] Schema v2 published, JSON Schema validates v2 capsules
- [ ] `passport run` emits v2 by default; `--schema-version v1` flag to force v1
- [ ] `passport verify --strict --path <v2-capsule>` runs ALL 6 crypto checks with no aux inputs (no SKIPPED for absent aux)
- [ ] 5 golden v2 fixtures schema-validate
- [ ] 4 tampered v2 fixtures reject with rc=2 + capsule.* error code
- [ ] All existing v1 fixtures continue to verify (no regression)
- [ ] `passport explain --path <v2-capsule>` adds line `verifier-mode: self-contained`
- [ ] Doc `SBO3L_PASSPORT_SOURCE_OF_TRUTH.md` has v1↔v2 compatibility table

**QA Test Plan:**
```bash
# 1. Round-trip v2 emit + strict verify
cargo run -p sbo3l-cli -- passport run \
  test-corpus/aprp/golden_001_minimal.json \
  --db /tmp/passport-v2-test.db \
  --agent research-agent.team.eth \
  --resolver offline-fixture --ens-fixture demo-fixtures/ens-records.json \
  --executor keeperhub --mode mock \
  --out /tmp/capsule-v2.json

cargo run -p sbo3l-cli -- passport verify --strict --path /tmp/capsule-v2.json
# expect: rc=0; structural + request_hash_recompute + policy_hash_recompute +
# receipt_signature + audit_chain + audit_event_link all PASSED, none SKIPPED

# 2. v1 backwards compat
cargo run -p sbo3l-cli -- passport verify --strict --path test-corpus/passport/golden_001_allow_keeperhub_mock.json
# expect: rc=0; structural + request_hash_recompute PASSED, others SKIPPED (v1 has no embedded fields)

# 3. v2 tampered fixtures
for f in test-corpus/passport/v2_tampered_*.json; do
  cargo run -p sbo3l-cli -- passport verify --path "$f"
  echo "$f rc=$?"  # expect 2 for all
done

# 4. Existing test suite still green
cargo test --workspace --all-targets

# 5. Schema validation
python3 scripts/validate_schemas.py
```

**[D] Daniel review:**
- [ ] Schema v2 backwards-compat: every existing v1 capsule on disk verifies
- [ ] `policy_snapshot` doesn't leak signing keys (policy never contains them)
- [ ] `audit_segment` size cap enforced (`< 1 MB` per capsule, error otherwise)
- [ ] Identity sub-claim "self-contained capsule" passes its test

---

## [F-7] Dockerfile (multi-stage, slim)

**Owner:** 🚢 Grace | **Effort:** 4h | **Depends:** F-1

**Files:**
- `Dockerfile` (new, root)
- `.dockerignore` (new)
- `docs/cli/docker.md` (new)

**What:**
Multi-stage build:
- Stage 1: `rust:1.85-bookworm` — builds `sbo3l-server` + `sbo3l` CLI in release mode
- Stage 2: `gcr.io/distroless/cc-debian12` — runtime, copies binary + `migrations/`
- Final image: < 100 MB
- `EXPOSE 8730`
- `ENV SBO3L_LISTEN=0.0.0.0:8730 SBO3L_ALLOW_UNSAFE_PUBLIC_BIND=1` (in container, public bind is the point)
- Default `CMD ["/usr/local/bin/sbo3l-server"]`

**Acceptance criteria:**
- [ ] `docker build -t sbo3l/server .` succeeds
- [ ] Image size < 100 MB
- [ ] `docker run -p 8730:8730 -e SBO3L_ALLOW_UNAUTHENTICATED=1 sbo3l/server` starts
- [ ] Curl from host hits daemon, gets allow response
- [ ] `docs/cli/docker.md` has copy-paste-runnable example

**QA Test Plan:**
```bash
docker build -t sbo3l/server:test .
SIZE=$(docker images sbo3l/server:test --format '{{.Size}}')
echo "image size: $SIZE"  # expect < 100MB

docker run -d --rm --name sbo3l-test -p 18730:8730 \
  -e SBO3L_ALLOW_UNAUTHENTICATED=1 \
  -e SBO3L_DEV_ONLY_SIGNER=1 \
  -e SBO3L_SIGNER_BACKEND=dev \
  sbo3l/server:test
sleep 5

curl -s http://localhost:18730/v1/payment-requests -X POST \
  -H "Content-Type: application/json" \
  -d @test-corpus/aprp/golden_001_minimal.json | jq -r .decision
# expect: "allow"

docker stop sbo3l-test
```

---

## [F-8] docker-compose.yml

**Owner:** 🚢 Grace | **Effort:** 2h | **Depends:** F-7

**Files:** `docker-compose.yml` (new, root)

**What:**
`docker compose up sbo3l` starts:
- `sbo3l-server` container with persistent volume `./data:/var/lib/sbo3l`
- Health check via `/health` endpoint (Alice adds in F-1+)
- Optional `sbo3l-mcp` profile: `docker compose --profile mcp up`

**Acceptance criteria:**
- [ ] `docker compose up sbo3l -d` brings up daemon in < 10s
- [ ] Daemon survives `docker compose restart` (sqlite persisted in `./data`)
- [ ] `docker compose down` cleans up cleanly
- [ ] `docker compose --profile mcp up sbo3l-mcp` exposes MCP stdio on host

**QA Test Plan:**
```bash
docker compose up sbo3l -d
sleep 5
docker compose ps | grep -q "running"
curl -s http://localhost:8730/v1/payment-requests -X POST \
  -H "Content-Type: application/json" \
  -d @test-corpus/aprp/golden_001_minimal.json | jq -r .decision
# expect: "allow"

docker compose restart sbo3l
sleep 3
curl -s http://localhost:8730/v1/payment-requests -X POST \
  -H "Content-Type: application/json" \
  -d @test-corpus/aprp/golden_001_minimal.json | jq -r .decision
# expect: "allow" (state persists)

docker compose down
```

---

## [F-9] TypeScript SDK on npm (`@sbo3l/sdk`)

**Owner:** 📘 Carol | **Effort:** 8h | **Depends:** F-1, F-6

**Files:**
- `sdks/typescript/package.json` (scope `@sbo3l`, version `0.1.0`)
- `sdks/typescript/tsconfig.json`
- `sdks/typescript/src/index.ts`
- `sdks/typescript/src/client.ts` (HTTP client, fetch-based)
- `sdks/typescript/src/types.ts` (auto-gen from schemas via `quicktype`)
- `sdks/typescript/src/passport.ts` (client-side capsule structural verifier — JS port of v1+v2)
- `sdks/typescript/src/auth.ts` (bearer + JWT helpers)
- `sdks/typescript/test/*.test.ts` (vitest)
- `sdks/typescript/README.md`
- `.github/workflows/sdk-typescript.yml` (publish on tag `sdk-ts-v*`)

**What:**
Typed TypeScript SDK. Core API:
```typescript
import { SBO3LClient } from "@sbo3l/sdk";

const client = new SBO3LClient({
  endpoint: "http://localhost:8730",
  bearerToken: process.env.SBO3L_BEARER_TOKEN!,
});

const receipt = await client.submit({
  agent_id: "research-agent-01",
  intent: "purchase_api_call",
  amount: { value: "0.05", currency: "USD" },
  // ...
});

if (receipt.decision === "allow") {
  console.log("execution_ref:", receipt.execution_ref);
  // verify capsule structurally
  const verdict = client.passport.verify(capsule);
}
```

Types auto-generated from `schemas/aprp.json` + `schemas/policy_receipt.json` + `sbo3l.passport_capsule.v2.json`.

**Acceptance criteria:**
- [ ] `npm install @sbo3l/sdk` works after publish
- [ ] Submit + parse receipt + structural-verify capsule
- [ ] 100% TypeScript type coverage (no `any`)
- [ ] `vitest` suite ≥ 30 unit tests passing
- [ ] Published to npm under `@sbo3l/sdk` scope (Daniel provides token)
- [ ] Bundle size < 50 KB gzipped (no fetch polyfill)

**QA Test Plan:**
```bash
cd sdks/typescript
npm install
npm test
npm run build

# Manual smoke against running daemon
node -e "
import { SBO3LClient } from './dist/index.js';
const c = new SBO3LClient({ endpoint: 'http://localhost:8730', bearerToken: 'test' });
const r = await c.submit(JSON.parse(require('fs').readFileSync('../../test-corpus/aprp/golden_001_minimal.json')));
console.log(r.decision);
"
# expect: "allow"

# Bundle size check
gzip -c dist/index.js | wc -c  # expect < 50000
```

**[D] Daniel review:**
- [ ] Daniel creates npm scope `@sbo3l` + provides publish token
- [ ] Reviews public API surface (every export intentional)
- [ ] Heidi runs against fresh daemon end-to-end

---

## [F-10] Python SDK on PyPI (`sbo3l-sdk`)

**Owner:** 🐍 Dave | **Effort:** 8h | **Depends:** F-1, F-6 (parallel with F-9)

**Files:**
- `sdks/python/pyproject.toml`
- `sdks/python/sbo3l_sdk/__init__.py`
- `sdks/python/sbo3l_sdk/client.py` (httpx-based, sync + async)
- `sdks/python/sbo3l_sdk/types.py` (Pydantic v2 strict models from schemas)
- `sdks/python/sbo3l_sdk/passport.py` (capsule structural verifier)
- `sdks/python/sbo3l_sdk/auth.py`
- `sdks/python/tests/`
- `sdks/python/README.md`
- `.github/workflows/sdk-python.yml`

**What:** Mirror of F-9 in Python. Async-first with sync wrappers.
```python
from sbo3l_sdk import SBO3LClient
import os

client = SBO3LClient(
    endpoint="http://localhost:8730",
    bearer_token=os.environ["SBO3L_BEARER_TOKEN"]
)
receipt = await client.submit({...})
```

**Acceptance criteria:**
- [ ] `pip install sbo3l-sdk` works after publish
- [ ] Sync + async API both supported
- [ ] Pydantic v2 strict
- [ ] pytest ≥ 30 tests passing
- [ ] mypy --strict clean
- [ ] Published to PyPI

**QA Test Plan:** equivalent to F-9 in Python toolchain. See [F-9] structure.

---

## [F-11] crates.io publishable chain (9 crates)

**Owner:** 🛠️ Bob | **Effort:** 6h | **Depends:** F-1..F-6 merged

**Files:**
- All `crates/*/Cargo.toml` (set `version = "0.1.0"`, `repository`, `description`, `license = "MIT OR Apache-2.0"`, `keywords`, `categories`)
- `.github/workflows/crates-publish.yml` (publish all 9 in dep order on tag `v*`)

**What:**
Make all 9 crates publishable to crates.io. Dependency order:
```
1. sbo3l-core (no internal deps)
2. sbo3l-storage (deps: sbo3l-core)
3. sbo3l-policy (deps: sbo3l-core)
4. sbo3l-identity (deps: sbo3l-core)
5. sbo3l-execution (deps: sbo3l-core, sbo3l-policy)
6. sbo3l-keeperhub-adapter (deps: sbo3l-core)
7. sbo3l-server (deps: all of above)
8. sbo3l-mcp (deps: above)
9. sbo3l-cli (deps: all of above)
```

**Acceptance criteria:**
- [ ] `cargo publish --dry-run -p sbo3l-core` succeeds
- [ ] All 9 crates dry-run in dependency order
- [ ] CI workflow publishes on tag `v0.1.0`
- [ ] After Daniel pushes tag, all 9 crates appear on crates.io

**QA Test Plan:**
```bash
for crate in sbo3l-core sbo3l-storage sbo3l-policy sbo3l-identity \
             sbo3l-execution sbo3l-keeperhub-adapter sbo3l-server \
             sbo3l-mcp sbo3l-cli; do
  cargo publish --dry-run -p $crate || { echo "❌ $crate failed dry-run"; exit 1; }
  echo "✅ $crate dry-run OK"
done
```

**[D] Daniel review:**
- [ ] Daniel provides `CARGO_REGISTRY_TOKEN` to GitHub Secrets
- [ ] Daniel pushes tag to trigger publish

---

## [F-12] examples/typescript-agent/

**Owner:** 📘 Carol | **Effort:** 4h | **Depends:** F-9

**Files:**
- `examples/typescript-agent/package.json`
- `examples/typescript-agent/src/index.ts` (~30 lines)
- `examples/typescript-agent/README.md`
- `examples/typescript-agent/expected-output.txt`

**What:**
Minimal real TypeScript agent:
1. Loads APRP from `test-corpus/aprp/golden_001_minimal.json`
2. Submits via `@sbo3l/sdk`
3. Prints decision + execution_ref
4. Verifies receipt signature locally
5. ~30 lines total

**Acceptance criteria:**
- [ ] `npx tsx examples/typescript-agent/src/index.ts` against running daemon → prints decision + execution_ref
- [ ] Output matches `expected-output.txt` (deterministic except ULIDs/timestamps/signatures)
- [ ] README has copy-paste run instructions

**QA Test Plan:**
```bash
cd examples/typescript-agent
npm install
node --import tsx ./src/index.ts > /tmp/ts-agent-output.txt
diff <(grep -E "^(decision|execution_ref):" /tmp/ts-agent-output.txt) \
     <(grep -E "^(decision|execution_ref):" expected-output.txt)
# expect: lines match
```

---

## [F-13] examples/python-agent/

**Owner:** 🐍 Dave | **Effort:** 4h | **Depends:** F-10

**Files:**
- `examples/python-agent/main.py` (~30 lines)
- `examples/python-agent/pyproject.toml`
- `examples/python-agent/README.md`
- `examples/python-agent/expected-output.txt`

**What:** Mirror of F-12 in Python.

**Acceptance criteria + QA:** Equivalent shape. See F-12.

---

## [T-2-1] KH Builder Feedback — file 5+ GitHub issues

**Owner:** Daniel | **Effort:** 2h | **Depends:** F-9 + F-10 (must have used SDKs to find friction)

**Files:** none in repo
**External:** github.com/keeperhub-protocol/* repos

**What:**
File 5+ specific actionable GitHub issues against KeeperHub repos. Each with:
- Concrete reproduction
- Expected vs actual
- Screenshot/log if applicable
- Tag @luca-keeperhub or maintainer

Topics drawn from real friction Daniel hit during today's live integration:
1. `wfb_*` vs `kh_*` token issuance UX (User tab vs Organisation tab; webhook rejects `kh_*`)
2. Webhook URL discoverability (must assemble `https://app.keeperhub.com/api/workflows/<id>/webhook` by hand)
3. Documentation gap on response shape (`executionId` field guarantees)
4. Rate limit headers (do they exist? what's the limit?)
5. Idempotency support on KH side (does KH dedupe envelopes by some key?)
6. Optional: MCP tool surface for cross-verification (`keeperhub.lookup_execution`)

**Acceptance criteria:**
- [ ] 5+ issues filed with detailed reproduction
- [ ] Each issue tagged with relevant maintainer
- [ ] Issue URLs collected in `FEEDBACK.md` "Issues filed" section
- [ ] Submit to KH Builder Feedback bounty form referencing all 5

**QA Test Plan (Heidi):**
- [ ] Each issue publicly accessible
- [ ] Each has reproduction section
- [ ] Each has maintainer tagged

---

## [T-2-2] FEEDBACK.md expanded with concrete pain points

**Owner:** Daniel | **Effort:** 1h | **Depends:** T-2-1

**Files:** `FEEDBACK.md`

**What:**
Append to existing FEEDBACK.md:
- Section "Concrete pain points hit during live integration" — 5+ bullets matching GitHub issues
- Section "Issues filed" — links each
- Section "Suggested fixes" — proposed shape (engineering-helpful, not complaints)

**Acceptance criteria:**
- [ ] FEEDBACK.md has all 3 new sections
- [ ] Each pain point has reproduction matching issue
- [ ] Suggested fix actionable (1-2 sentence proposal)

---

## Phase 1 done condition

All 16 tickets merged + Phase 1 exit gate green (see `08-exit-gates.md`). T-2-1 + T-2-2 submitted to KH Builder Feedback. **$250 locked.**

Move to Phase 2.
