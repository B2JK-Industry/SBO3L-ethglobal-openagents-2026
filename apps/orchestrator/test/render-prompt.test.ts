import { describe, expect, it } from "vitest";

import { inferPhase, renderAgentPrompt } from "../src/render-prompt.js";

describe("inferPhase", () => {
  it.each([
    ["F-1", 1],
    ["F-13", 1],
    ["T-2-1", 1],
    ["T-3-4", 2],
    ["T-4-2", 2],
    ["T-5-5", 2],
    ["CTI-3-2", 2],
    ["T-1-3", 2],
    ["T-1-7", 3],
    ["T-6-1", 3],
    ["T-7-1", 3],
    ["T-8-3", 3],
    ["CTI-4-5", 3],
  ] as const)("%s → phase %i", (id, phase) => {
    expect(inferPhase(id)).toBe(phase);
  });

  it("defaults unknown prefixes to phase 2", () => {
    expect(inferPhase("X-99")).toBe(2);
  });
});

describe("renderAgentPrompt", () => {
  it("renders a prompt with all required sections", () => {
    const out = renderAgentPrompt({
      slot: "Dev 1",
      branchSlug: "dev1",
      ticketIdentifier: "F-2",
      ticketTitle: "Persistent budget store",
      phase: 1,
    });

    expect(out).toContain("You are Dev 1.");
    expect(out).toContain("Your assigned ticket: F-2 (Persistent budget store)");
    expect(out).toContain("docs/win-backlog/05-phase-1.md");
    expect(out).toContain("Branch: agent/dev1/F-2");
    expect(out).toContain("Begin reading the backlog now.");
    expect(out).toContain("docs/win-backlog/03-agents.md");
  });

  it("uses Phase 2 backlog file for Phase 2 tickets", () => {
    const out = renderAgentPrompt({
      slot: "Dev 4",
      branchSlug: "dev4",
      ticketIdentifier: "T-3-1",
      ticketTitle: "Durin issuance flow",
      phase: 2,
    });
    expect(out).toContain("docs/win-backlog/06-phase-2.md");
  });

  it("uses Phase 3 backlog file for Phase 3 tickets", () => {
    const out = renderAgentPrompt({
      slot: "Dev 4",
      branchSlug: "dev4",
      ticketIdentifier: "T-6-1",
      ticketTitle: "0G Storage capsule",
      phase: 3,
    });
    expect(out).toContain("docs/win-backlog/07-phase-3.md");
  });

  it("matches the prompt-template surface (key constraint lines)", () => {
    const out = renderAgentPrompt({
      slot: "QA + Release",
      branchSlug: "qa",
      ticketIdentifier: "F-3",
      ticketTitle: "Idempotency atomicity (state machine)",
      phase: 1,
    });

    // Constraints section, verbatim from 09-prompt-template.md.
    expect(out).toContain("- One ticket = one PR");
    expect(out).toContain("- PR title = ticket title verbatim");
    expect(out).toContain("- Daniel + Heidi approve before merge");
    expect(out).toContain("- Wait for unmet dependencies; do not attempt workarounds");
  });
});
