import { isVizEvent, type VizEvent } from "./events";

// Event source: real WebSocket against Dev 1's daemon endpoint, OR a
// mock generator for standalone preview. Selected by the `mock=1`
// query param OR absence of `ws=` URL.

export interface EventSource {
  start(onEvent: (e: VizEvent) => void, onStatus: (s: "connecting" | "connected" | "disconnected") => void): void;
  stop(): void;
}

export function realWebSocketSource(url: string): EventSource {
  let ws: WebSocket | null = null;
  let reconnectTimer: number | null = null;

  return {
    start(onEvent, onStatus) {
      const connect = (): void => {
        onStatus("connecting");
        ws = new WebSocket(url);
        ws.addEventListener("open", () => onStatus("connected"));
        ws.addEventListener("message", (msg) => {
          try {
            const parsed: unknown = JSON.parse(msg.data as string);
            if (isVizEvent(parsed)) onEvent(parsed);
          } catch {
            /* ignore malformed payload */
          }
        });
        ws.addEventListener("close", () => {
          onStatus("disconnected");
          reconnectTimer = window.setTimeout(connect, 2000);
        });
      };
      connect();
    },
    stop() {
      if (reconnectTimer !== null) window.clearTimeout(reconnectTimer);
      ws?.close();
    },
  };
}

const AGENTS = ["research-01", "trader-02", "auditor-03", "indexer-04", "router-05"];

// Mock event source — emits a deterministic-feeling sequence so the viz
// is demonstrable without a daemon. Replaced by real WebSocket once
// Dev 1 ships ws_events.rs.
export function mockSource(): EventSource {
  let timer: number | null = null;
  let step = 0;

  const emit = (onEvent: (e: VizEvent) => void): void => {
    const now = Date.now();
    if (step < AGENTS.length) {
      const id = AGENTS[step]!;
      onEvent({ kind: "agent.discovered", agent_id: id, ens_name: `${id}.sbo3lagent.eth`, pubkey_b58: `mock-pubkey-${step}`, ts_ms: now });
    } else {
      const i = step % AGENTS.length;
      const a = AGENTS[i]!;
      const b = AGENTS[(i + 1 + (step % 3)) % AGENTS.length]!;
      const which = step % 3;
      if (which === 0) onEvent({ kind: "attestation.signed", from: a, to: b, attestation_id: `att-${step}`, ts_ms: now });
      else if (which === 1) onEvent({ kind: "decision.made", agent_id: a, decision: "allow", ts_ms: now });
      else onEvent({ kind: "decision.made", agent_id: a, decision: "deny", deny_code: "policy.deny_unknown_provider", ts_ms: now });
    }
    step += 1;
  };

  return {
    start(onEvent, onStatus) {
      onStatus("connected");
      timer = window.setInterval(() => emit(onEvent), 1500);
      emit(onEvent);
    },
    stop() {
      if (timer !== null) window.clearInterval(timer);
    },
  };
}
