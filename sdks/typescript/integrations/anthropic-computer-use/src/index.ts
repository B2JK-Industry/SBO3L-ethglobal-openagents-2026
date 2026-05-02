/**
 * `@sbo3l/anthropic-computer-use` — SBO3L policy gate for Claude's
 * computer-use / bash / text_editor tool actions.
 *
 * Where `@sbo3l/anthropic` (T-1-10) wraps a single function-tool, this
 * package gates the *built-in* tools Claude calls when given access to
 * the desktop:
 *
 *   - `computer` (computer_20241022, computer_20250124) — mouse/keyboard
 *   - `bash` (bash_20250124, bash_20241022) — shell commands
 *   - `text_editor` (str_replace_editor / text_editor_20250124) — file edits
 *
 * Every emitted `tool_use` block flows through `gateComputerAction(...)`
 * BEFORE the consumer's executor runs. The gate maps the action into
 * an APRP (with risk classification per action class) and submits to
 * SBO3L. On allow → executor runs. On deny → `tool_result` block
 * returned with the deny envelope so Claude can pick a different
 * approach.
 *
 *   ```ts
 *   import Anthropic from "@anthropic-ai/sdk";
 *   import { SBO3LClient } from "@sbo3l/sdk";
 *   import { gateComputerAction } from "@sbo3l/anthropic-computer-use";
 *
 *   const claude = new Anthropic();
 *   const client = new SBO3LClient({ endpoint: "http://localhost:8730" });
 *
 *   for (const block of response.content) {
 *     if (block.type !== "tool_use") continue;
 *     const result = await gateComputerAction({
 *       sbo3l: client,
 *       block,
 *       agentId: "research-agent-01",
 *       executor: realComputerHandler,  // your existing handler
 *     });
 *     toolResults.push(result);
 *   }
 *   ```
 */

import {
  SBO3LClient,
  SBO3LError,
  type PaymentRequest,
  type PolicyReceipt,
} from "@sbo3l/sdk";

export type { PaymentRequest, PolicyReceipt };

/** A `tool_use` content block as emitted by Claude (subset we need). */
export interface AnthropicToolUseBlock {
  type: "tool_use";
  id: string;
  name: string;
  input: unknown;
}

/** A `tool_result` content block ready to push into the next message turn. */
export interface AnthropicToolResultBlock {
  type: "tool_result";
  tool_use_id: string;
  content: string;
  is_error?: boolean;
}

/**
 * Action class derived from Claude's tool name. Drives the APRP's
 * `risk_class` field — bash and computer-mouse get `high`, text editor
 * and screenshots get `medium`. Operators can override per-call via the
 * `riskClassifier` option.
 */
export type ActionClass =
  | "computer.mouse"
  | "computer.keyboard"
  | "computer.screenshot"
  | "bash.exec"
  | "text_editor.write"
  | "text_editor.read"
  | "unknown";

const DEFAULT_RISK_BY_CLASS: Record<ActionClass, "low" | "medium" | "high" | "critical"> = {
  // Mouse clicks can authorise payment + state-changing UI; bash can do
  // anything. Both default to `high` so a permissive policy must
  // explicitly allow them, not the inverse.
  "computer.mouse": "high",
  "computer.keyboard": "high",
  "bash.exec": "high",
  // Reads are observational. Permissive low-risk by default — operators
  // who care about screen-scraping can override per-call.
  "computer.screenshot": "low",
  "text_editor.read": "low",
  // Writes mutate state but typically scoped to a workspace. Medium.
  "text_editor.write": "medium",
  // Unknown tool → fail-closed by treating as critical so a default-
  // deny policy denies fast.
  unknown: "critical",
};

/**
 * Map a Claude tool_use block into an action class. Recognises the
 * canonical Anthropic tool names plus their dated variants
 * (`computer_20241022`, `bash_20250124`, etc.). Falls through to
 * `unknown` so callers can extend without monkey-patching.
 */
export function classifyAction(block: AnthropicToolUseBlock): ActionClass {
  const n = block.name;
  if (n === "computer" || n.startsWith("computer_")) {
    const input = block.input as { action?: string } | null;
    const action = input?.action ?? "";
    if (action === "screenshot") return "computer.screenshot";
    if (action === "type" || action === "key") return "computer.keyboard";
    return "computer.mouse";
  }
  if (n === "bash" || n.startsWith("bash_")) return "bash.exec";
  if (n === "str_replace_editor" || n.startsWith("text_editor")) {
    const input = block.input as { command?: string } | null;
    const cmd = input?.command ?? "";
    if (cmd === "view") return "text_editor.read";
    return "text_editor.write";
  }
  return "unknown";
}

/**
 * Build a deterministic APRP from a Claude computer-use action.
 * `agent_id` and `task_id` come from the caller; everything else is
 * derived from the action class so the policy gate can branch on
 * `intent` + `risk_class` + `provider_url`.
 */
export interface BuildAprpInput {
  agentId: string;
  taskId?: string;
  block: AnthropicToolUseBlock;
  riskClassifier?: (block: AnthropicToolUseBlock, cls: ActionClass) => "low" | "medium" | "high" | "critical";
  /** Override the synthetic provider URL the gate reports. Default: `urn:anthropic-computer-use:<class>`. */
  providerUrl?: string;
  /** Override expiry. Default: now + 5 minutes. */
  expiry?: string;
}

