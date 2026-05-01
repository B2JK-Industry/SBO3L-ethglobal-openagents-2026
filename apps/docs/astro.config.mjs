import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

// SBO3L docs site config.
//
// Static-only build. Pagefind built-in search (zero-JS-on-load, indexed
// at build time, same-origin). Strict CSP enforced via vercel.json.
//
// Sidebar mirrors docs/design/phase-2-frontend.md §4.2 sitemap. Most
// sections are scaffolded with "Coming soon" stubs in this prep PR;
// content port lands in CTI-3-3 main.
export default defineConfig({
  output: "static",
  site: "https://sbo3l-docs.vercel.app",
  trailingSlash: "never",
  integrations: [
    starlight({
      title: "SBO3L Docs",
      description: "Documentation for the SBO3L agent trust layer.",
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026",
        },
      ],
      customCss: ["./src/styles/custom.css"],
      sidebar: [
        { label: "Quickstart", link: "/quickstart" },
        {
          label: "Concepts",
          items: [
            { label: "Overview", link: "/concepts" },
            { label: "APRP wire format", link: "/concepts/aprp" },
            { label: "Audit log", link: "/concepts/audit-log" },
            { label: "Capsule v2", link: "/concepts/capsule" },
            { label: "Signing model", link: "/concepts/signing" },
            { label: "Sponsor adapters", link: "/concepts/sponsor-adapters" },
            { label: "Idempotency", link: "/concepts/idempotency" },
            { label: "Audit replay", link: "/concepts/audit-replay" },
            { label: "Policy decision", link: "/concepts/policy", badge: { text: "soon", variant: "note" } },
            { label: "Multi-scope budget", link: "/concepts/budget", badge: { text: "soon", variant: "note" } },
            { label: "Trust DNS", link: "/concepts/trust-dns", badge: { text: "soon", variant: "note" } },
          ],
        },
        {
          label: "SDKs",
          items: [
            { label: "Overview", link: "/sdks" },
            { label: "TypeScript", link: "/sdks/typescript", badge: { text: "soon", variant: "note" } },
            { label: "Python", link: "/sdks/python", badge: { text: "soon", variant: "note" } },
          ],
        },
        {
          label: "CLI reference",
          items: [
            { label: "Overview", link: "/cli" },
            { label: "passport run", link: "/cli/passport-run" },
            { label: "passport verify", link: "/cli/passport-verify" },
            { label: "audit export-bundle", link: "/cli/audit-export-bundle" },
            { label: "agent register", link: "/cli/agent-register" },
            { label: "agent verify-ens", link: "/cli/agent-verify-ens" },
          ],
        },
        { label: "API reference", link: "/api" },
        { label: "Examples", link: "/examples" },
        { label: "Integrations", link: "/integrations" },
        {
          label: "Reference",
          items: [
            { label: "Overview", link: "/reference" },
            { label: "Error codes", link: "/reference/errors", badge: { text: "soon", variant: "note" } },
            { label: "Schemas", link: "/reference/schemas", badge: { text: "soon", variant: "note" } },
            { label: "Security notes", link: "/reference/security", badge: { text: "soon", variant: "note" } },
          ],
        },
      ],
      pagefind: true,
      lastUpdated: true,
      pagination: false,
      tableOfContents: { minHeadingLevel: 2, maxHeadingLevel: 4 },
    }),
  ],
});
