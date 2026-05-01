/** @type {import("next").NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // Workspace-internal SBO3L packages — let Next bundle their TS sources directly.
  transpilePackages: ["@sbo3l/sdk", "@sbo3l/vercel-ai"],
};

export default nextConfig;
