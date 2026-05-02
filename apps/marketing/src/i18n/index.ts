// i18n helper. Loads all locale JSON files and exposes `t(key, locale)`.
// Astro pages read `locale` from `Astro.currentLocale` (set by the
// i18n config in astro.config.mjs) and pass it through.
//
// Brand-critical phrases in non-EN locales carry `_TODO_<LOCALE>_REVIEW_*`
// markers for native-speaker review. The `t()` lookup ignores keys
// starting with underscore so markers stay in JSON without leaking
// into rendered UI. Drafts via DeepL + manual fluency pass.
//
// RTL locales (ar, he) opt-in via `_meta.rtl: true` in their JSON;
// `isRtlLocale()` reads that flag so layouts set `dir="rtl"` on <html>.

import en from "./en.json";
import sk from "./sk.json";
import ko from "./ko.json";
import ja from "./ja.json";
import de from "./de.json";
import fr from "./fr.json";
import it from "./it.json";
import es from "./es.json";
import ptBr from "./pt-br.json";
import pl from "./pl.json";
import cs from "./cs.json";
import hu from "./hu.json";
import ar from "./ar.json";
import he from "./he.json";
import zhCn from "./zh-cn.json";
import zhTw from "./zh-tw.json";
import ru from "./ru.json";
import uk from "./uk.json";
import tr from "./tr.json";
import hi from "./hi.json";
import th from "./th.json";

export const LOCALES = [
  "en", "sk", "ko", "ja",
  "de", "fr", "it", "es", "pt-br", "pl", "cs", "hu",
  "ar", "he", "zh-cn", "zh-tw", "ru", "uk", "tr", "hi", "th",
] as const;
export type Locale = (typeof LOCALES)[number];
export const DEFAULT_LOCALE: Locale = "en";

const dictionaries = {
  en, sk, ko, ja,
  de, fr, it, es, "pt-br": ptBr, pl, cs, hu,
  ar, he, "zh-cn": zhCn, "zh-tw": zhTw, ru, uk, tr, hi, th,
} as const;

function lookup(dict: unknown, path: string[]): string | undefined {
  let cursor: unknown = dict;
  for (const segment of path) {
    if (typeof cursor !== "object" || cursor === null) return undefined;
    if (segment.startsWith("_")) return undefined;
    cursor = (cursor as Record<string, unknown>)[segment];
  }
  return typeof cursor === "string" ? cursor : undefined;
}

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

export function isRtlLocale(locale: Locale): boolean {
  const meta = (dictionaries[locale] as { _meta?: { rtl?: boolean } })._meta;
  return meta?.rtl === true;
}
