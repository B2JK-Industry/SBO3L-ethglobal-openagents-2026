import {
  forceCenter,
  forceLink,
  forceManyBody,
  forceSimulation,
  type Simulation,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";
import { drag as d3drag, type D3DragEvent } from "d3-drag";
import { select } from "d3-selection";
import type { VizEvent } from "./events";
import { escapeHtml, mountTooltip } from "./tooltip";

// Canvas renderer — drop-in alternative to graph.ts's SVG renderer.
// Same Graph interface, same d3-force simulation, same tooltip + drag
// interactions. Targeted for ≥ 100 agents where SVG starts dropping
// frames.
//
// Hit testing is brute-force linear over the node array. At 500 nodes
// with one mousemove per frame at 60fps that's 30k checks/sec — fast
// enough on modern hardware. Above ~2000 nodes a quadtree from
// d3-quadtree would be the next step.

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

const REDUCED_MOTION = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const PULSE_DURATION_MS = REDUCED_MOTION ? 0 : 800;
const NODE_R = window.matchMedia("(max-width: 640px)").matches ? 10 : 12;
const EDGE_HIT_TOLERANCE = 6;

export function mountCanvasGraph(canvas: HTMLCanvasElement): Graph {
  const host = canvas.parentElement ?? document.body;
  const tip = mountTooltip(host);
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("canvas 2D context unavailable");

  const styles = getComputedStyle(canvas);
  const palette = {
    bg: styles.getPropertyValue("--bg").trim() || "#0a0a0f",
    fg: styles.getPropertyValue("--fg").trim() || "#e6e6ec",
    muted: styles.getPropertyValue("--muted").trim() || "#9999a8",
    accent: styles.getPropertyValue("--accent").trim() || "#4ad6a7",
    border: styles.getPropertyValue("--border").trim() || "#2a2a3a",
    deny: "#ff6b6b",
  };

  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  let width = canvas.clientWidth || 800;
  let height = canvas.clientHeight || 600;
  resizeCanvas();

  const nodes: Node[] = [];
  const nodeIndex = new Map<string, Node>();
  const edges: Edge[] = [];
  const pendingEdges: Edge[] = [];

  const sim: Simulation<Node, Edge> = forceSimulation(nodes)
    .force("link", forceLink<Node, Edge>(edges).id((n) => n.id).distance(80))
    .force("charge", forceManyBody().strength(-180))
    .force("center", forceCenter(width / 2, height / 2))
    .alphaDecay(0.03)
    .on("tick", scheduleDraw);

  let drawScheduled = false;
  function scheduleDraw(): void {
    if (drawScheduled) return;
    drawScheduled = true;
    requestAnimationFrame(draw);
  }

  function draw(): void {
    drawScheduled = false;
    ctx!.clearRect(0, 0, width, height);

    // edges
    for (const e of edges) {
      const s = e.source as Node;
      const t = e.target as Node;
      ctx!.beginPath();
      ctx!.moveTo(s.x ?? 0, s.y ?? 0);
      ctx!.lineTo(t.x ?? 0, t.y ?? 0);
      ctx!.strokeStyle = e.signed ? palette.accent : palette.border;
      ctx!.lineWidth = 1.2;
      ctx!.stroke();
    }

    // nodes
    for (const n of nodes) {
      ctx!.beginPath();
      ctx!.arc(n.x ?? 0, n.y ?? 0, NODE_R, 0, Math.PI * 2);
      if (n.decision === "allow") ctx!.fillStyle = palette.accent;
      else if (n.decision === "deny") ctx!.fillStyle = palette.deny;
      else ctx!.fillStyle = palette.bg;
      ctx!.fill();
      ctx!.strokeStyle = palette.accent;
      ctx!.lineWidth = 1.6;
      ctx!.stroke();
    }
  }

  function resizeCanvas(): void {
    width = canvas.clientWidth || 800;
    height = canvas.clientHeight || 600;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
  }
  const resize = (): void => {
    resizeCanvas();
    sim.force("center", forceCenter(width / 2, height / 2)).alpha(0.2).restart();
  };
  window.addEventListener("resize", resize);

  // Hit testing — find nearest node or edge under a given client point.
  function nearestNode(cx: number, cy: number): Node | null {
    const rect = canvas.getBoundingClientRect();
    const x = cx - rect.left;
    const y = cy - rect.top;
    for (const n of nodes) {
      const dx = (n.x ?? 0) - x;
      const dy = (n.y ?? 0) - y;
      if (dx * dx + dy * dy <= NODE_R * NODE_R) return n;
    }
    return null;
  }
  function nearestEdge(cx: number, cy: number): Edge | null {
    const rect = canvas.getBoundingClientRect();
    const px = cx - rect.left;
    const py = cy - rect.top;
    for (const e of edges) {
      const s = e.source as Node;
      const t = e.target as Node;
      const sx = s.x ?? 0;
      const sy = s.y ?? 0;
      const tx = t.x ?? 0;
      const ty = t.y ?? 0;
      const dx = tx - sx;
      const dy = ty - sy;
      const len2 = dx * dx + dy * dy;
      if (len2 === 0) continue;
      const t01 = Math.max(0, Math.min(1, ((px - sx) * dx + (py - sy) * dy) / len2));
      const ex = sx + t01 * dx;
      const ey = sy + t01 * dy;
      const ddx = px - ex;
      const ddy = py - ey;
      if (ddx * ddx + ddy * ddy <= EDGE_HIT_TOLERANCE * EDGE_HIT_TOLERANCE) return e;
    }
    return null;
  }

  function showNodeTip(ev: MouseEvent, n: Node): void {
    const html = `
      <strong>${escapeHtml(n.ensName || n.id)}</strong><br>
      <span class="muted">pubkey:</span> <code>${escapeHtml(n.pubkey.slice(0, 18))}…</code>
      ${n.chainLength !== undefined ? `<br><span class="muted">audit chain:</span> ${n.chainLength}` : ""}
      ${n.denyCode ? `<br><span class="fail">last deny:</span> <code>${escapeHtml(n.denyCode)}</code>` : ""}
    `;
    tip.show(html, ev.clientX, ev.clientY);
  }
  function showEdgeTip(ev: MouseEvent, e: Edge): void {
    if (!e.attestationId) return;
    const ts = e.signedAtMs ? new Date(e.signedAtMs).toISOString() : "—";
    tip.show(
      `<strong>attestation</strong><br><code>${escapeHtml(e.attestationId)}</code><br><span class="muted">signed:</span> <code>${escapeHtml(ts)}</code>`,
      ev.clientX,
      ev.clientY,
    );
  }

  const onMove = (ev: MouseEvent): void => {
    const node = nearestNode(ev.clientX, ev.clientY);
    if (node) { showNodeTip(ev, node); canvas.style.cursor = "grab"; return; }
    const edge = nearestEdge(ev.clientX, ev.clientY);
    if (edge) { showEdgeTip(ev, edge); canvas.style.cursor = "pointer"; return; }
    tip.hide();
    canvas.style.cursor = "";
  };
  canvas.addEventListener("mousemove", onMove);
  canvas.addEventListener("mouseleave", () => tip.hide());

  // Drag — d3-drag works on any element; we hand it a node lookup.
  select(canvas).call(
    d3drag<HTMLCanvasElement, unknown>()
      .subject((event: D3DragEvent<HTMLCanvasElement, unknown, Node>) => nearestNode(event.sourceEvent.clientX, event.sourceEvent.clientY))
      .on("start", (event: D3DragEvent<HTMLCanvasElement, unknown, Node>) => {
        if (!event.subject) return;
        if (!event.active) sim.alphaTarget(0.3).restart();
        event.subject.fx = event.subject.x;
        event.subject.fy = event.subject.y;
      })
      .on("drag", (event: D3DragEvent<HTMLCanvasElement, unknown, Node>) => {
        if (!event.subject) return;
        event.subject.fx = event.x;
        event.subject.fy = event.y;
      })
      .on("end", (event: D3DragEvent<HTMLCanvasElement, unknown, Node>) => {
        if (!event.subject) return;
        if (!event.active) sim.alphaTarget(0);
        event.subject.fx = null;
        event.subject.fy = null;
      }),
  );

  function drainPendingEdges(): void {
    if (pendingEdges.length === 0) return;
    const stillPending: Edge[] = [];
    for (const edge of pendingEdges) {
      const sId = typeof edge.source === "string" ? edge.source : edge.source.id;
      const tId = typeof edge.target === "string" ? edge.target : edge.target.id;
      if (nodeIndex.has(sId) && nodeIndex.has(tId)) edges.push(edge);
      else stillPending.push(edge);
    }
    pendingEdges.length = 0;
    pendingEdges.push(...stillPending);
  }

  function pulse(id: string, decision: "allow" | "deny", denyCode?: string): void {
    const node = nodeIndex.get(id);
    if (!node) return;
    node.decision = decision;
    if (denyCode) node.denyCode = denyCode;
    scheduleDraw();
    if (PULSE_DURATION_MS > 0) {
      window.setTimeout(() => { node.decision = undefined; scheduleDraw(); }, PULSE_DURATION_MS);
    }
  }

  return {
    apply(e) {
      if (e.kind === "agent.discovered") {
        if (!nodeIndex.has(e.agent_id)) {
          const node: Node = {
            id: e.agent_id,
            label: e.ens_name.split(".")[0] ?? e.agent_id,
            ensName: e.ens_name,
            pubkey: e.pubkey_b58,
          };
          nodes.push(node);
          nodeIndex.set(e.agent_id, node);
          drainPendingEdges();
        }
      } else if (e.kind === "attestation.signed") {
        const edge: Edge = {
          source: e.from, target: e.to, signed: true,
          attestationId: e.attestation_id, signedAtMs: e.ts_ms,
        };
        if (nodeIndex.has(e.from) && nodeIndex.has(e.to)) edges.push(edge);
        else pendingEdges.push(edge);
      } else if (e.kind === "decision.made") {
        pulse(e.agent_id, e.decision, e.deny_code);
      } else if (e.kind === "audit.checkpoint") {
        const node = nodeIndex.get(e.agent_id);
        if (node) node.chainLength = e.chain_length;
      }
      sim.nodes(nodes);
      (sim.force("link") as ReturnType<typeof forceLink>).links(edges);
      sim.alpha(0.4).restart();
    },
    destroy() {
      sim.stop();
      tip.destroy();
      window.removeEventListener("resize", resize);
      canvas.removeEventListener("mousemove", onMove);
      ctx!.clearRect(0, 0, width, height);
    },
  };
}
