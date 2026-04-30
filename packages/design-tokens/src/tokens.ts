/**
 * SBO3L design tokens — typed JS surface mirroring tokens.css.
 *
 * For consumers that need values in JS (D3 force-graph node fills,
 * Astro component prop defaults, etc.). CSS-only consumers should
 * import "@sbo3l/design-tokens/css" instead.
 */

export const darkTokens = {
  colour: {
    bg:      '#0a0a0f',
    fg:      '#e6e6ec',
    muted:   '#9999a8',
    accent:  '#4ad6a7',
    codeBg:  '#14141c',
    border:  '#2a2a3a',
  },
  layout: {
    max:    '920px',
    maxApp: '1280px',
  },
  font: {
    sans: 'ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif',
    mono: 'ui-monospace, "SF Mono", Menlo, Consolas, monospace',
  },
  fontSize: {
    fs0: '0.85rem',
    fs1: '1rem',
    fs2: '1.15em',
    fs3: '1.7em',
    fs4: 'clamp(1.8em, 4vw, 2.6em)',
  },
  radius: {
    sm: '4px',
    md: '8px',
    lg: '12px',
  },
  lineHeight: {
    prose: 1.55,
    code:  1.4,
  },
} as const;

export const lightTokens = {
  ...darkTokens,
  colour: {
    bg:      '#ffffff',
    fg:      '#1a1a25',
    muted:   '#5a5a6a',
    accent:  '#1a8b6c',
    codeBg:  '#f5f5fa',
    border:  '#e0e0e8',
  },
} as const;

export type Tokens = typeof darkTokens;

export const tokens = darkTokens;
