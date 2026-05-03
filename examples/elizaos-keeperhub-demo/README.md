# `elizaos-keeperhub-demo`

End-to-end ElizaOS demo for `@sbo3l/elizaos-keeperhub`. A hardcoded chat turn
(no LLM in the loop) emits an APRP → the `SBO3L_KEEPERHUB_PAYMENT_REQUEST`
Action validates + dispatches it → SBO3L decides → on allow, the daemon's
KeeperHub adapter fires workflow `m4t4cnpmhv8qquce3bv3c` and surfaces the
captured `executionId` as `kh_execution_ref`.

## Run

Pre-req: an SBO3L daemon running at `http://localhost:8730` (see top-level
README for `sbo3l-server` boot).

```bash
npm install
npm run agent
```

Set `SBO3L_ENDPOINT` to point at a remote daemon.

## Expected output (allow path)

```
▶ daemon: http://localhost:8730
▶ KH workflow target = m4t4cnpmhv8qquce3bv3c

▶ Action: SBO3L_KEEPERHUB_PAYMENT_REQUEST
  similes: PAY_VIA_KEEPERHUB, PURCHASE_VIA_KEEPERHUB, SUBMIT_KH_PAYMENT, REQUEST_KH_PAYMENT

  ✓ validate passed

=== envelope ===
  decision: "allow"
  kh_workflow_id_advisory: "m4t4cnpmhv8qquce3bv3c"
  kh_execution_ref: "kh-01HTAWX5..."
  audit_event_id: "evt-..."
  request_hash: "..."
  policy_hash: "..."
  matched_rule_id: "..."
  deny_code: null

✓ allow + KH executed → kh_execution_ref=kh-01HTAWX5...
  audit_event_id: evt-...
```

## License

MIT
