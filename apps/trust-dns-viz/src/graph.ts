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

interface Node extends SimulationNodeDatum {
  id: string;
  label: string;
  decision?: "allow" | "deny";
  chainLength?: number;
}

interface Edge extends SimulationLinkDatum<Node> {
  source: string | Node;
  target: string | Node;
  signed?: boolean;
}

export interface Graph {
  apply(e: VizEvent): void;
  destroy(): void;
}

export function mountGraph(svg: SVGSVGElement): Graph {
  const sel = select(svg);
  const width = svg.clientWidth || 800;
  const height = svg.clientHeight || 600;

  const nodes: Node[] = [];
  const edges: Edge[] = [];

  const linkLayer = sel.append("g").attr("class", "edges");
  const nodeLayer = sel.append("g").attr("class", "nodes");

  const sim: Simulation<Node, Edge> = forceSimulation(nodes)
    .force(
      "link",
      forceLink<Node, Edge>(edges).id((n) => n.id).distance(110),
    )
    .force("charge", forceManyBody().strength(-260))
    .force("center", forceCenter(width / 2, height / 2))
    .alphaDecay(0.04)
    .on("tick", render);

  function render(): void {
    const e = linkLayer
      .selectAll<SVGLineElement, Edge>("line.edge")
      .data(edges, (d) => `${nodeId(d.source)}-${nodeId(d.target)}`);
    e.enter()
      .append("line")
      .attr("class", (d) => `edge${d.signed ? " signed" : ""}`)
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
      .call(installDrag(sim));
    nEnter.append("circle").attr("r", 18);
    nEnter
      .append("text")
      .attr("text-anchor", "middle")
      .attr("dy", 30)
      .text((d) => d.label);
    n.merge(nEnter).attr("class", (d) => nodeClass(d)).attr("transform", (d) => `translate(${d.x ?? 0},${d.y ?? 0})`);
    n.exit().remove();
  }

  function pulse(id: string, decision: "allow" | "deny"): void {
    const node = nodes.find((x) => x.id === id);
    if (!node) return;
    node.decision = decision;
    render();
    window.setTimeout(() => {
      node.decision = undefined;
      render();
    }, 800);
  }

  return {
    apply(e: VizEvent) {
      if (e.kind === "agent.discovered") {
        if (!nodes.some((x) => x.id === e.agent_id)) {
          nodes.push({ id: e.agent_id, label: e.ens_name.split(".")[0] ?? e.agent_id });
        }
      } else if (e.kind === "attestation.signed") {
        edges.push({ source: e.from, target: e.to, signed: true });
      } else if (e.kind === "decision.made") {
        pulse(e.agent_id, e.decision);
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
