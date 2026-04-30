import { describe, expect, it } from "vitest";

import {
  SLOTS,
  isSlot,
  loadSlotConfig,
  slotBranchSlug,
} from "../src/slot-mapping.js";

describe("isSlot", () => {
  it("recognises all canonical slot names", () => {
    for (const s of SLOTS) {
      expect(isSlot(s)).toBe(true);
    }
  });

  it("rejects unknown / casing-shifted strings", () => {
    expect(isSlot("dev 1")).toBe(false);
    expect(isSlot("Dev1")).toBe(false);
    expect(isSlot("Daniel")).toBe(false);
    expect(isSlot(undefined)).toBe(false);
    expect(isSlot(null)).toBe(false);
    expect(isSlot("")).toBe(false);
  });
});

describe("slotBranchSlug", () => {
  it("maps slots to branch slugs", () => {
    expect(slotBranchSlug("Dev 1")).toBe("dev1");
    expect(slotBranchSlug("Dev 4")).toBe("dev4");
    expect(slotBranchSlug("QA + Release")).toBe("qa");
  });
});

describe("loadSlotConfig", () => {
  it("loads webhook URL from per-slot env var", () => {
    const env = {
      DISCORD_WEBHOOK_DEV1_URL: "https://discord.example/dev1",
    } as NodeJS.ProcessEnv;
    const config = loadSlotConfig("Dev 1", env);
    expect(config.discordWebhookUrl).toBe("https://discord.example/dev1");
    expect(config.branchSlug).toBe("dev1");
  });

  it("uses QA prefix for the QA + Release slot", () => {
    const env = {
      DISCORD_WEBHOOK_QA_URL: "https://discord.example/qa",
    } as NodeJS.ProcessEnv;
    const config = loadSlotConfig("QA + Release", env);
    expect(config.discordWebhookUrl).toBe("https://discord.example/qa");
    expect(config.branchSlug).toBe("qa");
  });

  it("throws when the slot's env var is missing", () => {
    expect(() => loadSlotConfig("Dev 2", {} as NodeJS.ProcessEnv)).toThrow(
      /DISCORD_WEBHOOK_DEV2_URL/,
    );
  });
});
