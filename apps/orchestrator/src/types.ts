/**
 * Linear webhook event subset that the orchestrator consumes.
 *
 * The Linear webhook payload is large; we only model the fields the handler
 * actually reads. See https://developers.linear.app/docs/graphql/webhooks for
 * the full schema.
 */

/** Linear assignee → 4+1 slot. The Linear assignee.name MUST match one of these. */
export type Slot = "Dev 1" | "Dev 2" | "Dev 3" | "Dev 4" | "QA + Release";

/** Recognised Linear workflow state types we care about. */
export type LinearStateType =
  | "triage"
  | "backlog"
  | "unstarted"
  | "started"
  | "completed"
  | "canceled";

export interface LinearLabel {
  name: string;
}

export interface LinearAssignee {
  id: string;
  name: string;
}

export interface LinearState {
  id: string;
  name: string;
  type: LinearStateType;
}

/** Issue payload as it appears inside webhook events and SDK queries. */
export interface LinearIssue {
  id: string;
  identifier: string;
  title: string;
  /** Linear's numeric priority: 0 (none) → 1 (urgent) → 4 (low). */
  priority: number;
  state: LinearState;
  assignee?: LinearAssignee | undefined;
  labels?: LinearLabel[] | undefined;
}

/** The subset of Linear webhook envelope the orchestrator inspects. */
export interface LinearWebhookEvent {
  /** "create" | "update" | "remove" — we only react to "update". */
  action: string;
  /** "Issue" | "Comment" | ... — we only react to "Issue". */
  type: string;
  data: LinearIssue;
}

/** Resolved configuration for one slot (Dev 1 etc.) — set via env. */
export interface SlotConfig {
  /** Discord webhook URL for prompt delivery. */
  discordWebhookUrl: string;
  /** kebab form used in branch names: "Dev 1" → "dev1". */
  branchSlug: string;
}

/** Outcome record returned by the handler — useful for tests + logs. */
export type HandleOutcome =
  | { kind: "ignored"; reason: string }
  | { kind: "queue_empty"; slot: Slot }
  | { kind: "dispatched"; slot: Slot; nextTicket: string };

export interface RenderPromptInput {
  slot: Slot;
  branchSlug: string;
  ticketIdentifier: string;
  ticketTitle: string;
  /** Phase number 1-3 — derived from the ticket prefix (F-/T-2-/T-3-...). */
  phase: 1 | 2 | 3;
}
