import { describe, expect, it } from "vitest";

import {
  buildAiPrompt,
  buildCompactPrompt,
  buildPageMarkdown,
  markdownUrlFor,
} from "@/lib/docs/ai-prompt";
import type { RegistryItem } from "@/lib/docs/registry";
import { getRegistryItem, resolveInstall } from "@/lib/docs/registry";

const item = (name: string): RegistryItem => {
  const found = getRegistryItem(name);
  if (!found) {
    throw new Error(`No registry item named "${name}"`);
  }
  return found;
};

/** Shaped like a processed page: an inlined example, then the usual sections. */
const BODY = `\`\`\`tsx title="components/examples/bar-visualizer-demo.tsx"
import { BarVisualizer } from "@/components/ui/bar-visualizer";
import { useDemoSignal } from "@/hooks/use-demo-signal";
\`\`\`

## Installation

\`\`\`bash
npx shadcn@latest add @audiocn/bar-visualizer
\`\`\`

## Usage

A \`FrameSource\` drives the bars.

## Examples

### Idle and loading

\`idle\` decides what bars do without a signal.

\`\`\`tsx title="components/examples/bar-visualizer-states.tsx"
const BarVisualizerStates = () => <BarVisualizer idle="pulse" />;
\`\`\`

### Inside a badge

\`\`\`tsx title="components/examples/bar-visualizer-mini.tsx"
const BarVisualizerMini = () => <Badge><BarVisualizer /></Badge>;
\`\`\`

## Accessibility

Pass \`aria-label\`.
`;

const prompt = buildAiPrompt({
  body: BODY,
  description: "A row of bars driven by frequency bands.",
  install: resolveInstall(item("bar-visualizer")),
  pathname: "/docs/components/bar-visualizer",
  title: "Bar Visualizer",
});

