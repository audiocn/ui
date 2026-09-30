import { defineConfig, defineDocs } from "fumadocs-mdx/config";

import { codeThemes } from "./lib/docs/code-themes";

export const docs = defineDocs({
  dir: "content/docs",
  docs: {
    postprocess: {
      includeProcessedMarkdown: true,
    },
  },
});

export default defineConfig({
  mdxOptions: {
    rehypeCodeOptions: { themes: codeThemes },
  },
});
