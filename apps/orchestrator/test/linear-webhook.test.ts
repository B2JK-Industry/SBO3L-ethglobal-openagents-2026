import { beforeEach, describe, expect, it } from "vitest";

import { DryRunTransport } from "../src/agent-bridge.js";
import type { LinearClient } from "../src/linear-client.js";
import { handleLinearWebhook } from "../src/linear-webhook.js";
import type { LinearIssue, LinearWebhookEvent } from "../src/types.js";

class FakeLinearClient implements LinearClient {
  public next: LinearIssue | null = null;
  public markedInProgress: string[] = [];
  public nextCalls: string[] = [];

  nextTicketForSlot(slot: string): Promise<LinearIssue | null> {
    this.nextCalls.push(slot);
    return Promise.resolve(this.next);
  }

  markInProgress(issueId: string): Promise<void> {
    this.markedInProgress.push(issueId);
    return Promise.resolve();
  }
}

function completedIssueEvent(overrides: {
  identifier: string;
  assignee?: string;
}): LinearWebhookEvent {
  return {
    action: "update",
    type: "Issue",
    data: {
      id: `id-${overrides.identifier}`,
      identifier: overrides.identifier,
      title: "completed ticket",
      priority: 2,
      state: { id: "state-done", name: "Done", type: "completed" },
      assignee:
        overrides.assignee !== undefined
          ? { id: "u-1", name: overrides.assignee }
          : undefined,
    },
  };
}

const ENV: NodeJS.ProcessEnv = {
  DISCORD_WEBHOOK_DEV1_URL: "https://discord.example/dev1",
  DISCORD_WEBHOOK_DEV2_URL: "https://discord.example/dev2",
  DISCORD_WEBHOOK_DEV3_URL: "https://discord.example/dev3",
  DISCORD_WEBHOOK_DEV4_URL: "https://discord.example/dev4",
  DISCORD_WEBHOOK_QA_URL: "https://discord.example/qa",
  DISCORD_WEBHOOK_COORDINATION_URL: "https://discord.example/coord",
};

describe("handleLinearWebhook", () => {
  let linear: FakeLinearClient;
  let transport: DryRunTransport;

  beforeEach(() => {
    linear = new FakeLinearClient();
    transport = new DryRunTransport();
  });

  it("ignores non-Issue events", async () => {
    const out = await handleLinearWebhook(
      {
        action: "create",
        type: "Comment",
        data: completedIssueEvent({ identifier: "F-1" }).data,
      },
      { linear, transport, env: ENV },
    );
    expect(out.kind).toBe("ignored");
    expect(transport.sent).toHaveLength(0);
    expect(linear.markedInProgress).toHaveLength(0);
  });

  it("ignores non-update actions", async () => {
    const ev = completedIssueEvent({ identifier: "F-1", assignee: "Dev 1" });
    ev.action = "create";
    const out = await handleLinearWebhook(ev, { linear, transport, env: ENV });
    expect(out.kind).toBe("ignored");
  });

  it("ignores tickets that did not transition to completed", async () => {
    const ev = completedIssueEvent({ identifier: "F-1", assignee: "Dev 1" });
    ev.data.state = { id: "x", name: "In Progress", type: "started" };
    const out = await handleLinearWebhook(ev, { linear, transport, env: ENV });
    expect(out.kind).toBe("ignored");
  });

  it("ignores completed tickets without a recognised slot assignee", async () => {
    const ev = completedIssueEvent({ identifier: "F-1", assignee: "Daniel" });
    const out = await handleLinearWebhook(ev, { linear, transport, env: ENV });
    expect(out.kind).toBe("ignored");
    expect(linear.nextCalls).toHaveLength(0);
  });

  it("posts queue-empty status when no next ticket is available", async () => {
    const ev = completedIssueEvent({ identifier: "F-1", assignee: "Dev 1" });
    linear.next = null;

    const fetched: Array<{ url: string; body: string }> = [];
    const fakeFetch = (async (url: string | URL, init?: RequestInit) => {
      fetched.push({
        url: String(url),
        body: typeof init?.body === "string" ? init.body : "",
      });
      return new Response("ok", { status: 200 });
    }) as unknown as typeof fetch;

    const out = await handleLinearWebhook(ev, {
      linear,
      transport,
      env: ENV,
      fetchImpl: fakeFetch,
    });

    expect(out).toEqual({ kind: "queue_empty", slot: "Dev 1" });
    expect(transport.sent).toHaveLength(0);
    expect(linear.markedInProgress).toHaveLength(0);
    expect(fetched).toHaveLength(1);
    expect(fetched[0]?.url).toBe(ENV["DISCORD_WEBHOOK_COORDINATION_URL"]);
    expect(fetched[0]?.body).toContain("Dev 1 queue empty");
  });

  it("dispatches the next ticket and marks it in progress", async () => {
    const ev = completedIssueEvent({ identifier: "F-1", assignee: "Dev 1" });
    linear.next = {
      id: "linear-uuid-F2",
      identifier: "F-2",
      title: "Persistent budget store",
      priority: 2,
      state: { id: "s-todo", name: "Todo", type: "unstarted" },
      assignee: { id: "u-1", name: "Dev 1" },
    };

    const out = await handleLinearWebhook(ev, { linear, transport, env: ENV });

    expect(out).toEqual({
      kind: "dispatched",
      slot: "Dev 1",
      nextTicket: "F-2",
    });
    expect(transport.sent).toHaveLength(1);
    expect(transport.sent[0]?.slot).toBe("Dev 1");
    expect(transport.sent[0]?.prompt).toContain(
      "Your assigned ticket: F-2 (Persistent budget store)",
    );
    expect(transport.sent[0]?.prompt).toContain("Branch: agent/dev1/F-2");
    expect(linear.markedInProgress).toEqual(["linear-uuid-F2"]);
  });

  it("handles QA + Release slot dispatch", async () => {
    const ev = completedIssueEvent({
      identifier: "X-Y",
      assignee: "QA + Release",
    });
    linear.next = {
      id: "qa-uuid",
      identifier: "F-1",
      title: "Real auth middleware (bearer + JWT)",
      priority: 1,
      state: { id: "s-todo", name: "Todo", type: "unstarted" },
      assignee: { id: "u-qa", name: "QA + Release" },
    };

    const out = await handleLinearWebhook(ev, { linear, transport, env: ENV });
    expect(out.kind).toBe("dispatched");
    expect(transport.sent[0]?.prompt).toContain("Branch: agent/qa/F-1");
  });
});