describe("buildAiPrompt", () => {
  it("opens with the page, its source and its description", () => {
    expect(prompt).toMatch(
      /^# Add Bar Visualizer from audiocn to this project\n/u
    );
    expect(prompt).toContain(
      "> Source: https://audiocn.dev/docs/components/bar-visualizer"
    );
    expect(prompt).toContain("A row of bars driven by frequency bands.");
  });

  it("tells the agent to register the registry and run the CLI", () => {
    expect(prompt).toContain(
      '{ "registries": { "@audiocn": "https://audiocn.dev/r/{name}.json" } }'
    );
    expect(prompt).toContain("npx shadcn@latest add @audiocn/bar-visualizer");
    expect(prompt).toContain("https://audiocn.dev/r/bar-visualizer.json");
  });

  it("replaces the page's own install section rather than repeating it", () => {
    const occurrences = prompt.split(
      "npx shadcn@latest add @audiocn/bar-visualizer"
    ).length;
    expect(occurrences - 1).toBe(1);
    expect(prompt).not.toContain("## Installation");
  });

  it("lists what the CLI writes alongside the component", () => {
    expect(prompt).toContain("This writes `components/ui/bar-visualizer.tsx`");
    expect(prompt).toContain("`@audiocn/core` (`lib/audio/*`, 15 files)");
    expect(prompt).toContain("`@audiocn/use-frame-source`");
    expect(prompt).toContain("`--meter-ok`");
    expect(prompt).toContain("each with a matching `-foreground` token");
  });

  it("keeps the page body and closes with what to do next", () => {
    expect(prompt).toContain("## Usage");
    expect(prompt).toContain("A `FrameSource` drives the bars.");
    expect(prompt).toContain("## Accessibility");
    expect(prompt).toContain(
      "- Import it from `@/components/ui/bar-visualizer`."
    );
    expect(prompt).toContain("- Keep the accessibility notes above.");
    expect(prompt).toContain("https://audiocn.dev/llms.txt");
  });

  it("replaces the examples with a link to each one", () => {
    expect(prompt).not.toContain("## Examples");
    expect(prompt).not.toContain("bar-visualizer-states.tsx");
    expect(prompt).not.toContain("BarVisualizerMini");

    expect(prompt).toContain(
      "- This page in the browser, with a live preview and the full source of each example: https://audiocn.dev/docs/components/bar-visualizer"
    );
    expect(prompt).toContain(
      "  - Idle and loading: https://audiocn.dev/docs/components/bar-visualizer#idle-and-loading"
    );
    expect(prompt).toContain(
      "  - Inside a badge: https://audiocn.dev/docs/components/bar-visualizer#inside-a-badge"
    );
  });

  // The anchors have to match the ids the docs page puts on its headings.
  it("slugs an example title the way the page does", () => {
    const linked = buildAiPrompt({
      body: "## Examples\n\n### Latching, driven by hand\n\n### LevelMeterScale, LevelMeterValue, LevelMeterClip\n",
      install: resolveInstall(item("level-meter")),
      pathname: "/docs/components/level-meter",
      title: "Level Meter",
    });
    expect(linked).toContain("#latching-driven-by-hand");
    expect(linked).toContain("#levelmeterscale-levelmetervalue-levelmeterclip");
  });

  it("still warns about an item only the linked examples import", () => {
    expect(prompt).toContain("The examples on this page also import");
    expect(prompt).toContain("`@audiocn/use-demo-signal`");
  });

  it("keeps the sections that follow the examples", () => {
    const resources = prompt.indexOf("## Resources");
    expect(prompt.indexOf("## Accessibility")).toBeLessThan(resources);
    expect(resources).toBeGreaterThan(prompt.indexOf("## After installing"));
  });

  it("links the page plainly when it has no examples", () => {
    const hook = buildAiPrompt({
      body: "## Usage\n\nCall it.\n",
      install: resolveInstall(item("use-audio-analyser")),
      pathname: "/docs/hooks/use-audio-analyser",
      title: "useAudioAnalyser",
    });
    expect(hook).toContain(
      "- This page in the browser: https://audiocn.dev/docs/hooks/use-audio-analyser"
    );
    expect(hook).not.toContain("live preview");
  });

  it("points at feeding data only for something that takes frames", () => {
    expect(prompt).toContain(
      "https://audiocn.dev/docs/concepts/feeding-data.md"
    );
    const plain = buildAiPrompt({
      body: "## Usage\n\nNo frames here.\n",
      install: resolveInstall(item("use-reduced-motion")),
      pathname: "/docs/hooks/use-reduced-motion",
      title: "useReducedMotion",
    });
    expect(plain).not.toContain("Feed it audio");
    expect(plain).toContain("This writes `hooks/use-reduced-motion.ts`.");
    expect(plain).not.toContain("installs what it depends on");
  });

  it("names the right kind of thing for a hook and a block", () => {
    const hook = buildAiPrompt({
      body: "## Usage\n",
      install: resolveInstall(item("use-audio-analyser")),
      pathname: "/docs/hooks/use-audio-analyser",
      title: "useAudioAnalyser",
    });
    expect(hook).toContain("Do not copy the hook by hand");
    expect(hook).toContain("- Import it from `@/hooks/use-audio-analyser`.");

    const block = buildAiPrompt({
      body: "## Usage\n",
      install: resolveInstall(item("music-player")),
      pathname: "/docs/blocks/music-player",
      title: "Music Player",
    });
    expect(block).toContain("Do not copy the block by hand");
    expect(block).toContain("shadcn/ui components:");
  });

  it("leaves no MDX components behind", () => {
    expect(prompt).not.toMatch(/<(?:ComponentPreview|PropsTable|Callout)\b/u);
  });

  it("ends with exactly one newline", () => {
    expect(prompt).toMatch(/[^\n]\n$/u);
  });
});

describe("buildPageMarkdown", () => {
  it("serves a page with no registry item as its own body", () => {
    const markdown = buildPageMarkdown({
      body: "## dBFS\n\nSome text.\n",
      description: "The units audiocn uses.",
      pathname: "/docs/concepts/decibels",
      title: "Decibels and levels",
    });
    expect(markdown).toMatch(/^# Decibels and levels\n/u);
    expect(markdown).toContain("The units audiocn uses.");
    expect(markdown).toContain("## dBFS");
    expect(markdown).not.toContain("## Install");
  });
});

describe("buildCompactPrompt", () => {
  it("points at the Markdown page instead of carrying it", () => {
    const compact = buildCompactPrompt({
      name: "bar-visualizer",
      pathname: "/docs/components/bar-visualizer",
      title: "Bar Visualizer",
    });
    expect(compact).toContain("Add the audiocn Bar Visualizer to my project.");
    expect(compact).toContain(
      "https://audiocn.dev/docs/components/bar-visualizer.md"
    );
    expect(compact).toContain("https://audiocn.dev/r/bar-visualizer.json");
    // Chat URLs cap out around 6 KB.
    expect(compact.length).toBeLessThan(500);
  });
});

describe("markdownUrlFor", () => {
  it("appends .md to a docs pathname", () => {
    expect(markdownUrlFor("/docs/components/knob")).toBe(
      "/docs/components/knob.md"
    );
  });
});
