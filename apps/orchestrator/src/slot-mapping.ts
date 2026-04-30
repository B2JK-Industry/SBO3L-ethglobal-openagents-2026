import type { Slot, SlotConfig } from "./types.js";

/** All slot identifiers, in canonical order. */
export const SLOTS: readonly Slot[] = [
  "Dev 1",
  "Dev 2",
  "Dev 3",
  "Dev 4",
  "QA + Release",
];

const SLOT_BRANCH_SLUGS: Record<Slot, string> = {
  "Dev 1": "dev1",
  "Dev 2": "dev2",
  "Dev 3": "dev3",
  "Dev 4": "dev4",
  "QA + Release": "qa",
};

/**
 * Type-guard: is `name` a recognised slot? Names come from Linear's
 * `assignee.name` field, which Daniel sets to "Dev 1" etc. when assigning.
 */
export function isSlot(name: string | undefined | null): name is Slot {
  if (!name) return false;
  return (SLOTS as readonly string[]).includes(name);
}

/** Maps `"Dev 1"` → `"dev1"` for branch names like `agent/dev1/F-1`. */
export function slotBranchSlug(slot: Slot): string {
  return SLOT_BRANCH_SLUGS[slot];
}

/** Slot → env var prefix; `Dev 1` → `DEV1`, `QA + Release` → `QA`. */
function slotEnvKey(slot: Slot): string {
  return slot === "QA + Release" ? "QA" : slot.replace(/\s+/g, "").toUpperCase();
}

/**
 * Resolves the per-slot Discord webhook URL from env. Throws if the env var
 * is missing — orchestrator is useless without a delivery channel for the
 * slot whose ticket just merged.
 */
export function loadSlotConfig(slot: Slot, env: NodeJS.ProcessEnv): SlotConfig {
  const key = `DISCORD_WEBHOOK_${slotEnvKey(slot)}_URL`;
  const url = env[key];
  if (!url) {
    throw new Error(
      `Missing env var ${key} (required to dispatch prompt to ${slot}).`,
    );
  }
  return { discordWebhookUrl: url, branchSlug: slotBranchSlug(slot) };
}
