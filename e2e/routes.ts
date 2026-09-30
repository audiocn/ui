import { readdirSync } from "node:fs";
import path from "node:path";

const collectPages = (dir: string, prefix = "/docs"): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    if (entry.isDirectory()) {
      return collectPages(
        path.join(dir, entry.name),
        `${prefix}/${entry.name}`
      );
    }
    if (!entry.name.endsWith(".mdx")) {
      return [];
    }
    const slug = entry.name.replace(/\.mdx$/u, "");
    return [slug === "index" ? prefix : `${prefix}/${slug}`];
  });

export const docsPages = collectPages(
  path.join(process.cwd(), "content/docs")
).toSorted();
export const publicPages = ["/", ...docsPages];
