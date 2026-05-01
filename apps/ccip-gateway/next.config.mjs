// CCIP-Read gateway is API-only. No client-side React, no images, no
// CSS — just route handlers under app/api/. Server output deploys
// straight to Vercel functions.

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // Belt-and-suspenders: lock the workspace root so Next doesn't
  // accidentally resolve files from sibling apps when monorepo
  // tooling expands.
  outputFileTracingRoot: new URL("./", import.meta.url).pathname,
  // No image optimization needed — gateway has no UI.
  images: {
    unoptimized: true,
  },
};

export default nextConfig;
