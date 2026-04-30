import type { RenderPromptInput } from "./types.js";

const PHASE_FILE: Record<1 | 2 | 3, string> = {
  1: "docs/win-backlog/05-phase-1.md",
  2: "docs/win-backlog/06-phase-2.md",
  3: "docs/win-backlog/07-phase-3.md",
};

/**
 * Maps a ticket identifier prefix (e.g. "F-1", "T-3-4", "CTI-4-2") to its
 * backlog phase. Default is phase 2 for ambiguous T-1-* mid-range; Daniel
 * can override by including phase metadata in the Linear issue if needed.
 *
 * Mapping basis: docs/win-backlog/{05,06,07}-phase-N.md ticket indexes.
 */
export function inferPhase(identifier: string): 1 | 2 | 3 {
  const id = identifier.toUpperCase();

  if (id.startsWith("F-")) return 1;
  if (id.startsWith("T-2-")) return 1;

  if (id.startsWith("T-3-")) return 2;
  if (id.startsWith("T-4-")) return 2;
  if (id.startsWith("T-5-")) return 2;
  if (id.startsWith("CTI-3-")) return 2;

  if (id.startsWith("T-6-")) return 3;
  if (id.startsWith("T-7-")) return 3;
  if (id.startsWith("T-8-")) return 3;
  if (id.startsWith("CTI-4-")) return 3;

  if (id.startsWith("T-1-")) {
    const trailing = Number.parseInt(id.slice("T-1-".length), 10);
    return Number.isFinite(trailing) && trailing >= 7 ? 3 : 2;
  }

  return 2;
}

/**
 * Renders the Universal prompt from docs/win-backlog/09-prompt-template.md
 * with the slot, ticket, and branch fields filled in. The shape (sections,
 * order, headings) mirrors the template literally so agents pattern-match
 * the prompt the way Daniel hand-writes them.
 */
export function renderAgentPrompt(input: RenderPromptInput): string {
  const phaseFile = PHASE_FILE[input.phase];
  return [
    `You are ${input.slot}.`,
    ``,
    `Your full operating profile is at docs/win-backlog/03-agents.md (find your section by name).`,
    ``,
    `Read the win backlog folder before doing anything else, in this order:`,
    `  1. docs/win-backlog/00-readme.md       — mission + how-to-use`,
    `  2. docs/win-backlog/01-identity.md     — locked product identity`,
    `  3. docs/win-backlog/02-standards.md    — dev + QA + PR + testing standards`,
    `  4. docs/win-backlog/03-agents.md       — your operating profile (your section)`,
    `  5. docs/win-backlog/04-orchestration.md — branch strategy + dependencies + daily rhythm`,
    ``,
    `Your assigned ticket: ${input.ticketIdentifier} (${input.ticketTitle})`,
    `Ticket location: ${phaseFile} (search for ${input.ticketIdentifier})`,
    ``,
    `Constraints:`,
    `- Branch: agent/${input.branchSlug}/${input.ticketIdentifier}`,
    `- One ticket = one PR`,
    `- PR title = ticket title verbatim`,
    `- Wait for unmet dependencies; do not attempt workarounds`,
    `- Submit PR when all acceptance criteria met`,
    `- Daniel + Heidi approve before merge`,
    ``,
    `If blocked, post in #sbo3l-blockers (or coordination channel). Do not proceed.`,
    `If clarification needed, post in #sbo3l-coordination, tag @daniel.`,
    ``,
    `Begin reading the backlog now.`,
  ].join("\n");
}
