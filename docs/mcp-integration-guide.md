# SBO3L MCP — judge-facing integration guide

> **MCP turns SBO3L from a daemon into infrastructure other agents can call.**

[Model Context Protocol](https://modelcontextprotocol.io/) is the open spec by which agents (Claude, Cursor, custom MCP clients) call out to tool servers. SBO3L ships an MCP stdio JSON-RPC 2.0 server, [`sbo3l-mcp`](../crates/sbo3l-mcp/), so any agent can ask SBO3L to validate an APRP, decide allow/deny, run a guarded execution, verify a Passport capsule, or **look up the audit bundle behind a previous decision** — without touching SQLite, the daemon, or the Rust API directly. Full wire reference in [`docs/cli/mcp.md`](cli/mcp.md).

## Per-tool example — `sbo3l.audit_lookup`

The IP-3 sister tool to `keeperhub.lookup_execution`. Given an audit-event id + receipt + signer pubkeys, it returns a verifiable `sbo3l.audit_bundle.v1`. Request shape (canonical, from `crates/sbo3l-mcp/tests/jsonrpc_integration.rs:330`):

```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "sbo3l.audit_lookup",
  "params": {
    "audit_event_id": "evt-01KQCPH41YBJSKGDBDQFT7XM9Y",
    "db": "/path/to/sbo3l.db",
    "receipt": { /* PolicyReceipt JSON */ },
    "receipt_pubkey": "<hex>",
    "audit_pubkey":   "<hex>"
  }
}
```

Response — `bundle.summary` excerpt, copied verbatim from `demo-scripts/artifacts/mcp-transcript.json` (regenerate with the run command below):

```json
{
  "audit_event_id":     "evt-01KQCPH41YBJSKGDBDQFT7XM9Y",
  "decision":           "allow",
  "policy_hash":        "e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf",
  "request_hash":       "835469d807bb8278a0851f98ffc909b246cb44661ba3c61a32fbce59a4848ae4",
  "audit_event_hash":   "6f0544efb052806d24cbb231cbabd0ed190c621e7908a2088dcbdedb01d49bb9",
  "audit_chain_root":   "6f0544efb052806d24cbb231cbabd0ed190c621e7908a2088dcbdedb01d49bb9",
  "audit_chain_latest": "6f0544efb052806d24cbb231cbabd0ed190c621e7908a2088dcbdedb01d49bb9"
}
```

The full response (`result.bundle`) is a complete `sbo3l.audit_bundle.v1` artefact — receipt, audit chain segment, signer pubkeys — so an MCP client can re-verify offline without a second tool call.

## End-to-end auditor query (sequence)

Pairing the two tools answers a single auditor question — *"is this KeeperHub `executionId` linked to a SBO3L-authorised decision?"* — in two MCP calls:

```text
auditor (MCP client)
   │
   │  1.  keeperhub.lookup_execution(execution_id)         ── target, KH side ──▶
   │                                                       ◀── status + run-log + echoed
   │                                                          sbo3l_audit_event_id
   │
   │  2.  sbo3l.audit_lookup(audit_event_id, …)          ── exists today ──▶
   │                                                       ◀── sbo3l.audit_bundle.v1
   │
   ▼
re-verify offline:
  receipt signature ↔ request_hash ↔ policy_hash ↔ audit chain prefix
```

SBO3L exposes the right-hand tool today (PR #46). The left-hand tool is **target on the KeeperHub side** — see [`docs/keeperhub-integration-paths.md` §IP-3](keeperhub-integration-paths.md#ip-3--keeperhublookup_executionexecution_id-mcp-tool) for the proposed schema and adoption shape. **SBO3L does not call a KeeperHub MCP server in this build.**

## Run it locally

```bash
bash demo-scripts/sponsors/mcp-passport.sh
```

Drives the freshly-built `sbo3l-mcp` server over a stdin/stdout pipe, exercises every shipping tool (`tools/list`, `sbo3l.validate_aprp`, `sbo3l.decide` allow + deny, `sbo3l.run_guarded_execution`, `sbo3l.verify_capsule`, `sbo3l.audit_lookup`), and writes a transcript to `demo-scripts/artifacts/mcp-transcript.json`. Exit 0 iff every step returned its expected result. Not in the 13-gate `run-openagents-final.sh`; standalone sponsor surface.

## See also

- Full wire reference: [`docs/cli/mcp.md`](cli/mcp.md)
- Strategic positioning: [`docs/keeperhub-integration-paths.md` §IP-3](keeperhub-integration-paths.md#ip-3--keeperhublookup_executionexecution_id-mcp-tool)
- Source: [`crates/sbo3l-mcp/`](../crates/sbo3l-mcp/), tests at [`crates/sbo3l-mcp/tests/jsonrpc_integration.rs`](../crates/sbo3l-mcp/tests/jsonrpc_integration.rs).
