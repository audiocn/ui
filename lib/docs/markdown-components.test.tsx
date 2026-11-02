import { renderToMarkdown } from "fumadocs-core/server";
import type { FunctionComponent } from "react";
import { createElement } from "react";
import { describe, expect, it } from "vitest";

import { markdownComponents } from "@/lib/docs/markdown-components";

const {
  Callout,
  ComponentPreview,
  ComponentSource,
  InstallCommand,
  PropsTable,
  Steps,
  Tab,
} = markdownComponents;

/**
 * `renderToMarkdown` walks the component the way `getText("processed")` does.
 * These components return Markdown strings, which React's own types do not
 * describe, so the element is built with the props spelled out here.
 */
const render = <P extends object>(
  Component: (props: P) => unknown,
  props: P
): Promise<string> =>
  renderToMarkdown(createElement(Component as FunctionComponent<P>, props));

describe("ComponentPreview", () => {
  it("inlines the example file in a fenced block", async () => {
    const markdown = await render(ComponentPreview, {
      name: "bar-visualizer-demo",
    });
    expect(markdown).toContain(
      '```tsx title="components/examples/bar-visualizer-demo.tsx"'
    );
    expect(markdown).toContain("const BarVisualizerDemo = () =>");
    expect(markdown.trimEnd()).toMatch(/```$/u);
  });
});

describe("ComponentSource", () => {
  it("fences the file at the given path, and picks the language", async () => {
    const markdown = await render(ComponentSource, {
      path: "lib/audio/decibels.ts",
    });
    expect(markdown).toContain('```tsx title="lib/audio/decibels.ts"');

    const css = await render(ComponentSource, {
      path: "app/globals.css",
      title: "globals.css",
    });
    expect(css).toContain('```css title="globals.css"');
  });
});

describe("InstallCommand", () => {
  it("keeps the command in a shell fence", async () => {
    const markdown = await render(InstallCommand, {
      command: "npx shadcn@latest add @audiocn/knob",
    });
    expect(markdown).toBe("```bash\nnpx shadcn@latest add @audiocn/knob\n```");
  });
});

describe("PropsTable", () => {
  it("becomes a table, with an em dash for an absent default", async () => {
    const markdown = await render(PropsTable, {
      rows: [
        ["barCount", "number", "24", "Bands are resampled to fit."],
        ["levels", "ArrayLike<number>", null, null],
      ],
    });
    expect(markdown).toContain("| Prop | Type | Default | Description |");
    expect(markdown).toContain(
      "| `barCount` | `number` | `24` | Bands are resampled to fit. |"
    );
    expect(markdown).toContain("| `levels` | `ArrayLike<number>` | — | — |");
  });

  it("escapes the pipes in a union type so the table survives", async () => {
    const markdown = await render(PropsTable, {
      rows: [["align", '"start" | "end"', null, null]],
    });
    expect(markdown).toContain('| `align` | `"start" \\| "end"` | — | — |');
    expect(markdown.split("\n").filter(Boolean)).toHaveLength(3);
  });

  it("says so when there are no props", async () => {
    expect(await render(PropsTable, { rows: [] })).toBe("No props.");
  });
});

describe("Callout", () => {
  it("becomes a blockquote led by its title", async () => {
    const markdown = await render(Callout, {
      children: "A media element connects to Web Audio once.",
      title: "Media elements",
    });
    expect(markdown).toBe(
      "> **Media elements**\n>\n> A media element connects to Web Audio once."
    );
  });

  it("works without a title", async () => {
    expect(await render(Callout, { children: "Just a note." })).toBe(
      "> Just a note."
    );
  });
});

describe("wrappers", () => {
  it("keeps the children of a step and labels a tab", async () => {
    expect(await render(Steps, { children: "Do the thing." })).toBe(
      "Do the thing."
    );
    expect(await render(Tab, { children: "pnpm add it", value: "pnpm" })).toBe(
      "**pnpm**\n\npnpm add it"
    );
  });
});
