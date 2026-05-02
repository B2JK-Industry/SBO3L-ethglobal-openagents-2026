import { defineConfig } from 'astro/config';

// SBO3L marketing site config.
//
// Static-only build. No SSR, no Vercel adapter needed — Vercel auto-serves
// the `dist/` output. Strict CSP enforced via apps/marketing/vercel.json.
//
// i18n: English at root (/), other locales at /<code>/. Latin batch
// (R13 P4a): de, fr, it, es, pt-br, pl, cs, hu. Brand-critical
// phrases in non-EN locales carry _TODO_<LOCALE>_REVIEW_* markers
// for native-speaker review; markers filtered at lookup time
// (see src/i18n/index.ts).
//
// Astro 5 docs: https://docs.astro.build/en/reference/configuration-reference/
export default defineConfig({
  output: 'static',
  site: 'https://sbo3l-marketing.vercel.app',
  trailingSlash: 'never',
  i18n: {
    defaultLocale: 'en',
    locales: ['en', 'sk', 'ko', 'de', 'fr', 'it', 'es', 'pt-br', 'pl', 'cs', 'hu'],
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
