import { mountGraph } from "./graph";
import { mockSource, realWebSocketSource } from "./source";

const params = new URLSearchParams(location.search);
const isEmbed = params.get("embed") === "1";
const wsUrl = params.get("ws");
const useMock = params.get("mock") === "1" || wsUrl === null;

if (isEmbed) {
  document.getElementById("chrome")?.classList.add("embed");
}

const svg = document.getElementById("viz") as SVGSVGElement | null;
const status = document.getElementById("status");
if (!svg) throw new Error("missing #viz element");

const graph = mountGraph(svg);
const source = useMock ? mockSource() : realWebSocketSource(wsUrl ?? "");

source.start(
  (e) => graph.apply(e),
  (s) => {
    if (status) {
      status.textContent = useMock ? `mock · ${s}` : s;
      status.className = s;
    }
  },
);

window.addEventListener("beforeunload", () => {
  source.stop();
  graph.destroy();
});
