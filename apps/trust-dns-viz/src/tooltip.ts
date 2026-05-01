// Tooltip overlay — single floating div, repositioned on hover.
// Pure DOM, no framework. Pulls token colours from CSS variables.

export interface Tooltip {
  show(html: string, x: number, y: number): void;
  hide(): void;
  destroy(): void;
}

export function mountTooltip(host: HTMLElement): Tooltip {
  const el = document.createElement("div");
  el.className = "viz-tooltip";
  el.setAttribute("role", "tooltip");
  el.setAttribute("aria-hidden", "true");
  host.append(el);

  const reposition = (x: number, y: number): void => {
    const rect = host.getBoundingClientRect();
    const tipW = el.offsetWidth || 240;
    const tipH = el.offsetHeight || 60;
    const px = Math.min(Math.max(x - rect.left + 14, 0), rect.width - tipW - 8);
    const py = Math.min(Math.max(y - rect.top + 14, 0), rect.height - tipH - 8);
    el.style.transform = `translate(${px}px, ${py}px)`;
  };

  return {
    show(html, x, y) {
      el.innerHTML = html;
      el.classList.add("visible");
      el.setAttribute("aria-hidden", "false");
      reposition(x, y);
    },
    hide() {
      el.classList.remove("visible");
      el.setAttribute("aria-hidden", "true");
    },
    destroy() {
      el.remove();
    },
  };
}

export function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
