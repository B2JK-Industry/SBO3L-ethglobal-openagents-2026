import { defineConfig } from 'astro/config';

// SBO3L marketing site config.
//
// Static-only build. No SSR, no Vercel adapter needed — Vercel auto-serves
// the `dist/` output. Strict CSP enforced via apps/marketing/vercel.json.
//
// i18n: English at root (/), Slovak at /sk/, Korean at /ko/. Korean
// brand-critical phrases marked _TODO_KO_REVIEW_* in src/i18n/ko.json
// pending native-speaker review (markers are filtered out at lookup
// time — see src/i18n/index.ts).
//
// Astro 5 docs: https://docs.astro.build/en/reference/configuration-reference/
export default defineConfig({
  output: 'static',
  site: 'https://sbo3l-marketing.vercel.app',
  trailingSlash: 'never',
  i18n: {
    defaultLocale: 'en',
    locales: ['en', 'sk', 'ko'],
    routing: {
      prefixDefaultLocale: false,
    },
  },
  build: {
    format: 'file',
    assets: '_astro',
  },
  compressHTML: true,
  prefetch: false,
  vite: {
    build: {
      cssCodeSplit: true,
    },
  },
});
