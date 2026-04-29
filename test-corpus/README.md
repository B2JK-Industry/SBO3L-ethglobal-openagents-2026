# SBO3L Test Corpus

This directory is normative input for implementation and CI.

## Rules

- Files under `golden_*` must validate against their schema.
- Files under `adversarial_*` must be rejected with the exact error code documented in `17_interface_contracts.md`.
- Hash fixtures are locked only after the first implementation computes RFC 8785/JCS output with `serde_json_canonicalizer`.
- Do not change existing corpus files silently. Add a new numbered fixture when behavior changes.

## Initial Acceptance Set

| Path | Purpose | Expected result |
|---|---|---|
| `aprp/golden_001_minimal.json` | Minimal valid APRP x402 request | accepted |
| `aprp/adversarial_unknown_field.json` | Strict schema guard | `schema.unknown_field` |
| `aprp/deny_prompt_injection_request.json` | Open Agents hero attack request | valid schema, policy denies with `policy.deny_recipient_not_allowlisted` |
| `policy/reference_low_risk.json` | Reference low-risk policy | accepted by policy linter |
| `x402/challenge_001.json` | Minimal x402 challenge | accepted |
| `audit/chain_v1.jsonl` | Three-event audit chain shape | accepted after implementation locks hashes/signatures |

