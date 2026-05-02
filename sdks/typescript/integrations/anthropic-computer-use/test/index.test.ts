import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SBO3LClient } from "@sbo3l/sdk";

import {
  buildAprpFromAction,
  classifyAction,
  gateComputerAction,
  type AnthropicToolUseBlock,
} from "../src/index.js";

const ENDPOINT = "http://sbo3l-test.local:8730";

function fetchOnce(response: object, status = 200): void {
  globalThis.fetch = vi.fn().mockResolvedValue({
    ok: status >= 200 && status < 300,
    status,
    headers: new Headers({ "content-type": "application/json" }),
    json: async () => response,
    text: async () => JSON.stringify(response),
  });
}

const allowResponse = (auditId = "evt-1") => ({
  decision: "allow",
  deny_code: null,
  matched_rule_id: "allow-compute-job",
  request_hash: "00".repeat(32),
  policy_hash: "00".repeat(32),
  audit_event_id: auditId,
  receipt: {
    execution_ref: "kh-1",
    agent_id: "research-agent-01",
    task_id: "cu-tu-001",
    decision: "allow",
    audit_event_id: auditId,
    request_hash: "00".repeat(32),
    policy_hash: "00".repeat(32),
    signature: { alg: "ed25519", key_id: "k1", value: "00".repeat(32) },
    signed_at: "2099-01-01T00:00:00Z",
  },
});

const denyResponse = (denyCode = "policy.deny_dangerous_bash", auditId = "evt-deny-1") => ({
  decision: "deny",
  deny_code: denyCode,
  matched_rule_id: null,
  request_hash: "00".repeat(32),
  policy_hash: "00".repeat(32),
  audit_event_id: auditId,
  receipt: {
    execution_ref: null,
    agent_id: "research-agent-01",
    task_id: "cu-tu-001",
    decision: "deny",
    audit_event_id: auditId,
    request_hash: "00".repeat(32),
    policy_hash: "00".repeat(32),
    signature: { alg: "ed25519", key_id: "k1", value: "00".repeat(32) },
    signed_at: "2099-01-01T00:00:00Z",
  },
});

beforeEach(() => {
  vi.restoreAllMocks();
});

afterEach(() => {
  vi.restoreAllMocks();
});

const mouseClick = (id = "tu-001"): AnthropicToolUseBlock => ({
  type: "tool_use",
  id,
  name: "computer",
  input: { action: "left_click", coordinate: [100, 200] },
});

const screenshot = (id = "tu-002"): AnthropicToolUseBlock => ({
  type: "tool_use",
  id,
  name: "computer_20241022",
  input: { action: "screenshot" },
});

const bash = (id = "tu-003"): AnthropicToolUseBlock => ({
  type: "tool_use",
  id,
  name: "bash_20250124",
  input: { command: "ls -la /tmp" },
});

const editorWrite = (id = "tu-004"): AnthropicToolUseBlock => ({
  type: "tool_use",
  id,
  name: "str_replace_editor",
  input: { command: "create", path: "/tmp/x.txt", file_text: "hi" },
});

const editorRead = (id = "tu-005"): AnthropicToolUseBlock => ({
  type: "tool_use",
  id,
  name: "text_editor_20250124",
  input: { command: "view", path: "/tmp/x.txt" },
});

describe("classifyAction", () => {
  it("computer + left_click = computer.mouse", () => {
    expect(classifyAction(mouseClick())).toBe("computer.mouse");
  });

  it("computer_20241022 + screenshot = computer.screenshot", () => {
    expect(classifyAction(screenshot())).toBe("computer.screenshot");
  });

  it("computer + type = computer.keyboard", () => {
    const block: AnthropicToolUseBlock = {
      type: "tool_use",
      id: "x",
      name: "computer",
      input: { action: "type", text: "hello" },
    };
    expect(classifyAction(block)).toBe("computer.keyboard");
  });

  it("bash_* = bash.exec", () => {
    expect(classifyAction(bash())).toBe("bash.exec");
  });

  it("str_replace_editor + create = text_editor.write", () => {
    expect(classifyAction(editorWrite())).toBe("text_editor.write");
  });

  it("text_editor + view = text_editor.read", () => {
    expect(classifyAction(editorRead())).toBe("text_editor.read");
  });

  it("unknown tool name → unknown", () => {
    const block: AnthropicToolUseBlock = {
      type: "tool_use",
      id: "x",
      name: "exotic_tool",
      input: {},
    };
    expect(classifyAction(block)).toBe("unknown");
  });
});

