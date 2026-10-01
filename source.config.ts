import { pageSchema } from "fumadocs-core/source/schema";
import { defineConfig, defineDocs } from "fumadocs-mdx/config";

import { codeThemes } from "./lib/docs/code-themes";

export const docs = defineDocs({
  dir: "content/docs",
  docs: {
    postprocess: {
      includeProcessedMarkdown: true,
    },
    schema: pageSchema.extend({
      seoDescription: pageSchema.shape.description,
      seoTitle: pageSchema.shape.description,
    }),
  },
});

export default defineConfig({
  mdxOptions: {
    rehypeCodeOptions: { themes: codeThemes },
  },
});
