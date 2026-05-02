# SBO3L × KeeperHub — submission one-pager (v1.0.1)

> **KeeperHub executes. SBO3L proves the execution was authorised.**
> **Audience:** KeeperHub team + ETHGlobal judges.
> See engineering deep-dive at [`docs/keeperhub-integration-paths.md`](../../keeperhub-integration-paths.md).

## Try it now (60 seconds)

```bash
cargo install sbo3l-cli --version 1.0.1
sbo3l serve --db /tmp/sbo3l-kh.db &
sleep 2

# Submit an APRP with a KH workflow id; SBO3L gates, signs, audits, then forwards.
SBO3L_KEEPERHUB_TOKEN="$KH_WFB_TOKEN" \
SBO3L_KEEPERHUB_WORKFLOW_ID="$KH_WORKFLOW_ID" \
sbo3l passport run /path/to/aprp.json \
  --executor keeperhub --mode live \
  --out /tmp/capsule-kh.json
sbo3l passport verify --strict --path /tmp/capsule-kh.json
# expect: PASSED with the executionId from KH echoed in execution.live_evidence
```

## What you get on the wire

KeeperHub workflow webhook receives this body (IP-1):

```json
{
  "agent_id": "research-agent-01",
  "intent": "swap",
  "sbo3l_request_hash": "5a46c8…",
  "sbo3l_policy_hash": "e044f1…",
  "sbo3l_receipt_signature": "8ae170…",
  "sbo3l_audit_event_id": "evt-01HXX…",
  "sbo3l_passport_uri": "https://sbo3l.dev/proof?capsule=…"
}
```

Five optional string fields. Echo them back on lookup → every execution row becomes cryptographically linked to the upstream authorisation.

## What's shipped in v1.0.1

- `sbo3l-keeperhub-adapter` v1.0.1 on crates.io (standalone — depend on it directly without pulling the rest of SBO3L)
- `local_mock()` and `live_from_env()` constructors (mock is CI-safe default)
- IP-1 envelope helper, IP-2 JSON Schema (`schemas/`), IP-3 MCP tool (`sbo3l.audit_lookup`), IP-4 standalone crate, IP-5 capsule URI
- Real `wfb_…` token live integration; demo gate captures `kh-<ULID>` execution id

## Why the pairing is natural

KeeperHub records *what was executed*. SBO3L records *why it was authorised*. Neither layer needs to absorb the other's responsibility. The IP-1 envelope is the bridge — and SBO3L produces every byte of it for free as part of its existing pipeline.

## What we'd love from KeeperHub (Builder Feedback bounty)

See [`FEEDBACK.md`](../../../FEEDBACK.md) and the 5+ KH GitHub issues filed under T-2-1.

## Crates / packages

| Surface | Install | Verify |
|---|---|---|
| Adapter crate | `cargo add sbo3l-keeperhub-adapter@1.0.1` | https://crates.io/crates/sbo3l-keeperhub-adapter |
| TS SDK | `npm install @sbo3l/sdk` | https://www.npmjs.com/package/@sbo3l/sdk |
| Py SDK | `pip install sbo3l-sdk` | https://pypi.org/project/sbo3l-sdk/ |