describe("buildAprpFromAction", () => {
  it("derives provider_url from action class", () => {
    const aprp = buildAprpFromAction({ agentId: "a", block: bash() });
    expect(aprp.provider_url).toBe("urn:anthropic-computer-use:bash.exec");
  });

  it("bash defaults to high risk", () => {
    expect(buildAprpFromAction({ agentId: "a", block: bash() }).risk_class).toBe("high");
  });

  it("screenshot defaults to low risk", () => {
    expect(
      buildAprpFromAction({ agentId: "a", block: screenshot() }).risk_class,
    ).toBe("low");
  });

  it("text_editor.write defaults to medium", () => {
    expect(
      buildAprpFromAction({ agentId: "a", block: editorWrite() }).risk_class,
    ).toBe("medium");
  });

  it("unknown defaults to critical (fail-closed)", () => {
    const block: AnthropicToolUseBlock = {
      type: "tool_use",
      id: "x",
      name: "weird",
      input: {},
    };
    expect(
      buildAprpFromAction({ agentId: "a", block }).risk_class,
    ).toBe("critical");
  });

  it("riskClassifier override is respected", () => {
    const aprp = buildAprpFromAction({
      agentId: "a",
      block: screenshot(),
      riskClassifier: () => "critical",
    });
    expect(aprp.risk_class).toBe("critical");
  });

  it("intent is pay_compute_job, amount 0 USD, smart_account_session", () => {
    const aprp = buildAprpFromAction({ agentId: "a", block: mouseClick() });
    expect(aprp.intent).toBe("pay_compute_job");
    expect(aprp.amount).toEqual({ value: "0", currency: "USD" });
    expect(aprp.payment_protocol).toBe("smart_account_session");
  });

  it("nonce is fresh per call", () => {
    const a = buildAprpFromAction({ agentId: "a", block: bash() });
    const b = buildAprpFromAction({ agentId: "a", block: bash() });
    expect(a.nonce).not.toBe(b.nonce);
  });

  it("expiry is 5 minutes ahead by default", () => {
    const before = Date.now();
    const aprp = buildAprpFromAction({ agentId: "a", block: bash() });
    const expiry = Date.parse(aprp.expiry);
    expect(expiry - before).toBeGreaterThanOrEqual(4 * 60 * 1000);
    expect(expiry - before).toBeLessThanOrEqual(6 * 60 * 1000);
  });
});

describe("gateComputerAction", () => {
  it("on allow: invokes executor + returns ok envelope", async () => {
    fetchOnce(allowResponse());
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    const executor = vi.fn().mockResolvedValue("clicked");

    const out = await gateComputerAction({
      sbo3l,
      block: mouseClick(),
      agentId: "research-agent-01",
      executor,
    });

    expect(executor).toHaveBeenCalledOnce();
    expect(out.is_error).toBeUndefined();
    const parsed = JSON.parse(out.content);
    expect(parsed.ok).toBe(true);
    expect(parsed.output).toBe("clicked");
    expect(parsed.audit_event_id).toBe("evt-1");
    expect(parsed.action_class).toBe("computer.mouse");
  });

  it("on deny: SKIPS executor + returns deny envelope", async () => {
    fetchOnce(denyResponse("policy.deny_dangerous_bash", "evt-deny-2"));
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    const executor = vi.fn();

    const out = await gateComputerAction({
      sbo3l,
      block: bash(),
      agentId: "research-agent-01",
      executor,
    });

    expect(executor).not.toHaveBeenCalled();
    expect(out.is_error).toBe(true);
    const parsed = JSON.parse(out.content);
    expect(parsed.error).toBe("policy.deny");
    expect(parsed.deny_code).toBe("policy.deny_dangerous_bash");
    expect(parsed.audit_event_id).toBe("evt-deny-2");
    expect(parsed.action_class).toBe("bash.exec");
  });

  it("executor throw on allow path → executor.failed envelope w/ audit_event_id preserved", async () => {
    fetchOnce(allowResponse("evt-allow-3"));
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    const executor = vi.fn().mockRejectedValue(new Error("xdotool failed"));

    const out = await gateComputerAction({
      sbo3l,
      block: mouseClick(),
      agentId: "research-agent-01",
      executor,
    });

    expect(out.is_error).toBe(true);
    const parsed = JSON.parse(out.content);
    expect(parsed.error).toBe("executor.failed");
    expect(parsed.detail).toContain("xdotool failed");
    expect(parsed.audit_event_id).toBe("evt-allow-3");
  });

  it("transport failure → transport.failed envelope, executor never invoked", async () => {
    globalThis.fetch = vi.fn().mockRejectedValue(new TypeError("network down"));
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    const executor = vi.fn();

    const out = await gateComputerAction({
      sbo3l,
      block: bash(),
      agentId: "research-agent-01",
      executor,
    });

    expect(executor).not.toHaveBeenCalled();
    expect(out.is_error).toBe(true);
    const parsed = JSON.parse(out.content);
    expect(["transport.failed", "transport.unknown"]).toContain(parsed.error);
    expect(parsed.action_class).toBe("bash.exec");
  });

  it("idempotencyKey callback is forwarded into submit", async () => {
    fetchOnce(allowResponse());
    const sbo3l = new SBO3LClient({ endpoint: ENDPOINT });
    const executor = vi.fn().mockResolvedValue("ok");
    const seen: string[] = [];

    await gateComputerAction({
      sbo3l,
      block: bash("specific-id"),
      agentId: "research-agent-01",
      executor,
      idempotencyKey: (a) => {
        const key = `key-${a.task_id}`;
        seen.push(key);
        return key;
      },
    });
    expect(seen).toEqual(["key-cu-specific-id"]);
  });
});
