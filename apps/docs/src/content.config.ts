import { defineCollection, z } from "astro:content";
import { docsLoader } from "@astrojs/starlight/loaders";
import { docsSchema } from "@astrojs/starlight/schema";

// Frank's standing rule (03-agents.md line 263):
//
//   "Every doc starts with audience + outcome.
//    'This page is for: agent developers. Outcome: you'll have a working
//    agent in 5 minutes.'"
//
// We make this a hard contract by extending Starlight's docsSchema with
// required `audience` and `outcome` fields. Astro fails the build if any
// .md / .mdx entry in src/content/docs/ omits them — Heidi can paste-run
// the failure and reject the PR before it gets reviewed.
//
// Optional `prereqs` is for guides that build on each other.
export const collections = {
  docs: defineCollection({
    loader: docsLoader(),
    schema: docsSchema({
      extend: z.object({
        audience: z
          .string()
          .min(1, "Frank rule: every doc must declare its audience"),
        outcome: z
          .string()
          .min(1, "Frank rule: every doc must declare its outcome"),
        prereqs: z.array(z.string()).optional(),
      }),
    }),
  }),
};
