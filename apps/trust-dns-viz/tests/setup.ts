// Vitest setup — polyfills the bits of the browser env that jsdom
// doesn't ship by default. Loaded automatically by `vitest.config.ts`
// for every test file.
//
// Why this is needed: `src/graph.ts` calls `window.matchMedia` at
// *module-load time* (the `REDUCED_MOTION` and `NODE_R` consts), so
// merely importing the module under jsdom throws. We give it a
// minimal shim that always reports "match: false" — the production
// code's reduced-motion / mobile-breakpoint paths gracefully degrade
// when the media query doesn't match.

if (typeof window !== "undefined" && typeof window.matchMedia !== "function") {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {
        /* legacy noop */
      },
      removeListener: () => {
        /* legacy noop */
      },
      addEventListener: () => {
        /* noop */
      },
      removeEventListener: () => {
        /* noop */
      },
      dispatchEvent: () => false,
    }),
  });
}
