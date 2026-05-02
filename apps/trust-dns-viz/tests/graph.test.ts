import { describe, expect, it, beforeEach } from "vitest";
import { mountGraph } from "../src/graph";

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

describe("mountGraph — out-of-order edge guard", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("does not throw when an edge arrives before its source/target nodes", () => {
    const svg = createSvg();
    const graph = mountGraph(svg);

    expect(() => {
      graph.apply({
        kind: "attestation.signed",
        from: "alpha",
        to: "beta",
        attestation_id: "att-orphan",
        ts_ms: 1000,
      });
    }).not.toThrow();

    // Edge held in the pending buffer; no DOM line yet.
    expect(svg.querySelectorAll("line.edge").length).toBe(0);

    graph.destroy();
  });

  it("promotes a pending edge once both endpoint nodes arrive", () => {
    const svg = createSvg();
    const graph = mountGraph(svg);

    graph.apply({
      kind: "attestation.signed",
      from: "alpha",
      to: "beta",
      attestation_id: "att-promotable",
      ts_ms: 1000,
    });
    expect(svg.querySelectorAll("line.edge").length).toBe(0);

    graph.apply({
      kind: "agent.discovered",
      agent_id: "alpha",
      ens_name: "alpha.sbo3lagent.eth",
      pubkey_b58: "kA",
      ts_ms: 1100,
    });
    // Only one endpoint exists; edge stays pending.
    expect(svg.querySelectorAll("line.edge").length).toBe(0);

    graph.apply({
      kind: "agent.discovered",
      agent_id: "beta",
      ens_name: "beta.sbo3lagent.eth",
      pubkey_b58: "kB",
      ts_ms: 1200,
    });
    // Both endpoints present — edge promoted, DOM line rendered.
    expect(svg.querySelectorAll("line.edge").length).toBe(1);

    graph.destroy();
  });

  it("survives a mid-stream reconnect that interleaves edges and nodes", () => {
    const svg = createSvg();
    const graph = mountGraph(svg);

    // Initial connection — 2 nodes, 1 edge.
    graph.apply({ kind: "agent.discovered", agent_id: "n1", ens_name: "n1.eth", pubkey_b58: "k1", ts_ms: 1 });
    graph.apply({ kind: "agent.discovered", agent_id: "n2", ens_name: "n2.eth", pubkey_b58: "k2", ts_ms: 2 });
    graph.apply({ kind: "attestation.signed", from: "n1", to: "n2", attestation_id: "att-1", ts_ms: 3 });
    expect(svg.querySelectorAll("line.edge").length).toBe(1);

    // Reconnect — replay arrives out of order: edge before node.
    expect(() => {
      graph.apply({ kind: "attestation.signed", from: "n3", to: "n1", attestation_id: "att-2", ts_ms: 10 });
      graph.apply({ kind: "attestation.signed", from: "n2", to: "n3", attestation_id: "att-3", ts_ms: 11 });
      graph.apply({ kind: "agent.discovered", agent_id: "n3", ens_name: "n3.eth", pubkey_b58: "k3", ts_ms: 12 });
    }).not.toThrow();

    // After n3 lands, both buffered edges promote.
    expect(svg.querySelectorAll("line.edge").length).toBe(3);

    graph.destroy();
  });
});
