import { isVizEvent, type VizEvent } from "./events";

// Event source: real WebSocket against Dev 1's daemon endpoint, OR a
// mock generator for standalone preview. Selected by the `mock=1`
// query param OR absence of `ws=` URL.

export interface EventSource {
  start(
    onEvent: (e: VizEvent) => void,
    onStatus: (s: "connecting" | "connected" | "disconnected") => void,
  ): void;
  stop(): void;
}

/// Reconnect schedule for `realWebSocketSource`: exponential backoff
/// starting at 1s, doubling up to a 30s cap. Reset to 1s on every
/// successful `open`. The schedule is exported so callers + tests
/// can introspect it; the actual impl multiplies the previous delay
/// by 2 rather than indexing this array, but the values are the
/// same.
export const RECONNECT_SCHEDULE_MS = [1_000, 2_000, 4_000, 8_000, 16_000, 30_000] as const;

/// Cap on the reconnect delay in ms. Once we hit this, the delay
/// stays here until a successful `open` resets the schedule.
export const RECONNECT_CAP_MS = 30_000;

/// Initial reconnect delay (ms). The first close → reconnect attempt
/// uses this; subsequent attempts double until the cap.
export const RECONNECT_INITIAL_MS = 1_000;

/// "Stale stream" threshold (ms). If the connection is open but we
/// haven't seen any frame for this long, the source preemptively
/// closes + reconnects. Daemons can drop a TCP connection silently
/// (load balancer, NAT idle timeout); without this watchdog the viz
/// would sit on a dead socket forever.
export const STALE_FRAME_THRESHOLD_MS = 60_000;

/// Optional knobs for `realWebSocketSource`. All defaults match the
/// pinned constants above; tests override to drive the watchdog
/// + reconnect path on a sub-second budget.
export interface RealWebSocketOptions {
  /// Override the WebSocket constructor — tests inject a fake.
  /// Defaults to the global `WebSocket`.
  webSocketCtor?: typeof WebSocket;
  /// Initial reconnect delay (ms). Defaults to
  /// [`RECONNECT_INITIAL_MS`].
  initialDelayMs?: number;
  /// Reconnect cap (ms). Defaults to [`RECONNECT_CAP_MS`].
  capDelayMs?: number;
  /// Stale-frame watchdog threshold (ms). Defaults to
  /// [`STALE_FRAME_THRESHOLD_MS`]. Set to `0` to disable the
  /// watchdog (used by the connect-then-close test where we want
  /// to assert reconnect timing without watchdog interference).
  staleThresholdMs?: number;
}

export function realWebSocketSource(
  url: string,
  opts: RealWebSocketOptions = {},
): EventSource {
  const WebSocketCtor: typeof WebSocket = opts.webSocketCtor ?? WebSocket;
  const initialDelay = opts.initialDelayMs ?? RECONNECT_INITIAL_MS;
  const capDelay = opts.capDelayMs ?? RECONNECT_CAP_MS;
  const staleThreshold = opts.staleThresholdMs ?? STALE_FRAME_THRESHOLD_MS;

  let ws: WebSocket | null = null;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  let staleTimer: ReturnType<typeof setTimeout> | null = null;
  let nextDelay = initialDelay;
  let stopped = false;

  return {
    start(onEvent, onStatus) {
      stopped = false;
      const armStaleWatchdog = (): void => {
        if (staleThreshold <= 0) return;
        if (staleTimer !== null) clearTimeout(staleTimer);
        staleTimer = setTimeout(() => {
          // No frame for `staleThreshold` ms while the socket
          // believes itself open. Force a reconnect — the close
          // handler will schedule it. Logging surfaces the event
          // so an operator sees why the viz blipped.
          // eslint-disable-next-line no-console
          console.warn(
            `[trust-dns-viz] stale stream — no frame for ${staleThreshold}ms; reconnecting`,
          );
          ws?.close();
        }, staleThreshold);
      };

      const connect = (): void => {
        if (stopped) return;
        onStatus("connecting");
        ws = new WebSocketCtor(url);
        ws.addEventListener("open", () => {
          onStatus("connected");
          // Reset backoff after a successful open. A future close
          // schedules from `initialDelay` again.
          nextDelay = initialDelay;
          armStaleWatchdog();
        });
        ws.addEventListener("message", (msg: MessageEvent) => {
          // Reset the stale watchdog on every frame — the connection
          // is provably alive while the daemon is publishing.
          armStaleWatchdog();
          try {
            const parsed: unknown = JSON.parse(msg.data as string);
            if (isVizEvent(parsed)) onEvent(parsed);
          } catch {
            /* ignore malformed payload */
          }
        });
        ws.addEventListener("close", () => {
          if (staleTimer !== null) {
            clearTimeout(staleTimer);
            staleTimer = null;
          }
          onStatus("disconnected");
          if (stopped) return;
          // Schedule the next reconnect with the current delay,
          // then double for the next attempt — capped at
          // `capDelay`. A successful open resets to `initialDelay`.
          const delay = nextDelay;
          nextDelay = Math.min(nextDelay * 2, capDelay);
          reconnectTimer = setTimeout(connect, delay);
        });
        ws.addEventListener("error", () => {
          // `error` events on WebSockets always also trigger a
          // close, so the close handler does the scheduling. We
          // only need to ensure the stale timer is canceled.
          if (staleTimer !== null) {
            clearTimeout(staleTimer);
            staleTimer = null;
          }
        });
      };
      connect();
    },
    stop() {
      stopped = true;
      if (reconnectTimer !== null) {
        clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
      if (staleTimer !== null) {
        clearTimeout(staleTimer);
        staleTimer = null;
      }
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
      onEvent({
        kind: "agent.discovered",
        agent_id: id,
        ens_name: `${id}.sbo3lagent.eth`,
        pubkey_b58: `mock-pubkey-${step}`,
        ts_ms: now,
      });
    } else {
      const i = step % AGENTS.length;
      const a = AGENTS[i]!;
      const b = AGENTS[(i + 1 + (step % 3)) % AGENTS.length]!;
      const which = step % 3;
      if (which === 0)
        onEvent({
          kind: "attestation.signed",
          from: a,
          to: b,
          attestation_id: `att-${step}`,
          ts_ms: now,
        });
      else if (which === 1) onEvent({ kind: "decision.made", agent_id: a, decision: "allow", ts_ms: now });
      else
        onEvent({
          kind: "decision.made",
          agent_id: a,
          decision: "deny",
          deny_code: "policy.deny_unknown_provider",
          ts_ms: now,
        });
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
