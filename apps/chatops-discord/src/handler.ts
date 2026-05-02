/**
 * Discord ChatOps handler — same dispatch surface as the Slack bot,
 * but the response envelope is shaped for Discord's interaction API.
 *
 * Pure functions, fully unit-testable. The HTTP entry point in
 * server.ts maps Discord interaction POSTs (after Ed25519 verification)
 * to these.
 */

export type DiscordResponseType =
  | 1 // PONG (interaction PING ack)
  | 4 // CHANNEL_MESSAGE_WITH_SOURCE
  | 5; // DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE

export interface DiscordResponse {
  type: DiscordResponseType;
  data?: {
    /** Plain text body. Markdown supported. */
    content: string;
    /** Bit flag 64 = ephemeral (only the invoking user sees it). */
    flags?: number;
  };
}

const FLAG_EPHEMERAL = 64;

/** Inline 6-check verifier (mirrors Slack handler + ci-plugins/_shared/verifier.mjs). */
function verifyCapsule(c: unknown): {
  decision: string;
  audit_event_id: string | null;
  checks: Array<{ name: string; ok: boolean; detail?: string }>;
} {
  const checks: Array<{ name: string; ok: boolean; detail?: string }> = [];
  const isObj = (v: unknown): v is Record<string, unknown> =>
    v !== null && typeof v === "object" && !Array.isArray(v);

  checks.push({ name: "capsule.is_object", ok: isObj(c) });
  if (!isObj(c)) return { decision: "deny", audit_event_id: null, checks };

  const ctype = (c["capsule_type"] as string | undefined) ?? (c["receipt_type"] as string | undefined) ?? "";
  checks.push({
    name: "capsule.type_recognised",
    ok: typeof ctype === "string" && ctype.startsWith("sbo3l."),
    detail: ctype,
  });

  const receipt = c["receipt"] as Record<string, unknown> | undefined;
  const decision = (c["decision"] as string | undefined) ?? (receipt?.["decision"] as string | undefined) ?? "unknown";
  checks.push({
    name: "capsule.decision_set",
    ok: ["allow", "deny", "requires_human"].includes(decision),
    detail: decision,
  });

  const auditId =
    (c["audit_event_id"] as string | undefined) ?? (receipt?.["audit_event_id"] as string | undefined) ?? null;
  checks.push({
    name: "capsule.audit_event_id_present",
    ok: typeof auditId === "string" && /^evt-/.test(auditId),
    detail: auditId ?? "(missing)",
  });

  const requestHash = (c["request_hash"] as string | undefined) ?? (receipt?.["request_hash"] as string | undefined) ?? null;
  checks.push({
    name: "capsule.request_hash_present",
    ok: typeof requestHash === "string" && requestHash.length === 64,
  });

  const policyHash = (c["policy_hash"] as string | undefined) ?? (receipt?.["policy_hash"] as string | undefined) ?? null;
  checks.push({
    name: "capsule.policy_hash_present",
    ok: typeof policyHash === "string" && policyHash.length === 64,
  });

  return { decision, audit_event_id: auditId, checks };
}

function ephemeralReply(content: string): DiscordResponse {
  return { type: 4, data: { content, flags: FLAG_EPHEMERAL } };
}

export interface VerifyConfig {
  capsuleText: string;
}

export function handleVerify(config: VerifyConfig): DiscordResponse {
  const text = config.capsuleText.trim();
  if (text.length === 0) {
    return ephemeralReply("Usage: `/sbo3l verify <capsule JSON>` — paste the capsule body inline.");
  }

  let capsule: unknown;
  try {
    capsule = JSON.parse(text);
  } catch (e) {
    return ephemeralReply(`❌ capsule is not valid JSON: ${e instanceof Error ? e.message : String(e)}`);
  }

  const result = verifyCapsule(capsule);
  const passed = result.checks.filter((c) => c.ok).length;
  const total = result.checks.length;
  const allOk = passed === total;
  const headerGlyph = allOk ? "✅" : "❌";

  const lines: string[] = [];
  lines.push(`**SBO3L verify**`);
  lines.push(`Decision: \`${result.decision}\``);
  if (result.audit_event_id !== null) lines.push(`Audit event id: \`${result.audit_event_id}\``);
  lines.push(`Checks: **${passed} / ${total}** ${headerGlyph}`);
  for (const c of result.checks) {
    const g = c.ok ? "✅" : "❌";
    const d = c.detail !== undefined ? ` — \`${c.detail}\`` : "";
    lines.push(`  ${g} ${c.name}${d}`);
  }

  return ephemeralReply(lines.join("\n"));
}

export interface AuditConfig {
  agentId: string;
  fetchAuditPrefix: (agentId: string) => Promise<{
    chain_length: number;
    head_event_id: string | null;
    recent: Array<{ event_id: string; type: string; ts: string }>;
  }>;
}

