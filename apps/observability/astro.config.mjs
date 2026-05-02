import { defineConfig } from "astro/config";
import react from "@astrojs/react";

// Static site — deploys to any CDN (Vercel, Netlify, GitHub Pages, S3+CF).
// We render React islands only for the chart panels; the page chrome is
// pure Astro for fast first paint.
export default defineConfig({
  output: "static",
  integrations: [react()],
  vite: {
    // Recharts ships ESM; keep the bundler happy.
    ssr: { noExternal: ["recharts"] },
  },
});
