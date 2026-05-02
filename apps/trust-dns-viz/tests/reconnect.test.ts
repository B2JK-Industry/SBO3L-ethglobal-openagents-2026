// Tests for `realWebSocketSource` — exponential-backoff reconnect +
// stale-frame watchdog.
//
// We don't mount the graph here; this is purely about the source's
// connect / close / reconnect behavior. The test injects a fake
// WebSocket constructor via `RealWebSocketOptions.webSocketCtor` so
// every connection attempt is observable + drivable from the test
// (no real network, no jsdom WebSocket — vitest's jsdom doesn't
// ship one anyway).

import { describe, expect, it, beforeEach, afterEach, vi } from "vitest";
import {
  RECONNECT_CAP_MS,
  RECONNECT_INITIAL_MS,
  realWebSocketSource,
} from "../src/source";

type FakeListener = (ev: unknown) => void;

class FakeWebSocket {
  static instances: FakeWebSocket[] = [];
  url: string;
  listeners: Map<string, FakeListener[]> = new Map();
  closed = false;

  constructor(url: string) {
    this.url = url;
    FakeWebSocket.instances.push(this);
  }
  addEventListener(kind: string, fn: FakeListener): void {
    const arr = this.listeners.get(kind) ?? [];
    arr.push(fn);
    this.listeners.set(kind, arr);
  }
  removeEventListener(): void {
    /* not used in source.ts */
  }
  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.fire("close", { code: 1006 });
  }
  // Test-only helpers — drive the lifecycle.
  open(): void {
    this.fire("open", {});
  }
  message(payload: unknown): void {
    this.fire("message", { data: JSON.stringify(payload) });
  }
  fire(kind: string, ev: unknown): void {
    for (const fn of this.listeners.get(kind) ?? []) fn(ev);
  }
  static reset(): void {
    FakeWebSocket.instances = [];
  }
  static last(): FakeWebSocket {
    const last = FakeWebSocket.instances[FakeWebSocket.instances.length - 1];
    if (!last) throw new Error("no FakeWebSocket instances yet");
    return last;
  }
}

