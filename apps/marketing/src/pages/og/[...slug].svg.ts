// Per-page OG image endpoint. Static-generated at build time — Astro's
// default `output: 'static'` mode prerenders one .svg per entry in
// OG_PAGES. The route uses `[...slug]` so nested keys like
// "submission/keeperhub" become `/og/submission/keeperhub.svg`.
//
// The SVG is brand-styled, viewBox 1200×630, six variants. CSS-free
// (no <style> blocks because some social fetchers strip them) — every
// fill/stroke is a presentation attribute.
//
// R19 Wave 2 Task D · D4. Converted from /tmp/sbo3l-design-kit/assets/og.jsx.

import type { APIRoute } from "astro";
import { OG_PAGES, getOgMeta, type OgVariant } from "~/data/og-registry";

export function getStaticPaths() {
  return Object.keys(OG_PAGES).map((slug) => ({ params: { slug } }));
}

const ACCENT = "#4ad6a7";
const BG     = "#0a0a0f";
const FG     = "#e6e6ec";
const MUTED  = "#9999a8";
const BORDER = "#2a2a3a";

function escapeXml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

function variantSvg(variant: OgVariant): string {
  switch (variant) {
    case "default":
      return `<g transform="translate(720 120)">
        <rect x="40"  y="30" width="6" height="100" fill="${ACCENT}"/>
        <rect x="160" y="30" width="6" height="100" fill="${ACCENT}"/>
        <text x="103" y="92" text-anchor="middle" font-family="ui-monospace, monospace" font-size="56" font-weight="700" fill="${ACCENT}">3</text>
        <line x1="46" y1="80" x2="160" y2="80" stroke="${ACCENT}" stroke-width="1" stroke-dasharray="3 3"/>
        <rect x="220" y="40" width="100" height="70" rx="2" fill="none" stroke="${ACCENT}" stroke-width="2"/>
        <path d="M 220 40 L 270 80 L 320 40" fill="none" stroke="${ACCENT}" stroke-width="1.5"/>
        <circle cx="304" cy="96" r="11" fill="none" stroke="${ACCENT}" stroke-width="1.5"/>
        <text x="304" y="100" text-anchor="middle" font-family="ui-monospace, monospace" font-size="11" font-weight="700" fill="${ACCENT}">3</text>
      </g>`;

    case "proof":
      return `<g transform="translate(820 130)">
        <rect x="0" y="0" width="220" height="140" rx="4" fill="${BG}" stroke="${ACCENT}" stroke-width="1"/>
        <line x1="110" y1="0" x2="110" y2="140" stroke="${BORDER}" stroke-width="1"/>
        <text x="55"  y="22" text-anchor="middle" font-family="ui-monospace, monospace" font-size="9" fill="${MUTED}" letter-spacing="1.5">AGENT</text>
        <text x="165" y="22" text-anchor="middle" font-family="ui-monospace, monospace" font-size="9" fill="${MUTED}" letter-spacing="1.5">DECISION</text>
        ${[0,1,2,3,4,5].map((i) => `<circle cx="${20 + (i % 3) * 28}" cy="${50 + Math.floor(i / 3) * 28}" r="9" fill="none" stroke="${ACCENT}" stroke-width="1"/>`).join("")}
        <rect x="125" y="45" width="80" height="30" rx="2" fill="none" stroke="${ACCENT}" stroke-width="1.5"/>
        <text x="165" y="65" text-anchor="middle" font-family="ui-monospace, monospace" font-size="11" font-weight="700" fill="${ACCENT}" letter-spacing="2">ALLOW</text>
        <line x1="125" y1="90" x2="205" y2="90" stroke="${BORDER}" stroke-width="0.5"/>
        <line x1="125" y1="100" x2="195" y2="100" stroke="${BORDER}" stroke-width="0.5"/>
        <line x1="125" y1="110" x2="200" y2="110" stroke="${BORDER}" stroke-width="0.5"/>
      </g>`;

    case "status": {
      const cells: [string, string][] = [
        ["live", ACCENT], ["live", ACCENT], ["mock", "#d4a14a"], ["live", ACCENT],
        ["pending", MUTED], ["live", ACCENT], ["live", ACCENT], ["mock", "#d4a14a"],
      ];
      return `<g transform="translate(820 130)">
        <rect x="0" y="0" width="280" height="140" rx="4" fill="${BG}" stroke="${BORDER}" stroke-width="1"/>
        ${cells.map(([label, color], i) => `
          <g transform="translate(${(i % 4) * 70 + 12} ${Math.floor(i / 4) * 60 + 16})">
            <rect x="0" y="0" width="56" height="44" rx="2" fill="none" stroke="${color}" stroke-width="1"/>
            <circle cx="10" cy="14" r="3" fill="${color}"/>
            <text x="20" y="18" font-family="ui-monospace, monospace" font-size="9" fill="${FG}">${label}</text>
            <line x1="6" y1="28" x2="50" y2="28" stroke="${BORDER}" stroke-width="0.5"/>
            <line x1="6" y1="36" x2="40" y2="36" stroke="${BORDER}" stroke-width="0.5"/>
          </g>
        `).join("")}
      </g>`;
    }

    case "roadmap":
      return `<g transform="translate(820 130)">
        ${["NOW", "Q3", "Q4"].map((label, i) => `
          <g transform="translate(${i * 95} 0)">
            <rect x="0" y="0" width="80" height="140" rx="3" fill="${BG}" stroke="${i === 0 ? ACCENT : BORDER}" stroke-width="${i === 0 ? 1.5 : 1}"/>
            <text x="40" y="22" text-anchor="middle" font-family="ui-monospace, monospace" font-size="10" fill="${i === 0 ? ACCENT : MUTED}" letter-spacing="2">${label}</text>
            <line x1="10" y1="34" x2="70" y2="34" stroke="${BORDER}" stroke-width="0.5"/>
            ${[0,1,2,3].map((j) => `
              <circle cx="14" cy="${48 + j * 18}" r="2.5" fill="${i === 0 ? ACCENT : MUTED}"/>
              <line x1="20" y1="${48 + j * 18}" x2="${62 - j * 4}" y2="${48 + j * 18}" stroke="${i === 0 ? FG : BORDER}" stroke-width="1"/>
            `).join("")}
          </g>
        `).join("")}
      </g>`;

    case "playground":
      return `<g transform="translate(820 130)">
        <rect x="0" y="0" width="280" height="140" rx="4" fill="${BG}" stroke="${BORDER}" stroke-width="1"/>
        <rect x="0" y="0" width="280" height="20" rx="4" fill="${BORDER}"/>
        <circle cx="10" cy="10" r="3" fill="${MUTED}"/>
        <circle cx="22" cy="10" r="3" fill="${MUTED}"/>
        <circle cx="34" cy="10" r="3" fill="${MUTED}"/>
        <text x="60" y="14" font-family="ui-monospace, monospace" font-size="9" fill="${FG}">policy.toml</text>
        <text x="14" y="40"  font-family="ui-monospace, monospace" font-size="10" fill="${MUTED}">[allow]</text>
        <text x="14" y="56"  font-family="ui-monospace, monospace" font-size="10" fill="${ACCENT}">action = "swap"</text>
        <text x="14" y="72"  font-family="ui-monospace, monospace" font-size="10" fill="${MUTED}">max_value = 50000</text>
        <text x="14" y="88"  font-family="ui-monospace, monospace" font-size="10" fill="${MUTED}">slippage_bps = 50</text>
        <text x="14" y="104" font-family="ui-monospace, monospace" font-size="10" fill="${ACCENT}">mev_guard = true</text>
        <text x="14" y="120" font-family="ui-monospace, monospace" font-size="10" fill="${MUTED}">expires = "10m"</text>
      </g>`;

    case "sponsor":
      return `<g transform="translate(820 140)">
        <rect x="0" y="0" width="280" height="120" rx="4" fill="${BG}" stroke="${ACCENT}" stroke-width="1"/>
        <text x="20" y="30" font-family="ui-monospace, monospace" font-size="10" fill="${MUTED}" letter-spacing="2">SPONSOR TRACK</text>
        <line x1="20" y1="40" x2="120" y2="40" stroke="${ACCENT}" stroke-width="1"/>
        ${[0,1,2].map((i) => `
          <g transform="translate(20 ${56 + i * 18})">
            <circle cx="6" cy="0" r="5" fill="none" stroke="${ACCENT}" stroke-width="1.2"/>
            <path d="M 3 0 L 5 2 L 9 -2" fill="none" stroke="${ACCENT}" stroke-width="1"/>
            <line x1="20" y1="0" x2="240" y2="0" stroke="${BORDER}" stroke-width="0.5"/>
          </g>
        `).join("")}
      </g>`;
  }
}

