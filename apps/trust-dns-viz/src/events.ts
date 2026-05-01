// Event protocol for the trust-dns visualisation.
//
// Server side (Dev 1's `crates/sbo3l-server/src/ws_events.rs`, T-3-5
// backend slice) emits one of these per agent event over a WebSocket.
// Until that lands, mock-source.ts simulates the same protocol so the
// frontend is fully testable in isolation.

export type VizEvent =
  | {
      kind: "agent.discovered";
      agent_id: string;
      ens_name: string;
      pubkey_b58: string;
      ts_ms: number;
    }
  | {
      kind: "attestation.signed";
      from: string;
      to: string;
      attestation_id: string;
      ts_ms: number;
    }
  | {
      kind: "decision.made";
      agent_id: string;
      decision: "allow" | "deny";
      deny_code?: string;
      ts_ms: number;
    }
  | {
      kind: "audit.checkpoint";
      agent_id: string;
      chain_length: number;
      root_hash: string;
      ts_ms: number;
    };

export function isVizEvent(value: unknown): value is VizEvent {
  if (typeof value !== "object" || value === null) return false;
  const v = value as { kind?: unknown };
  return (
    v.kind === "agent.discovered" ||
    v.kind === "attestation.signed" ||
    v.kind === "decision.made" ||
    v.kind === "audit.checkpoint"
  );
}
