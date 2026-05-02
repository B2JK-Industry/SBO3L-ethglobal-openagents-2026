import { mountGraph } from "./graph";
import { mountCanvasGraph } from "./canvas-renderer";
import { mockSource, realWebSocketSource } from "./source";

const CANVAS_THRESHOLD = 100;

const params = new URLSearchParams(location.search);
const isEmbed = params.get("embed") === "1";
const wsUrl = params.get("ws");
const useMock = params.get("mock") === "1" || wsUrl === null;

// Renderer choice: explicit ?renderer=canvas|svg overrides; otherwise
// look at ?nodes=N and switch to canvas above CANVAS_THRESHOLD; default
// is SVG (5-agent demo / unspecified counts).
const explicitRenderer = params.get("renderer");
const expectedNodes = Number.parseInt(params.get("nodes") ?? "0", 10);
const useCanvas =
  explicitRenderer === "canvas" ||
  (explicitRenderer !== "svg" && expectedNodes > CANVAS_THRESHOLD);

if (isEmbed) {
  document.getElementById("chrome")?.classList.add("embed");
}

const svg = document.getElementById("viz") as SVGSVGElement | null;
const canvas = document.getElementById("viz-canvas") as HTMLCanvasElement | null;
const status = document.getElementById("status");
if (!svg) throw new Error("missing #viz element");
if (!canvas) throw new Error("missing #viz-canvas element");

if (useCanvas) {
  svg.style.display = "none";
  canvas.style.display = "block";
} else {
  canvas.style.display = "none";
  svg.style.display = "block";
}

const graph = useCanvas ? mountCanvasGraph(canvas) : mountGraph(svg);
const source = useMock ? mockSource() : realWebSocketSource(wsUrl ?? "");

source.start(
  (e) => graph.apply(e),
  (s) => {
    if (status) {
      const renderer = useCanvas ? "canvas" : "svg";
      status.textContent = useMock ? `mock · ${renderer} · ${s}` : `${renderer} · ${s}`;
      status.className = s;
    }
  },
);

window.addEventListener("beforeunload", () => {
  source.stop();
  graph.destroy();
});
