# Phase Exit Gates

> Cannot enter next phase until current exit gate is green. Heidi runs all gates literally; Daniel signs off.

---

## Phase 1 Exit Gate (Day 30 target)

### Tickets that MUST be merged

- [ ] F-1 (Real auth middleware)
- [ ] F-2 (Persistent budget store)
- [ ] F-3 (Idempotency atomicity)
- [ ] F-4 (Public-bind safety gate)
- [ ] F-5 (KMS abstraction with backends)
- [ ] F-6 (Self-contained Passport capsule v2)
- [ ] F-7 (Dockerfile)
- [ ] F-8 (docker-compose.yml)
- [ ] F-9 (TypeScript SDK published to npm)
- [ ] F-10 (Python SDK published to PyPI)
- [ ] F-11 (crates.io publishable chain — all 9 crates published)
- [ ] F-12 (examples/typescript-agent)
- [ ] F-13 (examples/python-agent)
- [ ] T-2-1 (5+ KH GitHub issues filed)
- [ ] T-2-2 (FEEDBACK.md expanded)

### Test commands Heidi runs (all must pass)

```bash
set -e

# Baseline regression (existing 377/377)
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo audit --no-fetch --deny warnings
python3 scripts/validate_schemas.py
python3 scripts/validate_openapi.py
python3 demo-fixtures/test_fixtures.py
python3 trust-badge/test_build.py
python3 operator-console/build.py --input operator-console/fixtures/operator-summary.json --evidence operator-console/fixtures/operator-evidence.json
python3 operator-console/test_build.py
bash demo-scripts/run-openagents-final.sh
bash demo-scripts/run-production-shaped-mock.sh

# F-1: Auth middleware
SBO3L_BEARER_TOKEN_HASH=$(htpasswd -nbB testuser sekret123 | cut -d: -f2) \
  cargo run --bin sbo3l-server > /tmp/auth-test.log 2>&1 &
sleep 3
RC=$(curl -sw "%{http_code}" -o /dev/null :8730/v1/payment-requests -X POST \
  -H "Authorization: Bearer sekret123" -H "Content-Type: application/json" \
  -d @test-corpus/aprp/golden_001_minimal.json)
[ "$RC" = "200" ] || (echo "F-1 FAIL"; exit 1)
RC=$(curl -sw "%{http_code}" -o /dev/null :8730/v1/payment-requests -X POST \
  -H "Content-Type: application/json" -d @test-corpus/aprp/golden_001_minimal.json)
[ "$RC" = "401" ] || (echo "F-1 unauth FAIL"; exit 1)
pkill sbo3l-server

# F-2: Budget persistence
rm -f /tmp/budget-exit.db
SBO3L_DB=/tmp/budget-exit.db SBO3L_ALLOW_UNAUTHENTICATED=1 \
  cargo run --bin sbo3l-server > /dev/null 2>&1 &
sleep 2
PAYLOAD=$(jq '.amount.value = "0.05" | .nonce = "01HTAWX5K3R8YV9NQB7C6P2D81"' test-corpus/aprp/golden_001_minimal.json)
curl -s :8730/v1/payment-requests -X POST -H "Content-Type: application/json" -d "$PAYLOAD" >/dev/null
pkill sbo3l-server
sleep 1
SBO3L_DB=/tmp/budget-exit.db SBO3L_ALLOW_UNAUTHENTICATED=1 \
  cargo run --bin sbo3l-server > /dev/null 2>&1 &
sleep 2
PAYLOAD=$(jq '.amount.value = "0.06" | .nonce = "01HTAWX5K3R8YV9NQB7C6P2D82"' test-corpus/aprp/golden_001_minimal.json)
DENY=$(curl -s :8730/v1/payment-requests -X POST -H "Content-Type: application/json" -d "$PAYLOAD" | jq -r .deny_code)
[ "$DENY" = "policy.budget_exceeded" ] || (echo "F-2 FAIL"; exit 1)
pkill sbo3l-server

# F-3: Idempotency race
KEY="01TESTRACE9UNIQUE16chars0"
PAYLOAD=$(jq '.nonce = "01HTAWX5K3R8YV9NQB7C6P2D83"' test-corpus/aprp/golden_001_minimal.json)
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server > /dev/null 2>&1 &
sleep 2
for i in {1..50}; do
  curl -s -o /dev/null -w "%{http_code}\n" :8730/v1/payment-requests -X POST \
    -H "Idempotency-Key: $KEY" -H "Content-Type: application/json" -d "$PAYLOAD" &
done
wait
# (Distribution check is intent-level; concrete test in cargo test --test test_idempotency_race)
pkill sbo3l-server

# F-4: Public-bind safety
SBO3L_LISTEN=0.0.0.0:18730 ./target/debug/sbo3l-server 2>&1 | grep -q "unsafe public bind"
[ $? -eq 0 ] || (echo "F-4 FAIL"; exit 1)

# F-6: Capsule v2 self-contained
cargo run -p sbo3l-cli -- passport run \
  test-corpus/aprp/golden_001_minimal.json \
  --db /tmp/v2-exit.db --agent research-agent.team.eth --resolver offline-fixture \
  --ens-fixture demo-fixtures/ens-records.json --executor keeperhub --mode mock \
  --out /tmp/capsule-v2-exit.json
RESULT=$(cargo run -p sbo3l-cli -- passport verify --strict --path /tmp/capsule-v2-exit.json 2>&1)
echo "$RESULT" | grep -q "PASSED" && ! echo "$RESULT" | grep -q "SKIPPED" || (echo "F-6 self-contained FAIL"; exit 1)

# F-7 + F-8: Docker
docker build -t sbo3l/server:exit-gate .
SIZE=$(docker images sbo3l/server:exit-gate --format '{{.Size}}' | grep -oE '[0-9]+(\.[0-9]+)?[MG]B')
echo "image size: $SIZE"
docker compose up sbo3l -d
sleep 5
curl -s :8730/v1/payment-requests -X POST -H "Content-Type: application/json" \
  -d @test-corpus/aprp/golden_001_minimal.json | jq -r .decision
docker compose down

# F-9: TS SDK
cd sdks/typescript && npm test && cd -
npm view @sbo3l/sdk versions  # >= 0.1.0

# F-10: Python SDK
cd sdks/python && uv run pytest && cd -
pip index versions sbo3l-sdk  # >= 0.1.0

# F-11: crates.io
for crate in sbo3l-core sbo3l-storage sbo3l-policy sbo3l-identity \
             sbo3l-execution sbo3l-keeperhub-adapter sbo3l-server \
             sbo3l-mcp sbo3l-cli; do
  cargo search $crate | grep -q "^$crate = "
done

# F-12: TS example
cd examples/typescript-agent && npm install && \
  SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server > /dev/null 2>&1 &
sleep 3
node --import tsx ./src/index.ts | grep -E "^(decision|execution_ref):"
pkill sbo3l-server
cd -

# F-13: Python example
cd examples/python-agent && pip install -r requirements.txt && \
  SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server > /dev/null 2>&1 &
sleep 3
python main.py | grep -E "^(decision|execution_ref):"
pkill sbo3l-server
cd -

# T-2-1, T-2-2: KH Builder Feedback
[ -f FEEDBACK.md ] && grep -q "Concrete pain points hit during live integration" FEEDBACK.md
grep -c "github.com/keeperhub" FEEDBACK.md  # >= 5

echo "✅ Phase 1 exit gate green"
```

