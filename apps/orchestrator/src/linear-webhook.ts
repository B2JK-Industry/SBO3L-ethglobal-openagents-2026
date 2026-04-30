import type { LinearClient } from "./linear-client.js";
import type { AgentTransport } from "./agent-bridge.js";
import { postCoordinationStatus } from "./agent-bridge.js";
import { inferPhase, renderAgentPrompt } from "./render-prompt.js";
import { isSlot, loadSlotConfig } from "./slot-mapping.js";
import type { HandleOutcome, LinearWebhookEvent, Slot } from "./types.js";

export interface HandlerDeps {
  linear: LinearClient;
  transport: AgentTransport;
  env: NodeJS.ProcessEnv;
  /** Optional override for the coordination Discord webhook (tests inject). */
  coordinationWebhookUrl?: string | undefined;
  /** Injectable fetch used for coordination posts. */
  fetchImpl?: typeof fetch;
}

/**
 * Core webhook handler. Pure of HTTP concerns — takes a parsed event and
 * returns a structured outcome. The Vercel `api/linear-webhook.ts` entry
 * point handles signature verification + body parsing then calls this.
 *
 * Reaction matrix:
 *   - non-Issue events            → ignored
 *   - non-update actions          → ignored
 *   - Issue not transitioned to a `completed` state → ignored
 *   - completed Issue, no recognised assignee       → ignored (Daniel manual)
 *   - completed Issue, slot queue empty             → coordination post, no dispatch
 *   - completed Issue, next ticket found            → render prompt, deliver, mark in_progress
 */
export async function handleLinearWebhook(
  event: LinearWebhookEvent,
  deps: HandlerDeps,
): Promise<HandleOutcome> {
  if (event.type !== "Issue") {
    return { kind: "ignored", reason: `non-issue event type=${event.type}` };
  }
  if (event.action !== "update") {
    return { kind: "ignored", reason: `non-update action=${event.action}` };
  }
  if (event.data.state.type !== "completed") {
    return {
      kind: "ignored",
      reason: `state.type=${event.data.state.type} (not completed)`,
    };
  }

  const assigneeName = event.data.assignee?.name;
  if (!isSlot(assigneeName)) {
    return {
      kind: "ignored",
      reason: `assignee not a 4+1 slot: ${assigneeName ?? "<unassigned>"}`,
    };
  }
  const slot: Slot = assigneeName;

  const next = await deps.linear.nextTicketForSlot(slot);
  if (!next) {
    await postCoordinationStatus(
      `🔔 ${slot} queue empty (just completed ${event.data.identifier}). Waiting for Daniel batch assignment.`,
      deps.coordinationWebhookUrl ?? deps.env["DISCORD_WEBHOOK_COORDINATION_URL"],
      deps.fetchImpl ?? fetch,
    );
    return { kind: "queue_empty", slot };
  }

  const slotConfig = loadSlotConfig(slot, deps.env);
  const prompt = renderAgentPrompt({
    slot,
    branchSlug: slotConfig.branchSlug,
    ticketIdentifier: next.identifier,
    ticketTitle: next.title,
    phase: inferPhase(next.identifier),
  });

  await deps.transport.send(slot, prompt, slotConfig);
  await deps.linear.markInProgress(next.id);

  return { kind: "dispatched", slot, nextTicket: next.identifier };
}
