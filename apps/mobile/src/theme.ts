// Mirrors the design tokens in packages/design-tokens. Kept inline
// here because Metro doesn't traverse workspace edges automatically.
// When we publish design-tokens to npm we'll switch to importing
// from @sbo3l/design-tokens directly.

export const tokens = {
  bg: "#0a0e1a",
  fg: "#f5f5f5",
  muted: "#9ca3af",
  border: "#1f2937",
  codeBg: "#111827",
  accent: "#4ade80",
  deny: "#f87171",
  fontMono: "ui-monospace",
  rSm: 4,
  rMd: 8,
  rLg: 12,
} as const;