### Bounty status check

- [ ] T-2-1: 5+ GitHub issues filed (URLs in FEEDBACK.md)
- [ ] T-2-2: FEEDBACK.md updated
- [ ] **KH Builder Feedback bounty form submitted** (Daniel manual confirmation)
- [ ] **$250 secured (or in queue for award)**

### Sign-off

- [ ] Heidi: all test commands green
- [ ] Daniel: identity sub-claims preserved, no security regressions

If green: **Phase 2 unlocked. Tickets T-3-1, F-11 (if not already), CTI-3-1 can start.**

---

## Phase 2 Exit Gate (Day 60 target)

### Tickets that MUST be merged

All 26 Phase 2 tickets:
- [ ] T-3-1 through T-3-7 (ENS Most Creative track)
- [ ] T-4-1 through T-4-3 (ENS AI Agents track)
- [ ] T-5-1 through T-5-6 (Uniswap track)
- [ ] T-1-1 through T-1-6 (6 framework integrations)
- [ ] CTI-3-1 through CTI-3-4 (sbo3l.dev surface)

### Test commands

```bash
set -e

# Phase 1 baseline regression
# (re-run all Phase 1 exit gate tests; must still pass)

# T-3-1: Durin issuance
SBO3L_ENS_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com \
cargo run -p sbo3l-cli -- agent register \
  --name exit-gate-test-$(date +%s) \
  --parent sbo3l.eth \
  --network sepolia \
  --records '{"sbo3l:agent_id":"exit-gate"}' \
  --dry-run
# expect: prints tx data

# T-3-3: Agent fleet on Sepolia
for name in research-agent trading-agent swap-agent audit-agent coordinator-agent; do
  SBO3L_ENS_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com \
  cargo run -p sbo3l-cli -- passport resolve ${name}.sbo3l.eth | grep -q "policy hash:" || \
    (echo "T-3-3 FAIL for $name"; exit 1)
done

# T-3-4: Cross-agent verification
ATT=$(cargo run -p sbo3l-cli -- cross-agent attest \
  --from research-agent.sbo3l.eth --to trading-agent.sbo3l.eth \
  --intent delegate_swap --signer-key /tmp/research-key.pem --expires-in 1h)
PAYLOAD=$(jq --argjson att "$ATT" '. + {"cross_agent_attestation": $att}' test-corpus/aprp/golden_001_minimal.json)
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server > /dev/null 2>&1 &
sleep 2
curl -s :8730/v1/payment-requests -X POST -H "Content-Type: application/json" -d "$PAYLOAD" | jq -r .decision
pkill sbo3l-server

# T-3-5: trust-dns viz live
curl -sf https://app.sbo3l.dev/trust-dns | grep -q "trust-dns"

# T-3-6: Trust DNS essay published
curl -sf https://docs.sbo3l.dev/trust-dns | grep -q "Trust DNS"

# T-5-5: Real Sepolia swap
SBO3L_UNISWAP_TRADING_API_KEY=$(cat /tmp/uniswap-key) \
SBO3L_UNISWAP_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com \
SBO3L_UNISWAP_PRIVATE_KEY=$(cat /tmp/sepolia-key) \
bash demo-scripts/sponsors/uniswap-real-swap.sh
TX_HASH=$(jq -r .execution.executor_evidence.tx_hash demo-scripts/artifacts/uniswap-real-swap-capsule.json)
[ -n "$TX_HASH" ] || (echo "T-5-5 no tx hash"; exit 1)

# T-1-1..T-1-6: framework integrations
for fw in langchain-typescript autogen elizaos; do
  cd integrations/$fw && npm test && cd -
done
for fw in langchain-python crewai llamaindex; do
  cd integrations/$fw && uv run pytest && cd -
done

# CTI-3-2: Marketing site
curl -sf https://sbo3l.dev | grep -q "Don't give your agent a wallet"

# CTI-3-3: Docs site
curl -sf https://docs.sbo3l.dev | grep -q "Quickstart"

# CTI-3-4: Hosted preview
curl -sf https://app.sbo3l.dev | grep -q "Login"

echo "✅ Phase 2 exit gate green"
```

