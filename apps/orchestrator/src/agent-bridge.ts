import type { Slot, SlotConfig } from "./types.js";

/**
 * Transport interface for delivering a rendered prompt to an agent runtime.
 *
 * The MVP implementation posts to a Discord webhook for the slot's prompt
 * channel. Future transports (tmux SSH paste, Claude Code API injection,
 * SQS, etc.) implement the same shape.
 */
export interface AgentTransport {
  send(slot: Slot, prompt: string, config: SlotConfig): Promise<void>;
}

/** Posts the prompt to Discord as a fenced code block in the slot's channel. */
export class DiscordWebhookTransport implements AgentTransport {
  constructor(private readonly fetchImpl: typeof fetch = fetch) {}

  async send(slot: Slot, prompt: string, config: SlotConfig): Promise<void> {
    const body = formatDiscordPayload(prompt);
    const res = await this.fetchImpl(config.discordWebhookUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      const detail = await safeReadBody(res);
      throw new Error(
        `Discord webhook delivery failed for ${slot}: ${res.status} ${res.statusText} — ${detail}`,
      );
    }
  }
}

/** No-op transport used when ORCHESTRATOR_DRY_RUN=1. Captures sends in memory. */
export class DryRunTransport implements AgentTransport {
  public readonly sent: Array<{ slot: Slot; prompt: string }> = [];

  send(slot: Slot, prompt: string, _config: SlotConfig): Promise<void> {
    this.sent.push({ slot, prompt });
    return Promise.resolve();
  }
}

const DISCORD_MAX_CONTENT = 2000;

/**
 * Discord caps webhook content at 2000 chars. Long prompts are split into
 * an intro line and a code-fenced body; if still too long, we send sequential
 * messages so the agent reads them in order.
 */
export function formatDiscordPayload(prompt: string): {
  username: string;
  content: string;
} {
  const username = "SBO3L Orchestrator";
  const fenced = "```\n" + prompt + "\n```";
  if (fenced.length <= DISCORD_MAX_CONTENT) {
    return { username, content: fenced };
  }
  // Truncate with marker; full prompt is also in orchestrator logs for audit.
  const trimmed = prompt.slice(0, DISCORD_MAX_CONTENT - 64);
  return {
    username,
    content:
      "```\n" + trimmed + "\n```\n_(prompt truncated — see orchestrator logs)_",
  };
}

async function safeReadBody(res: Response): Promise<string> {
  try {
    return (await res.text()).slice(0, 256);
  } catch {
    return "<no body>";
  }
}

/** Posts a one-line status update (queue empty, error, etc.) to coordination. */
export async function postCoordinationStatus(
  message: string,
  url: string | undefined,
  fetchImpl: typeof fetch = fetch,
): Promise<void> {
  if (!url) return;
  await fetchImpl(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username: "SBO3L Orchestrator", content: message }),
  });
}
