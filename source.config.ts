import { pageSchema } from "fumadocs-core/source/schema";
import { defineConfig, defineDocs } from "fumadocs-mdx/config";

import { codeThemes } from "./lib/docs/code-themes";
import { remarkInstallCommand } from "./lib/docs/remark-install-command";

export const docs = defineDocs({
  dir: "content/docs",
  docs: {
    postprocess: {
      // `function` keeps MDX elements as JSX so `getText("processed")` can
      // resolve them from a components map. See lib/docs/markdown-components.
      includeProcessedMarkdown: { headingIds: false, output: "function" },
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
    remarkNpmOptions: false,
    remarkPlugins: [remarkInstallCommand],
  },
});