### Bounty status check

- [ ] T-3-7: ENS Most Creative submission packaged (`submissions/ens-most-creative/submission.md`)
- [ ] Uniswap Best API submission packaged (`submissions/uniswap-best-api/submission.md`)
- [ ] Both submitted to ETHGlobal track forms (Daniel manual)

### Sign-off

- [ ] Heidi: all green
- [ ] Daniel: submissions reviewed before form submit

If green: **Phase 3 unlocked.**

---

## Phase 3 Exit Gate (Day 100 target — submission deadline)

### Tickets that MUST be merged

All 22 Phase 3 tickets.

### Test commands

```bash
set -e

# All Phase 1 + Phase 2 baselines re-pass

# T-1-7: KH 100+ executions
LINES=$(grep -c "^kh-" demo-scripts/kh-fleet-execution-log.md)
[ $LINES -ge 100 ] || (echo "T-1-7 only $LINES executions"; exit 1)

# T-6-1: 0G Storage capsule
SBO3L_0G_STORAGE_ENDPOINT=$(cat /tmp/0g-storage-endpoint) \
cargo run -p sbo3l-cli -- passport run ... --storage 0g --out /tmp/capsule-0g.json
ROOT_HASH=$(jq -r .passport_uri /tmp/capsule-0g.json | sed 's|0g://||')
[ -n "$ROOT_HASH" ] || (echo "T-6-1 FAIL"; exit 1)

# T-7-1: Swarm running
docker compose -f apps/swarm/docker-compose.yml up -d
sleep 10
bash apps/swarm/test-coordination.sh
docker compose -f apps/swarm/docker-compose.yml down

# T-8-3: Multi-node AXL
docker compose -f docker-compose.multi-node.yml up -d
sleep 5
bash demo-scripts/cross-node-axl-demo.sh
docker compose -f docker-compose.multi-node.yml down

# CTI-4-1: Golden vertical
bash examples/golden-vertical/run.sh > /tmp/gv-output.txt
diff <(grep -E "^(STEP|✅)" /tmp/gv-output.txt) <(grep -E "^(STEP|✅)" examples/golden-vertical/expected-transcript.txt)

# CTI-4-3: proof site v2
curl -sf https://sbo3l.dev/proof | grep -q "Passport capsule"

# CTI-4-5: 8 submissions packaged
for track in keeperhub-best-use keeperhub-builder-feedback ens-most-creative \
             ens-ai-agents uniswap-best-api 0g-track-a 0g-track-b gensyn-axl; do
  test -f submissions/$track/submission.md || (echo "missing $track"; exit 1)
  test -f submissions/$track/demo-video.url || (echo "missing $track video"; exit 1)
done

echo "✅ Phase 3 exit gate green — ready to submit"
```

