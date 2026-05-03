// SBO3L /proof Service Worker — R22 PR2.
//
// Goal: make the Passport-capsule verifier installable + offline-capable.
// /proof is the page judges click to verify a capsule; the WASM bundle
// (~2.4 MB) is the heavy artifact. Cache-first for the WASM + JS shim
// + the page shell so a returning user can verify with no network.
//
// Strategy:
//   - Pre-cache shell on install (page HTML + WASM + JS shim + manifest + icons).
//   - Cache-first for /wasm/* + /icon-* + /manifest.json (rarely change;
//     if they do, we bump SW_VERSION and the new SW takes over).
//   - Network-first for HTML pages with cache fallback (so a navigation
//     while offline still resolves /proof from cache).
//   - Bypass non-GET, cross-origin, /capsules/* (rows are committed
//     fixtures, but we want the freshest copy if the user is online).
//
// Versioning: bumping SW_VERSION invalidates all caches on activate.
// CSP: vercel.json allows worker-src 'self'; SW is same-origin.

const SW_VERSION = "v1-2026-05-02";
const SHELL_CACHE = `sbo3l-shell-${SW_VERSION}`;
const RUNTIME_CACHE = `sbo3l-runtime-${SW_VERSION}`;

const SHELL_ASSETS = [
  "/proof",
  "/manifest.json",
  "/icon-192.png",
  "/icon-512.png",
  "/icon-512-maskable.png",
  "/favicon.svg",
  "/wasm/sbo3l_core.js",
  "/wasm/sbo3l_core_bg.wasm",
];

self.addEventListener("install", (event) => {
  // Self-review fix (PR #496): cache.addAll() is all-or-nothing — a
  // single 404 in SHELL_ASSETS would leave the cache empty and
  // offline mode broken. Use per-asset cache.add() wrapped in
  // allSettled so individual failures don't kill the rest. Logged
  // for observability when one drifts.
  event.waitUntil(
    caches.open(SHELL_CACHE).then((cache) =>
      Promise.allSettled(
        SHELL_ASSETS.map((url) =>
          cache.add(url).catch((err) => {
            // eslint-disable-next-line no-console
            console.warn(`[sbo3l-sw] shell asset failed: ${url}`, err);
            return null;
          })
        )
      )
    )
  );
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(
        keys
          .filter((k) => k !== SHELL_CACHE && k !== RUNTIME_CACHE)
          .map((k) => caches.delete(k))
      )
    )
  );
  self.clients.claim();
});

function isCacheFirst(url) {
  return (
    url.pathname.startsWith("/wasm/") ||
    url.pathname.startsWith("/icon-") ||
    url.pathname === "/manifest.json" ||
    url.pathname === "/favicon.svg"
  );
}

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;

  const url = new URL(req.url);
  if (url.origin !== self.location.origin) return;

  // Bypass committed capsule fixtures so judges always see the freshest
  // bytes if they refresh while online.
  if (url.pathname.startsWith("/capsules/")) return;

  if (isCacheFirst(url)) {
    event.respondWith(
      caches.match(req).then((cached) => {
        if (cached) return cached;
        return fetch(req).then((res) => {
          if (res && res.ok) {
            const copy = res.clone();
            caches.open(RUNTIME_CACHE).then((c) => c.put(req, copy));
          }
          return res;
        });
      })
    );
    return;
  }

  // Network-first for HTML so users see updated pages when online; fall
  // back to cached shell if offline. Limit fallback to navigation
  // requests so JSON / data files don't get stale shells.
  if (req.mode === "navigate" || (req.headers.get("accept") || "").includes("text/html")) {
    event.respondWith(
      fetch(req)
        .then((res) => {
          if (res && res.ok) {
            const copy = res.clone();
            caches.open(RUNTIME_CACHE).then((c) => c.put(req, copy));
          }
          return res;
        })
        .catch(() =>
          caches.match(req).then((cached) => cached || caches.match("/proof"))
        )
    );
  }
});
