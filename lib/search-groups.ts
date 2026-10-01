import { flattenTree } from "fumadocs-core/page-tree";
import type { Root } from "fumadocs-core/page-tree";

export interface SearchGroupItem {
  title: string;
  url: string;
}

export interface SearchGroup {
  label: string;
  items: SearchGroupItem[];
}

/** Docs sections, by URL. Pages outside the others are Getting started. */
const SECTIONS = [
  { label: "Getting started", path: "/docs" },
  { label: "Components", path: "/docs/components" },
  { label: "Blocks", path: "/docs/blocks" },
  { label: "Hooks", path: "/docs/hooks" },
  { label: "Concepts", path: "/docs/concepts" },
] as const;

const [GETTING_STARTED] = SECTIONS;

const sectionOf = (url: string) =>
  SECTIONS.find(
    (section) =>
      section !== GETTING_STARTED &&
      (url === section.path || url.startsWith(`${section.path}/`))
  ) ?? GETTING_STARTED;

/**
 * The docs pages in each section, in sidebar order, for the search dialog
 * to show before anything is typed. Sections go by URL, since the sidebar
 * spreads the components' own sub-headings into its top level.
 */
export const searchGroups = (tree: Root): SearchGroup[] => {
  const groups = SECTIONS.map((section): SearchGroup => ({
    items: [],
    label: section.label,
  }));
  for (const page of flattenTree(tree.children)) {
    const section = sectionOf(page.url);
    const group = groups[SECTIONS.indexOf(section)];
    const isOverview = section !== GETTING_STARTED && page.url === section.path;
    if (isOverview) {
      group?.items.unshift({
        title: `All ${section.label.toLowerCase()}`,
        url: page.url,
      });
    } else {
      const title = typeof page.name === "string" ? page.name : "";
      group?.items.push({ title, url: page.url });
    }
  }
  return groups.filter((group) => group.items.length > 0);
};
