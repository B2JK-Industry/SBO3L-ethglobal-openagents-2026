import { defineConfig } from 'astro/config';
import sitemap from '@astrojs/sitemap';

// SBO3L marketing site config.
//
// Static-only build. No SSR, no Vercel adapter needed — Vercel auto-serves
// the `dist/` output. Strict CSP enforced via apps/marketing/vercel.json.
//
// i18n: English at root (/), other locales at /<code>/. 20 locales total
// across three batches: EN+SK+KO baseline (R9), Latin batch DE/FR/IT/ES/
// PT-BR/PL/CS/HU (R13 P4a), and RTL+CJK+Cyrillic+Devanagari+Thai+Turkish
// batch AR/HE/ZH-CN/ZH-TW/RU/UK/TR/HI/TH (R13 P4b).
//
// AR + HE are RTL — layouts read `isRtlLocale(locale)` from src/i18n
// and emit `dir="rtl"` on <html>. Brand-critical phrases carry
// _TODO_<LOCALE>_REVIEW_* markers; markers filtered at lookup time.
//
// Astro 5 docs: https://docs.astro.build/en/reference/configuration-reference/
export default defineConfig({
  output: 'static',
  site: 'https://sbo3l-marketing.vercel.app',
  trailingSlash: 'never',
  i18n: {
    defaultLocale: 'en',
    locales: [
      'en', 'sk', 'ko', 'ja',
      'de', 'fr', 'it', 'es', 'pt-br', 'pl', 'cs', 'hu',
      'ar', 'he', 'zh-cn', 'zh-tw', 'ru', 'uk', 'tr', 'hi', 'th',
    ],
    // Codex review fix (PR #284 + #296): we declared 17 locales in
    // the array but only `sk/` and `ko/` page directories exist on
    // disk. Without a fallback mapping, navigating to /de/, /ja/,
    // /ar/, etc. via the LocaleSwitcher would 404. Fall back to EN
    // content so the URLs resolve; i18n strings still pull from
    // each locale's JSON via the `t()` helper in src/i18n/index.ts,
    // so headers + CTAs translate even when the page body itself
    // is the English file.
    fallback: {
      sk: 'en', ko: 'en', ja: 'en',
      de: 'en', fr: 'en', it: 'en', es: 'en', 'pt-br': 'en',
      pl: 'en', cs: 'en', hu: 'en',
      ar: 'en', he: 'en', 'zh-cn': 'en', 'zh-tw': 'en',
      ru: 'en', uk: 'en', tr: 'en', hi: 'en', th: 'en',
    },
    routing: {
      prefixDefaultLocale: false,
      fallbackType: 'redirect',
    },
  },
  // @astrojs/sitemap auto-generates sitemap-index.xml + sitemap-N.xml
  // from every static route at build. Linked from public/robots.txt
  // so well-behaved crawlers (Google, Bing, judge link-checkers)
  // discover the per-page OG images + sponsor sub-pages without
  // having to walk the index.
  integrations: [
    sitemap({
      filter: (page) => !page.includes('/og/'),
    }),
  ],
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
