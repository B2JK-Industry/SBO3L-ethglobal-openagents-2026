# Mandate — 3:30 demo video script

Target: **3:30**. Hard stop: **3:50**. 720p+, real human voice, no AI TTS, no music in place of narration.

| t | Speaker | Visual | Notes |
|---|---|---|---|
| 0:00–0:15 | "Don't give your agent a wallet. Give it a mandate. Mandate is a local policy, budget, receipt and audit firewall that keeps autonomous AI agents from spending in ways they shouldn't." | Title card + tagline | Keep the intro under 15s. |
| 0:15–0:35 | "Every Mandate agent has a public ENS identity. Here is `research-agent.team.eth`. Notice the published `mandate:policy_hash` matches Mandate's active policy hash — if they ever drift, the agent is treated as un-trustable." | `bash demo-scripts/sponsors/ens-agent-identity.sh` | Highlight the `ens.verify: ok (matches active policy …)` line. |
| 0:35–1:10 | "The agent receives a legitimate task: buy a small API call. It emits a payment request. Mandate decides — `auto_approved` — and signs a policy receipt. The audit log records the decision." | `./demo-agents/research-agent/run --scenario legit-x402` | Pause on the `decision: Allow`, `request_hash`, `policy_hash`, `audit_event`, `receipt_sig` block. |
| 1:10–1:45 | "Approved decisions route to KeeperHub. The execution_ref appears, tied back to the policy receipt." | `bash demo-scripts/sponsors/keeperhub-guarded-execution.sh` (just the allow path) | Linger on `kh-<ULID>`. |
| 1:45–2:20 | "Same agent, same Mandate. We hand it a hostile attached document that tells it to send 10 USDC to an attacker address. The agent complies." | Show `attack_prompt` line in the prompt-injection scenario output | Make the injection visible — judges should *see* the attack text. |
| 2:20–2:55 | "Mandate denies before any signer or executor runs. Deny code: `policy.deny_recipient_not_allowlisted` (or `deny_unknown_provider`). KeeperHub refuses the denied receipt." | `./demo-agents/research-agent/run --scenario prompt-injection --execute-keeperhub` | Linger on `decision: Deny` + `keeperhub.refused`. |
| 2:55–3:20 | "Same boundary works for Uniswap. A bounded USDC→ETH swap is allowed; an attacker quote into a rug-token with extreme slippage to a denied recipient is rejected — by both the swap-policy guard *and* Mandate." | `bash demo-scripts/sponsors/uniswap-guarded-swap.sh` | Show the FAIL lines on the deny path + `uniswap.refused`. |
| 3:20–3:40 | "Audit chain end-to-end. Three events linked, all signed. Tamper with one byte and the verifier rejects." | The orchestrator's step 11 output | Show `strict-hash verify rejected the tampered audit event`. |
| 3:40–3:50 | "Don't give your agent a wallet. Give it a mandate." | Title card | Done. |

## Recording checklist

- [ ] 720p+ (1080p preferred), real screen recorder, no phone.
- [ ] Real human voiceover. No AI TTS, no music-only segments.
- [ ] Do not speed up terminal output. If pacing is tight, edit out long waits.
- [ ] Reset state with `bash demo-scripts/reset.sh` before recording so any persistent state starts fresh.
- [ ] Record commit hash on the title card so judges can reproduce.
- [ ] If targeting a specific partner prize, anchor the relevant section to ≥ 30s.
