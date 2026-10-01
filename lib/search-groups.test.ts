import type { Root } from "fumadocs-core/page-tree";
import { describe, expect, it } from "vitest";

import { searchGroups } from "@/lib/search-groups";

const page = (name: string, url: string) =>
  ({ name, type: "page", url }) as const;

// Shaped like the docs sidebar: the components folder is spread into the top
// level, so its own sub-headings sit beside the section headings.
const tree: Root = {
  children: [
    { name: "Getting started", type: "separator" },
    page("Introduction", "/docs"),
    page("Installation", "/docs/installation"),
    { name: "Concepts", type: "separator" },
    page("Theming", "/docs/concepts/theming"),
    { name: "Components", type: "separator" },
    page("Components", "/docs/components"),
    { name: "Controls", type: "separator" },
    page("Knob", "/docs/components/knob"),
    { name: "Mixer", type: "separator" },
    page("Mixer", "/docs/components/mixer"),
    { name: "Blocks", type: "separator" },
    {
      children: [page("Soundboard", "/docs/blocks/soundboard")],
      index: page("Blocks", "/docs/blocks"),
      name: "Blocks",
      type: "folder",
    },
  ],
  name: "Docs",
};

describe("searchGroups", () => {
  it("groups pages by section, overview first, in menu order", () => {
    expect(searchGroups(tree)).toEqual([
      {
        items: [
          { title: "Introduction", url: "/docs" },
          { title: "Installation", url: "/docs/installation" },
        ],
        label: "Getting started",
      },
      {
        items: [
          { title: "All components", url: "/docs/components" },
          { title: "Knob", url: "/docs/components/knob" },
          { title: "Mixer", url: "/docs/components/mixer" },
        ],
        label: "Components",
      },
      {
        items: [
          { title: "All blocks", url: "/docs/blocks" },
          { title: "Soundboard", url: "/docs/blocks/soundboard" },
        ],
        label: "Blocks",
      },
      {
        items: [{ title: "Theming", url: "/docs/concepts/theming" }],
        label: "Concepts",
      },
    ]);
  });
});
