import { defineCollection, z } from "astro:content";

// Blog collection — long-form essays + manifestos.
//
// Frank's standing rule (03-agents.md line 263): every doc starts with
// audience + outcome. Enforced here as required schema fields; CI fails if
// a post is missing them.
const blog = defineCollection({
  type: "content",
  schema: z.object({
    title: z.string().min(1),
    description: z.string().min(1),
    audience: z.string().min(1),
    outcome: z.string().min(1),
    pubDate: z.coerce.date(),
    updatedDate: z.coerce.date().optional(),
    draft: z.boolean().default(false),
    tags: z.array(z.string()).default([]),
  }),
});

// Submissions collection — per-partner bounty one-pagers materialised
// from docs/submission/bounty-*.md by scripts/copy-bounty-pages.mjs at
// prebuild. The source-of-truth lives in docs/submission/ alongside the
// rest of the submission pack; the marketing site mirrors them so the
// content site can render via Astro's content pipeline.
const submissions = defineCollection({
  type: "content",
  schema: z.object({
    title: z.string().min(1),
    slug: z.string().min(1),
    audience: z.string().optional(),
    source_file: z.string().min(1),
  }),
});

export const collections = { blog, submissions };
