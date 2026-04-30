import { describe, expect, it } from "vitest";

import {
  DiscordWebhookTransport,
  formatDiscordPayload,
} from "../src/agent-bridge.js";

describe("formatDiscordPayload", () => {
  it("wraps short prompts in a code fence", () => {
    const payload = formatDiscordPayload("hello world");
    expect(payload.username).toBe("SBO3L Orchestrator");
    expect(payload.content.startsWith("```\n")).toBe(true);
    expect(payload.content.endsWith("\n```")).toBe(true);
    expect(payload.content).toContain("hello world");
  });

  it("truncates prompts over the 2000 char Discord limit", () => {
    const big = "x".repeat(3000);
    const payload = formatDiscordPayload(big);
    expect(payload.content.length).toBeLessThanOrEqual(2000);
    expect(payload.content).toContain("(prompt truncated");
  });
});

describe("DiscordWebhookTransport", () => {
  it("POSTs the payload to the slot's webhook URL", async () => {
    const calls: Array<{ url: string; init: RequestInit | undefined }> = [];
    const fakeFetch = (async (url: string | URL, init?: RequestInit) => {
      calls.push({ url: String(url), init });
      return new Response(null, { status: 204 });
    }) as unknown as typeof fetch;

    const transport = new DiscordWebhookTransport(fakeFetch);
    await transport.send("Dev 1", "the-prompt", {
      discordWebhookUrl: "https://discord.example/dev1",
      branchSlug: "dev1",
    });

    expect(calls).toHaveLength(1);
    expect(calls[0]?.url).toBe("https://discord.example/dev1");
    expect(calls[0]?.init?.method).toBe("POST");
    const body = String(calls[0]?.init?.body ?? "");
    expect(body).toContain("the-prompt");
  });

  it("throws on non-2xx responses", async () => {
    const fakeFetch = (async () =>
      new Response("rate limited", { status: 429 })) as unknown as typeof fetch;

    const transport = new DiscordWebhookTransport(fakeFetch);
    await expect(
      transport.send("Dev 1", "p", {
        discordWebhookUrl: "https://discord.example/dev1",
        branchSlug: "dev1",
      }),
    ).rejects.toThrow(/429/);
  });
});