export async function handleAudit(config: AuditConfig): Promise<DiscordResponse> {
  const id = config.agentId.trim();
  if (id.length === 0) {
    return ephemeralReply("Usage: `/sbo3l audit <agent_id>` — e.g. `/sbo3l audit research-agent-01`");
  }

  let prefix: Awaited<ReturnType<typeof config.fetchAuditPrefix>>;
  try {
    prefix = await config.fetchAuditPrefix(id);
  } catch (e) {
    return ephemeralReply(`❌ daemon error: ${e instanceof Error ? e.message : String(e)}`);
  }

  const lines: string[] = [];
  lines.push(`**Audit chain** — \`${id}\``);
  lines.push(`Chain length: **${prefix.chain_length}**`);
  if (prefix.head_event_id !== null) lines.push(`Head: \`${prefix.head_event_id}\``);
  if (prefix.recent.length > 0) {
    lines.push(`Recent events:`);
    for (const e of prefix.recent) {
      lines.push(`  • \`${e.event_id}\` **${e.type}** — ${e.ts}`);
    }
  } else {
    lines.push(`(no events yet)`);
  }

  return ephemeralReply(lines.join("\n"));
}

export interface DecideConfig {
  aprpText: string;
  submit: (aprp: unknown) => Promise<{
    decision: string;
    deny_code: string | null;
    matched_rule_id: string | null;
    audit_event_id: string;
    receipt: { execution_ref: string | null };
  }>;
}

export async function handleDecide(config: DecideConfig): Promise<DiscordResponse> {
  const text = config.aprpText.trim();
  if (text.length === 0) {
    return ephemeralReply("Usage: `/sbo3l decide <APRP JSON>` — paste an APRP v1 body inline.");
  }

  let aprp: unknown;
  try {
    aprp = JSON.parse(text);
  } catch (e) {
    return ephemeralReply(`❌ APRP is not valid JSON: ${e instanceof Error ? e.message : String(e)}`);
  }

  let result;
  try {
    result = await config.submit(aprp);
  } catch (e) {
    return ephemeralReply(`❌ daemon error: ${e instanceof Error ? e.message : String(e)}`);
  }

  const glyph = result.decision === "allow" ? "✅" : result.decision === "requires_human" ? "⚠️" : "⊗";
  const lines: string[] = [];
  lines.push(`${glyph} **SBO3L decide**`);
  lines.push(`Decision: \`${result.decision}\``);
  lines.push(`Audit event id: \`${result.audit_event_id}\``);
  if (result.matched_rule_id !== null) lines.push(`Matched rule: \`${result.matched_rule_id}\``);
  if (result.deny_code !== null) lines.push(`Deny code: \`${result.deny_code}\``);
  if (result.receipt.execution_ref !== null) lines.push(`Execution ref: \`${result.receipt.execution_ref}\``);

  return ephemeralReply(lines.join("\n"));
}

export interface SlashCommandConfig {
  /**
   * The Discord interaction's command name and the chosen subcommand.
   * Discord splits subcommands as a separate option — we accept either
   * "verify capsule_text" via `text` (Slack-compat shape) OR an explicit
   * `subcommand`/`option` pair.
   */
  text?: string;
  subcommand?: string;
  option?: string;
  fetchAuditPrefix: AuditConfig["fetchAuditPrefix"];
  submit: DecideConfig["submit"];
}

export async function dispatchSlashCommand(config: SlashCommandConfig): Promise<DiscordResponse> {
  // Resolve subcommand + payload from either input shape.
  let subcommand: string;
  let rest: string;
  if (config.subcommand !== undefined) {
    subcommand = config.subcommand;
    rest = config.option ?? "";
  } else {
    const text = (config.text ?? "").trim();
    const firstSpace = text.indexOf(" ");
    subcommand = firstSpace === -1 ? text : text.slice(0, firstSpace);
    rest = firstSpace === -1 ? "" : text.slice(firstSpace + 1);
  }

  switch (subcommand) {
    case "verify":
      return handleVerify({ capsuleText: rest });
    case "audit":
      return handleAudit({ agentId: rest, fetchAuditPrefix: config.fetchAuditPrefix });
    case "decide":
      return handleDecide({ aprpText: rest, submit: config.submit });
    case "":
    case "help":
      return ephemeralReply(
        [
          "**SBO3L slash commands**",
          "• `/sbo3l verify <capsule JSON>` — 6-check inline verification",
          "• `/sbo3l audit <agent_id>` — recent audit chain prefix",
          "• `/sbo3l decide <APRP JSON>` — submit through SBO3L daemon",
        ].join("\n"),
      );
    default:
      return ephemeralReply(`Unknown subcommand \`${subcommand}\`. Try \`/sbo3l help\`.`);
  }
}
