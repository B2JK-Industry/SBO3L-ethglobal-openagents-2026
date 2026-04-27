# Builder Feedback

Notes for partner sponsors during the ETHGlobal Open Agents 2026 build of **Mandate**. This file is required for some partner prizes (e.g. Uniswap API integration) and is offered to all selected partners.

## KeeperHub

To be filled in after KeeperHub adapter integration.

- What worked:
- What was unclear:
- Suggested improvements:

## ENS

To be filled in after ENS identity adapter integration.

- What worked:
- What was unclear:
- Suggested improvements:

## Uniswap (stretch)

To be filled in after the guarded-swap adapter is wired against the Uniswap API.

- What worked:
- What was unclear:
- Suggested improvements:

## General

- The agent-identity → policy-hash → audit-root pattern via ENS text records felt natural; partners that resolve ENS metadata should consider standardising the `mandate:*` keys.
- Sponsor adapters benefit from a clean separation between "decide" (policy) and "execute" (sponsor). This is the architectural angle Mandate wants to validate.