### Final submission check

- [ ] All 8 ETHGlobal track forms submitted (Daniel)
- [ ] Master demo video URL in each form (CTI-4-2)
- [ ] Live demo links working (sbo3l.dev, app.sbo3l.dev, docs.sbo3l.dev)
- [ ] EIP draft submitted to ethereum/EIPs (CTI-4-4)
- [ ] OSS launch posts up (CTI-4-7)
- [ ] Sponsor outreach DMs sent (CTI-4-6)
- [ ] All KH live executions logged
- [ ] All ENS records on chain
- [ ] All Sepolia swaps confirmed on Etherscan

### Sign-off

- [ ] Heidi: all green, no flake
- [ ] Daniel: every track submission reviewed; he believes it's win-shaped

**Pencils down. Submission complete. Wait for results.**

---

## Reset / failure handling

If a phase exit gate fails:
1. Heidi posts failure log in `#sbo3l-coordination`
2. Daniel arbitrates: hot-fix forward (if < 4h work) or rollback (if structural)
3. New tickets opened for any structural fixes
4. Re-run gate after fixes merge

If Phase exit gate fails > 2x in a row:
1. Daniel pauses all per-track work
2. All-hands focus on stabilizing the gate
3. Until green, no Phase N+1 work happens

If overall timeline slips:
1. Phase 3 has 30-day buffer (Day 70 ideal completion, Day 100 deadline)
2. If Phase 1 slips past Day 30, scope-cut Phase 2 (drop T-1-3 LlamaIndex, T-1-4 AutoGen first)
3. If Phase 2 slips past Day 60, scope-cut Phase 3 (drop T-8 Gensyn AXL first, then T-6 0G Track A)
4. **Never cut foundation (F-*) or core track tickets** — those are the wins
