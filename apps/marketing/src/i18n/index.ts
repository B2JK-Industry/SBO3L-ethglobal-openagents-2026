// Tiny i18n helper. Loads en + sk + ko JSON and exposes a `t(key, locale)`
// function. Astro pages read `locale` from `Astro.currentLocale`
// (set by the i18n config in astro.config.mjs) and pass it through.
//
// Korean (ko) ships in v1.2.0 with brand-critical phrases marked
// `_TODO_KO_REVIEW_*` for native-speaker review on next pass; the
// `t()` lookup ignores keys starting with underscore so the markers
// don't leak into rendered UI. Translations elsewhere were drafted
// against DeepL and proofread for fluency.

import en from "./en.json";
import sk from "./sk.json";
import ko from "./ko.json";

export const LOCALES = ["en", "sk", "ko"] as const;
export type Locale = (typeof LOCALES)[number];
export const DEFAULT_LOCALE: Locale = "en";

const dictionaries = { en, sk, ko } as const;

function lookup(dict: unknown, path: string[]): string | undefined {
  let cursor: unknown = dict;
  for (const segment of path) {
    if (typeof cursor !== "object" || cursor === null) return undefined;
    // Skip `_TODO_KO_REVIEW_*` review markers — they're translator
    // notes, not user-visible strings.
    if (segment.startsWith("_")) return undefined;
    cursor = (cursor as Record<string, unknown>)[segment];
  }
  return typeof cursor === "string" ? cursor : undefined;
}

// `t("section.key", "sk")` → string. Falls back to en if the key is
// missing in the requested locale, then returns the dotted path so
// missing-key bugs are visible in the UI rather than silently empty.
export function t(key: string, locale: Locale = DEFAULT_LOCALE): string {
  const path = key.split(".");
  return lookup(dictionaries[locale], path) ?? lookup(dictionaries[DEFAULT_LOCALE], path) ?? key;
}

export function localizedPath(path: string, locale: Locale): string {
  if (locale === DEFAULT_LOCALE) return path;
  return `/${locale}${path === "/" ? "" : path}`;
}

export function isLocale(value: string | undefined): value is Locale {
  return value !== undefined && (LOCALES as readonly string[]).includes(value);
}