describe("realWebSocketSource — exponential-backoff reconnect", () => {
  beforeEach(() => {
    FakeWebSocket.reset();
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("schedules first reconnect at 1s after a clean close", () => {
    const source = realWebSocketSource("ws://daemon/v1/events", {
      webSocketCtor: FakeWebSocket as unknown as typeof WebSocket,
      staleThresholdMs: 0, // disable watchdog so the test only
      // exercises the reconnect-after-close path.
    });
    const events: unknown[] = [];
    const statuses: string[] = [];
    source.start(
      (e) => events.push(e),
      (s) => statuses.push(s),
    );

    expect(FakeWebSocket.instances.length).toBe(1);
    FakeWebSocket.last().open();
    expect(statuses).toEqual(["connecting", "connected"]);

    // Drop the connection. The source should set a 1s reconnect
    // timer; advancing 999ms should NOT yet have fired it; 1000ms
    // should.
    FakeWebSocket.last().close();
    expect(statuses[statuses.length - 1]).toBe("disconnected");
    expect(FakeWebSocket.instances.length).toBe(1);

    vi.advanceTimersByTime(999);
    expect(FakeWebSocket.instances.length).toBe(1);
    vi.advanceTimersByTime(1);
    expect(FakeWebSocket.instances.length).toBe(2);

    source.stop();
  });

  it("doubles the delay after each subsequent close: 1s, 2s, 4s, 8s, 16s, 30s", () => {
    const source = realWebSocketSource("ws://daemon/v1/events", {
      webSocketCtor: FakeWebSocket as unknown as typeof WebSocket,
      staleThresholdMs: 0,
    });
    source.start(
      () => {
        /* noop */
      },
      () => {
        /* noop */
      },
    );

    // No `open` between closes — that mimics a daemon that's
    // straight-up unreachable. The schedule must double each time
    // and cap at 30s.
    const expected = [1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000];
    for (let i = 0; i < expected.length; i += 1) {
      const before = FakeWebSocket.instances.length;
      FakeWebSocket.last().close();
      const delay = expected[i]!;
      // One ms before the timer should NOT have produced a new
      // socket; advancing by 1 ms more should.
      vi.advanceTimersByTime(delay - 1);
      expect(FakeWebSocket.instances.length).toBe(before);
      vi.advanceTimersByTime(1);
      expect(FakeWebSocket.instances.length).toBe(before + 1);
    }

    source.stop();
  });

  it("resets the backoff schedule on a successful open", () => {
    const source = realWebSocketSource("ws://daemon/v1/events", {
      webSocketCtor: FakeWebSocket as unknown as typeof WebSocket,
      staleThresholdMs: 0,
    });
    source.start(
      () => {},
      () => {},
    );

    // Burn through to a 4s delay…
    FakeWebSocket.last().close();
    vi.advanceTimersByTime(1_000);
    FakeWebSocket.last().close();
    vi.advanceTimersByTime(2_000);
    // …open + close once more. The reconnect should now be back
    // at 1s, NOT 8s.
    FakeWebSocket.last().open();
    FakeWebSocket.last().close();
    const before = FakeWebSocket.instances.length;
    vi.advanceTimersByTime(999);
    expect(FakeWebSocket.instances.length).toBe(before);
    vi.advanceTimersByTime(1);
    expect(FakeWebSocket.instances.length).toBe(before + 1);

    source.stop();
  });
});

describe("realWebSocketSource — stale-frame watchdog", () => {
  beforeEach(() => {
    FakeWebSocket.reset();
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("forces a reconnect when no frame arrives within the stale threshold", () => {
    const source = realWebSocketSource("ws://daemon/v1/events", {
      webSocketCtor: FakeWebSocket as unknown as typeof WebSocket,
      staleThresholdMs: 60_000,
      initialDelayMs: 1, // tighten reconnect so the test asserts
      // the watchdog-then-reconnect chain quickly.
    });
    source.start(
      () => {},
      () => {},
    );
    FakeWebSocket.last().open();
    expect(FakeWebSocket.instances.length).toBe(1);

    // Watchdog at 60s. Advance 60s → close fires → reconnect
    // schedules at +1ms (from initialDelayMs) → new socket.
    vi.advanceTimersByTime(60_000);
    expect(FakeWebSocket.last().closed).toBe(true);
    vi.advanceTimersByTime(1);
    expect(FakeWebSocket.instances.length).toBe(2);

    source.stop();
  });

  it("a frame resets the watchdog so the connection survives past the threshold", () => {
    const source = realWebSocketSource("ws://daemon/v1/events", {
      webSocketCtor: FakeWebSocket as unknown as typeof WebSocket,
      staleThresholdMs: 60_000,
    });
    source.start(
      () => {},
      () => {},
    );
    const ws = FakeWebSocket.last();
    ws.open();

    // Advance 30s, send a frame, advance 30s more — total 60s of
    // wall time but the watchdog should NOT have fired because
    // the message reset it.
    vi.advanceTimersByTime(30_000);
    ws.message({
      kind: "decision.made",
      agent_id: "a",
      decision: "allow",
      ts_ms: 1,
    });
    vi.advanceTimersByTime(30_000);
    expect(ws.closed).toBe(false);
    expect(FakeWebSocket.instances.length).toBe(1);

    source.stop();
  });
});

describe("realWebSocketSource — kill-and-reconnect within 5s warm-path budget", () => {
  beforeEach(() => {
    FakeWebSocket.reset();
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  // The brief's success criterion: kill server mid-stream,
  // reconnect within 5s on warm path. Warm-path = the previous
  // connection had a successful open, so the backoff schedule
  // resets to 1s.
  it("reconnects within 5 seconds of a mid-stream kill (warm path)", () => {
    const source = realWebSocketSource("ws://daemon/v1/events", {
      webSocketCtor: FakeWebSocket as unknown as typeof WebSocket,
      staleThresholdMs: 0,
    });
    const events: unknown[] = [];
    source.start(
      (e) => events.push(e),
      () => {},
    );
    FakeWebSocket.last().open();
    FakeWebSocket.last().message({
      kind: "agent.discovered",
      agent_id: "first",
      ens_name: "first.sbo3lagent.eth",
      pubkey_b58: "p",
      ts_ms: 1,
    });
    expect(events.length).toBe(1);

    // Kill mid-stream.
    FakeWebSocket.last().close();
    // Within 5s budget the source must have spawned a new socket.
    vi.advanceTimersByTime(5_000);
    expect(FakeWebSocket.instances.length).toBeGreaterThanOrEqual(2);

    // Walk-through: the new socket can immediately be opened +
    // resume publishing, with the same handler chain.
    FakeWebSocket.last().open();
    FakeWebSocket.last().message({
      kind: "agent.discovered",
      agent_id: "second",
      ens_name: "second.sbo3lagent.eth",
      pubkey_b58: "p2",
      ts_ms: 2,
    });
    expect(events.length).toBe(2);

    source.stop();
  });
});

describe("RECONNECT_* constants are exported and well-formed", () => {
  it("initial < cap and cap == 30s", () => {
    expect(RECONNECT_INITIAL_MS).toBeLessThan(RECONNECT_CAP_MS);
    expect(RECONNECT_CAP_MS).toBe(30_000);
  });
});