export const GET: APIRoute = ({ params }) => {
  const slug = (params.slug ?? "default") as string;
  const meta = getOgMeta(slug);
  const variantBlock = variantSvg(meta.variant);
  const slugLabel = `/${slug.toUpperCase()}`;
  const patternId = `dot-${slug.replace(/[^a-z0-9]/gi, "-")}`;

  const svg = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 630" role="img" aria-label="${escapeXml(meta.title)} · SBO3L">
  <rect width="1200" height="630" fill="${BG}"/>
  <defs>
    <pattern id="${patternId}" x="0" y="0" width="32" height="32" patternUnits="userSpaceOnUse">
      <circle cx="1" cy="1" r="1" fill="${BORDER}"/>
    </pattern>
  </defs>
  <rect width="1200" height="630" fill="url(#${patternId})" opacity="0.5"/>

  <g transform="translate(60 60)">
    <rect x="0" y="0" width="48" height="48" rx="6" fill="${ACCENT}"/>
    <text x="24" y="34" text-anchor="middle" font-family="ui-monospace, monospace" font-size="28" font-weight="700" fill="${BG}">3</text>
    <text x="64" y="32" font-family="ui-monospace, monospace" font-size="22" fill="${FG}" letter-spacing="2">SBO3L</text>
  </g>

  ${variantBlock}

  <g transform="translate(60 280)">
    <text x="0" y="0" font-family="ui-monospace, monospace" font-size="14" fill="${ACCENT}" letter-spacing="3">${escapeXml(slugLabel)}</text>
    <text x="0" y="60" font-family="ui-sans-serif, system-ui, 'IBM Plex Sans', sans-serif" font-size="56" font-weight="600" fill="${FG}">${escapeXml(meta.title)}</text>
    <text x="0" y="110" font-family="ui-sans-serif, system-ui, 'IBM Plex Sans', sans-serif" font-size="22" fill="${MUTED}">${escapeXml(meta.subtitle)}</text>
  </g>

  <g transform="translate(870 540)">
    <rect x="0" y="0" width="280" height="50" rx="3" fill="none" stroke="${BORDER}" stroke-width="1"/>
    <line x1="0" y1="0" x2="8" y2="0" stroke="${ACCENT}" stroke-width="2"/>
    <line x1="0" y1="0" x2="0" y2="8" stroke="${ACCENT}" stroke-width="2"/>
    <line x1="280" y1="50" x2="272" y2="50" stroke="${ACCENT}" stroke-width="2"/>
    <line x1="280" y1="50" x2="280" y2="42" stroke="${ACCENT}" stroke-width="2"/>
    <text x="14" y="22" font-family="ui-monospace, monospace" font-size="11" fill="${MUTED}" letter-spacing="2">ETHGLOBAL</text>
    <text x="14" y="40" font-family="ui-monospace, monospace" font-size="13" fill="${FG}" letter-spacing="1">OPEN AGENTS · 2026</text>
  </g>

  <text x="60" y="570" font-family="ui-monospace, monospace" font-size="13" fill="${MUTED}">Don't give your agent a wallet. Give it a mandate.</text>
</svg>`;

  return new Response(svg, {
    status: 200,
    headers: {
      "content-type": "image/svg+xml; charset=utf-8",
      "cache-control": "public, max-age=31536000, immutable",
    },
  });
};
