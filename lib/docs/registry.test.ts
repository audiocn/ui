import { readdirSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

import type { RegistryItem } from "@/lib/docs/registry";
import {
  getRegistryItem,
  importPathFor,
  itemForImportPath,
  registryItemForPath,
  registryItemPath,
  registryItemUrl,
  resolveInstall,
} from "@/lib/docs/registry";

const item = (name: string): RegistryItem => {
  const found = getRegistryItem(name);
  if (!found) {
    throw new Error(`No registry item named "${name}"`);
  }
  return found;
};

const SECTIONS = ["components", "blocks", "hooks"] as const;

const pageSlugs = (section: string) =>
  readdirSync(path.join(process.cwd(), "content/docs", section))
    .filter((file) => file.endsWith(".mdx") && file !== "index.mdx")
    .map((file) => file.replace(/\.mdx$/u, ""));

describe("resolveInstall", () => {
  it("walks registry dependencies transitively", () => {
    const plan = resolveInstall(item("music-player"));
    const names = plan.registryItems.map((entry) => entry.name);

    expect(names).toContain("audio-player");
    // Only reachable through the components the block composes.
    expect(names).toContain("core");
    expect(plan.shadcnItems).toEqual(
      expect.arrayContaining(["card", "empty", "label", "switch", "toggle"])
    );
    expect(plan.npmDependencies).toContain("@phosphor-icons/react");
  });

  it("lists an item reached by several paths only once", () => {
    const names = resolveInstall(item("music-player")).registryItems.map(
      (entry) => entry.name
    );
    expect(names).toHaveLength(new Set(names).size);
  });

  it("reports the audio tokens only when the core comes with it", () => {
    expect(resolveInstall(item("bar-visualizer")).cssVariables).toContain(
      "--meter-ok"
    );
    expect(resolveInstall(item("use-visibility")).cssVariables).toEqual([]);
  });

  it("keeps a standalone item's plan empty", () => {
    const plan = resolveInstall(item("use-reduced-motion"));
    expect(plan.registryItems).toEqual([]);
    expect(plan.npmDependencies).toEqual([]);
    expect(plan.shadcnItems).toEqual([]);
  });
});

describe("importPathFor", () => {
  it("maps each kind of item to the alias consumers import", () => {
    expect(importPathFor(item("bar-visualizer"))).toBe(
      "@/components/ui/bar-visualizer"
    );
    expect(importPathFor(item("use-audio-analyser"))).toBe(
      "@/hooks/use-audio-analyser"
    );
    expect(importPathFor(item("music-player"))).toBe(
      "@/components/blocks/music-player/music-player"
    );
  });
});

describe("registryItemPath", () => {
  // The canonical URL redirects to `www`, which a browser fetch cannot follow
  // cross-origin, so the button needs the path on its own deployment.
  it("is the canonical URL's path, same origin as the reader", () => {
    expect(registryItemPath("bar-visualizer")).toBe("/r/bar-visualizer.json");
    expect(new URL(registryItemUrl("knob")).pathname).toBe(
      registryItemPath("knob")
    );
  });
});

describe("itemForImportPath", () => {
  it("finds the item an example imports", () => {
    expect(itemForImportPath("@/hooks/use-demo-signal")?.name).toBe(
      "use-demo-signal"
    );
    expect(itemForImportPath("@/components/ui/badge")).toBeUndefined();
  });
});

describe("registryItemForPath", () => {
  it("matches only the sections whose pages document an item", () => {
    expect(registryItemForPath("/docs/components/knob")?.name).toBe("knob");
    expect(registryItemForPath("/docs/hooks/use-level")?.name).toBe(
      "use-level"
    );
    expect(registryItemForPath("/docs/concepts/theming")).toBeUndefined();
    expect(registryItemForPath("/docs/installation")).toBeUndefined();
    expect(registryItemForPath("/docs/components")).toBeUndefined();
  });

  // The whole feature rests on the last path segment being the item name.
  it("has an item for every component, block and hook page", () => {
    for (const section of SECTIONS) {
      for (const slug of pageSlugs(section)) {
        const found = registryItemForPath(`/docs/${section}/${slug}`);
        expect(found?.name, `/docs/${section}/${slug}`).toBe(slug);
      }
    }
  });
});