export function buildAprpFromAction(input: BuildAprpInput): PaymentRequest {
  const cls = classifyAction(input.block);
  const risk = input.riskClassifier?.(input.block, cls) ?? DEFAULT_RISK_BY_CLASS[cls];
  const taskId = input.taskId ?? `cu-${input.block.id}`;
  const providerUrl = input.providerUrl ?? `urn:anthropic-computer-use:${cls}`;
  const expiry = input.expiry ?? new Date(Date.now() + 5 * 60 * 1000).toISOString();
  const nonce = freshNonce();

  // Even a "click" or "bash" action carries no real money; we model it
  // as a zero-amount `pay_compute_job` so the policy DSL's existing
  // shapes apply (intent / risk_class / provider). Operators can write
  // rules that match on `provider_url` (urn:anthropic-computer-use:bash.exec)
  // to permit/deny per action class.
  return {
    agent_id: input.agentId,
    task_id: taskId,
    intent: "pay_compute_job",
    amount: { value: "0", currency: "USD" },
    token: "USDC",
    destination: {
      type: "smart_account",
      address: "0x0000000000000000000000000000000000000000",
    },
    payment_protocol: "smart_account_session",
    chain: "base",
    provider_url: providerUrl,
    expiry,
    nonce,
    risk_class: risk,
  } as PaymentRequest;
}

function freshNonce(): string {
  return globalThis.crypto?.randomUUID?.() ?? `nonce-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

/** Caller-supplied function that actually performs the action on the desktop / shell / FS. */
export type ComputerExecutor = (block: AnthropicToolUseBlock) => Promise<string>;

export interface GateOptions {
  sbo3l: SBO3LClient;
  block: AnthropicToolUseBlock;
  agentId: string;
  /** Caller's existing computer-use handler. Only invoked on `allow`. */
  executor: ComputerExecutor;
  taskId?: string;
  riskClassifier?: (block: AnthropicToolUseBlock, cls: ActionClass) => "low" | "medium" | "high" | "critical";
  /** Optional callback to derive an idempotency key per call. */
  idempotencyKey?: (aprp: PaymentRequest) => string;
}

/**
 * Gate one Claude `tool_use` action through SBO3L. Returns the
 * `tool_result` block ready to push into the next `messages.create`
 * call.
 *
 *   - allow → executor runs; result wrapped as `{is_error: false, content: <executor output>}`
 *   - deny / requires_human → executor SKIPPED; `is_error: true, content: { error, deny_code, audit_event_id, action_class }`
 *   - executor itself throws → `is_error: true, content: { error: 'executor.failed', detail }`
 *   - SBO3L transport fails → `is_error: true, content: { error: 'transport.failed', detail }`
 *
 * The SBO3L receipt's `audit_event_id` is preserved on every code path
 * so post-run audits can correlate.
 */
export async function gateComputerAction(opts: GateOptions): Promise<AnthropicToolResultBlock> {
  const cls = classifyAction(opts.block);
  const aprp = buildAprpFromAction({
    agentId: opts.agentId,
    block: opts.block,
    ...(opts.taskId !== undefined ? { taskId: opts.taskId } : {}),
    ...(opts.riskClassifier !== undefined ? { riskClassifier: opts.riskClassifier } : {}),
  });

  let receipt: PolicyReceipt;
  try {
    const submitOpts =
      opts.idempotencyKey !== undefined
        ? { idempotencyKey: opts.idempotencyKey(aprp) }
        : {};
    const r = await opts.sbo3l.submit(aprp, submitOpts);
    if (r.decision !== "allow") {
      return {
        type: "tool_result",
        tool_use_id: opts.block.id,
        is_error: true,
        content: JSON.stringify({
          error: r.decision === "deny" ? "policy.deny" : "policy.requires_human",
          decision: r.decision,
          deny_code: r.deny_code,
          matched_rule_id: r.matched_rule_id,
          audit_event_id: r.audit_event_id,
          action_class: cls,
        }),
      };
    }
    receipt = r.receipt;
  } catch (e) {
    if (e instanceof SBO3LError) {
      return {
        type: "tool_result",
        tool_use_id: opts.block.id,
        is_error: true,
        content: JSON.stringify({
          error: "transport.failed",
          detail: e.message,
          action_class: cls,
        }),
      };
    }
    return {
      type: "tool_result",
      tool_use_id: opts.block.id,
      is_error: true,
      content: JSON.stringify({
        error: "transport.unknown",
        detail: e instanceof Error ? e.message : String(e),
        action_class: cls,
      }),
    };
  }

  // SBO3L allowed — run the caller's executor.
  try {
    const out = await opts.executor(opts.block);
    return {
      type: "tool_result",
      tool_use_id: opts.block.id,
      content: JSON.stringify({
        ok: true,
        output: out,
        audit_event_id: receipt.audit_event_id,
        action_class: cls,
      }),
    };
  } catch (e) {
    return {
      type: "tool_result",
      tool_use_id: opts.block.id,
      is_error: true,
      content: JSON.stringify({
        error: "executor.failed",
        detail: e instanceof Error ? e.message : String(e),
        audit_event_id: receipt.audit_event_id,
        action_class: cls,
      }),
    };
  }
}

export { SBO3LError };
