import {
  forceCenter,
  forceLink,
  forceManyBody,
  forceSimulation,
  type Simulation,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";
import { select, type Selection } from "d3-selection";
import { drag, type D3DragEvent } from "d3-drag";
import type { VizEvent } from "./events";
import { escapeHtml, mountTooltip, type Tooltip } from "./tooltip";

interface Node extends SimulationNodeDatum {
  id: string;
  label: string;
  ensName: string;
  pubkey: string;
  decision?: "allow" | "deny";
  denyCode?: string;
  chainLength?: number;
}

interface Edge extends SimulationLinkDatum<Node> {
  source: string | Node;
  target: string | Node;
  attestationId?: string;
  signedAtMs?: number;
  signed?: boolean;
}

export interface Graph {
  apply(e: VizEvent): void;
  destroy(): void;
}

const CANVAS_THRESHOLD = 200;
const REDUCED_MOTION = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const PULSE_DURATION_MS = REDUCED_MOTION ? 0 : 800;
const NODE_R = window.matchMedia("(max-width: 640px)").matches ? 14 : 18;

export function mountGraph(svg: SVGSVGElement): Graph {
  const host = svg.parentElement ?? document.body;
  const tip = mountTooltip(host);
  const sel = select(svg);
  const width = svg.clientWidth || 800;
  const height = svg.clientHeight || 600;

  const nodes: Node[] = [];
  const edges: Edge[] = [];

  const linkLayer = sel.append("g").attr("class", "edges");
  const nodeLayer = sel.append("g").attr("class", "nodes");

  const sim: Simulation<Node, Edge> = forceSimulation(nodes)
    .force("link", forceLink<Node, Edge>(edges).id((n) => n.id).distance(110))
    .force("charge", forceManyBody().strength(-260))
    .force("center", forceCenter(width / 2, height / 2))
    .alphaDecay(0.04)
    .on("tick", render);

  function checkCanvasThreshold(): void {
    if (nodes.length === CANVAS_THRESHOLD + 1) {
      console.warn(
        `[trust-dns-viz] node count exceeded ${CANVAS_THRESHOLD}; SVG renderer may drop frames. Canvas fallback tracked under T-3-5 main follow-up.`,
      );
    }
  }

  function render(): void {
    const e = linkLayer
      .selectAll<SVGLineElement, Edge>("line.edge")
      .data(edges, (d) => `${nodeId(d.source)}-${nodeId(d.target)}-${d.attestationId ?? ""}`);
    e.enter()
      .append("line")
      .attr("class", (d) => `edge${d.signed ? " signed" : ""}`)
      .on("mouseenter", (ev, d) => showEdgeTip(ev, d))
      .on("mouseleave", () => tip.hide())
      .merge(e as never)
      .attr("x1", (d) => (d.source as Node).x ?? 0)
      .attr("y1", (d) => (d.source as Node).y ?? 0)
      .attr("x2", (d) => (d.target as Node).x ?? 0)
      .attr("y2", (d) => (d.target as Node).y ?? 0);
    e.exit().remove();

    const n = nodeLayer
      .selectAll<SVGGElement, Node>("g.node")
      .data(nodes, (d) => d.id);
    const nEnter = n
      .enter()
      .append("g")
      .attr("class", (d) => nodeClass(d))
      .call(installDrag(sim))
      .on("mouseenter", (ev, d) => showNodeTip(ev, d))
      .on("mouseleave", () => tip.hide())
      .on("focus", (ev, d) => showNodeTip(ev, d))
      .on("blur", () => tip.hide())
      .attr("tabindex", 0);
    nEnter.append("circle").attr("r", NODE_R);
    nEnter
      .append("text")
      .attr("text-anchor", "middle")
      .attr("dy", NODE_R + 12)
      .text((d) => d.label);
    n.merge(nEnter)
      .attr("class", (d) => nodeClass(d))
      .attr("transform", (d) => `translate(${d.x ?? 0},${d.y ?? 0})`);
    n.exit().remove();
  }

  function showNodeTip(ev: MouseEvent | FocusEvent, d: Node): void {
    const html = `
      <strong>${escapeHtml(d.ensName || d.id)}</strong><br>
      <span class="muted">pubkey:</span> <code>${escapeHtml(d.pubkey.slice(0, 18))}…</code>
      ${d.chainLength !== undefined ? `<br><span class="muted">audit chain:</span> ${d.chainLength}` : ""}
      ${d.denyCode ? `<br><span class="fail">last deny:</span> <code>${escapeHtml(d.denyCode)}</code>` : ""}
    `;
    const x = "clientX" in ev ? ev.clientX : 0;
    const y = "clientY" in ev ? ev.clientY : 0;
    tip.show(html, x, y);
  }

  function showEdgeTip(ev: MouseEvent, d: Edge): void {
    if (!d.attestationId) return;
    const ts = d.signedAtMs ? new Date(d.signedAtMs).toISOString() : "—";
    const html = `
      <strong>attestation</strong><br>
      <code>${escapeHtml(d.attestationId)}</code><br>
      <span class="muted">signed:</span> <code>${escapeHtml(ts)}</code>
    `;
    tip.show(html, ev.clientX, ev.clientY);
  }

  function pulse(id: string, decision: "allow" | "deny", denyCode?: string): void {
    const node = nodes.find((x) => x.id === id);
    if (!node) return;
    node.decision = decision;
    if (denyCode) node.denyCode = denyCode;
    render();
    if (PULSE_DURATION_MS > 0) {
      window.setTimeout(() => {
        node.decision = undefined;
        render();
      }, PULSE_DURATION_MS);
    }
  }

  return {
    apply(e: VizEvent) {
      if (e.kind === "agent.discovered") {
        if (!nodes.some((x) => x.id === e.agent_id)) {
          nodes.push({
            id: e.agent_id,
            label: e.ens_name.split(".")[0] ?? e.agent_id,
            ensName: e.ens_name,
            pubkey: e.pubkey_b58,
          });
          checkCanvasThreshold();
        }
      } else if (e.kind === "attestation.signed") {
        edges.push({
          source: e.from,
          target: e.to,
          signed: true,
          attestationId: e.attestation_id,
          signedAtMs: e.ts_ms,
        });
      } else if (e.kind === "decision.made") {
        pulse(e.agent_id, e.decision, e.deny_code);
      } else if (e.kind === "audit.checkpoint") {
        const node = nodes.find((x) => x.id === e.agent_id);
        if (node) node.chainLength = e.chain_length;
      }
      sim.nodes(nodes);
      (sim.force("link") as ReturnType<typeof forceLink>).links(edges);
      sim.alpha(0.4).restart();
    },
    destroy() {
      sim.stop();
      tip.destroy();
      sel.selectAll("*").remove();
    },
  };
}

function nodeClass(d: Node): string {
  return `node${d.decision ? ` ${d.decision}` : ""}`;
}

function nodeId(ref: string | Node): string {
  return typeof ref === "string" ? ref : ref.id;
}

function installDrag(sim: Simulation<Node, Edge>): (sel: Selection<SVGGElement, Node, SVGGElement, unknown>) => void {
  return (sel) =>
    sel.call(
      drag<SVGGElement, Node>()
        .on("start", (event: D3DragEvent<SVGGElement, Node, Node>, d) => {
          if (!event.active) sim.alphaTarget(0.3).restart();
          d.fx = d.x;
          d.fy = d.y;
        })
        .on("drag", (event: D3DragEvent<SVGGElement, Node, Node>, d) => {
          d.fx = event.x;
          d.fy = event.y;
        })
        .on("end", (event: D3DragEvent<SVGGElement, Node, Node>, d) => {
          if (!event.active) sim.alphaTarget(0);
          d.fx = null;
          d.fy = null;
        }),
    );
}
