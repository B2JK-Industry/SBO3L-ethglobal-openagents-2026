// T-3-5 viz-side end-to-end test. Sister to
// `crates/sbo3l-server/tests/ws_events_e2e.rs`: confirms the
// frontend's `mockSource` → `mountGraph` loop renders five distinct
// agent nodes within the 10-second budget the brief calls out.
//
// Why mock and not the real WebSocket: vitest's environment is jsdom,
// which doesn't ship a `WebSocket` server. The mock source emits
// VizEvents that are byte-identical to what `realWebSocketSource`
// receives over the wire (same `events.ts` discriminant enum, same
// `isVizEvent` guard), so a green test here proves the graph layer
// handles the contract; a green test in `ws_events_e2e.rs` proves the
// daemon emits that same contract. The two together pin the loop.
//
// Time discipline: jsdom timers are real-time-driven by default, but
// vitest's `vi.useFakeTimers()` advances them deterministically. We
// drive the mock's 1.5s tick interval forward without sleeping, so
// the test completes in milliseconds despite asserting "within 10s
// of wall time".

import { describe, expect, it, beforeEach, afterEach, vi } from "vitest";
import { mountGraph } from "../src/graph";
import { mockSource } from "../src/source";
import type { VizEvent } from "../src/events";

const SVG_NS = "http://www.w3.org/2000/svg";

function createSvg(): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg") as SVGSVGElement;
  svg.setAttribute("width", "800");
  svg.setAttribute("height", "600");
  Object.defineProperty(svg, "clientWidth", { value: 800, configurable: true });
  Object.defineProperty(svg, "clientHeight", { value: 600, configurable: true });
  document.body.append(svg);
  return svg;
}

describe("T-3-5 e2e — viz consumes source and renders 5 agents", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders 5 distinct agent nodes from the mock source within 10s", () => {
    const svg = createSvg();
    const graph = mountGraph(svg);
    const source = mockSource();

    const seenStatus: string[] = [];
    const seenEvents: VizEvent[] = [];
    source.start(
      (event) => {
        seenEvents.push(event);
        graph.apply(event);
      },
      (status) => {
        seenStatus.push(status);
      },
    );

    // Mock fires once immediately on `start` (the first
    // `agent.discovered`), then again every 1500 ms. Five agents
    // (first immediate + four more at 1.5s intervals) take 6 seconds
    // of simulated time. Advance one tick at a time so the
    // simulation faithfully replays the wire schedule rather than
    // batching all 5 emits in a single zero-time burst.
    for (let i = 0; i < 4; i += 1) {
      vi.advanceTimersByTime(1500);
    }

    // Assertion 1: the source emitted at least 5 agent.discovered
    // frames. (The brief says "5 nodes appear within 10s"; with the
    // mock that's exactly 5 agent.discovered events.)
    const discovered = seenEvents.filter((e) => e.kind === "agent.discovered");
    expect(discovered.length).toBeGreaterThanOrEqual(5);

    // Assertion 2: the graph rendered 5 distinct DOM circles. The
    // graph layer dedupes by agent_id, so seeing 5 nodes proves
    // each emitted agent_id was distinct AND each was applied.
    const circles = svg.querySelectorAll("g.node");
    expect(circles.length).toBeGreaterThanOrEqual(5);

    // Assertion 3: status callback fired with "connected" — the
    // viz UI uses this to paint the live indicator. A regression
    // that drops the status callback shows up here as a missing
    // entry.
    expect(seenStatus).toContain("connected");

    source.stop();
    graph.destroy();
  });

  it("budgets at most 10 seconds of simulated time to reach 5 nodes", () => {
    const svg = createSvg();
    const graph = mountGraph(svg);
    const source = mockSource();
    const events: VizEvent[] = [];
    source.start(
      (e) => {
        events.push(e);
        graph.apply(e);
      },
      () => {
        /* status not asserted here */
      },
    );

    // Walk forward in 500 ms steps until the graph carries 5 nodes
    // OR we hit the 10-second budget. The early-exit semantic
    // matches what a real demo viewer experiences: as soon as 5
    // nodes are on screen the test passes; only a regression that
    // delays an emit past 10s should fail.
    let elapsedMs = 0;
    const budgetMs = 10_000;
    const stepMs = 500;
    let nodes = svg.querySelectorAll("g.node").length;
    while (nodes < 5 && elapsedMs < budgetMs) {
      vi.advanceTimersByTime(stepMs);
      elapsedMs += stepMs;
      nodes = svg.querySelectorAll("g.node").length;
    }

    expect(nodes).toBeGreaterThanOrEqual(5);
    expect(elapsedMs).toBeLessThan(budgetMs);

    source.stop();
    graph.destroy();
  });
});
