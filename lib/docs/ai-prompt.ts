import type { InstallPlan, RegistryItem } from "@/lib/docs/registry";
import {
  importPathFor,
  itemForImportPath,
  primaryFile,
  registryItemUrl,
} from "@/lib/docs/registry";
import { siteConfig } from "@/lib/site";

const LIBRARY_INTRO =
  "audiocn is a shadcn/ui registry of audio components for React: meters, visualizers, knobs, faders, players and mixers. The code is copied into the project and owned by it.";

const NOUNS: Record<string, string> = {
  "registry:block": "block",
  "registry:hook": "hook",
  "registry:lib": "library",
  "registry:ui": "component",
};

const absoluteUrl = (pathname: string): string =>
  `${siteConfig.url}${pathname}`;

/** The Markdown twin of a docs page, which agents can fetch. */
export const markdownUrlFor = (pathname: string): string => `${pathname}.md`;

const nounFor = (item: RegistryItem): string => NOUNS[item.type] ?? "component";

/** `lib/audio/*` for a multi-file item in one directory, else the single path. */
const filesSummary = (item: RegistryItem): string => {
  const paths = item.files.map((file) => file.path);
  if (paths.length <= 1) {
    return `\`${paths[0] ?? ""}\``;
  }
  const directories = new Set(
    paths.map((path) => path.slice(0, path.lastIndexOf("/")))
  );
  if (directories.size === 1) {
    const [directory] = [...directories];
    return `\`${directory}/*\`, ${paths.length} files`;
  }
  return `${paths.length} files`;
};

const FENCE = /^`{3,}/u;

/**
 * Walk the lines of a Markdown document, skipping over fenced code so that
 * `##` inside an example is never mistaken for a heading.
 */
const mapBlocks = (
  body: string,
  onBlock: (lines: string[], isFence: boolean) => string[]
): string => {
  const out: string[] = [];
  const lines = body.split("\n");
  let index = 0;

  while (index < lines.length) {
    const marker = FENCE.exec(lines[index])?.at(0);
    if (marker === undefined) {
      out.push(...onBlock([lines[index]], false));
      index += 1;
      continue;
    }
    const block = [lines[index]];
    index += 1;
    while (index < lines.length) {
      block.push(lines[index]);
      const closed = lines[index].trimStart().startsWith(marker);
      index += 1;
      if (closed) {
        break;
      }
    }
    out.push(...onBlock(block, true));
  }

  return out.join("\n");
};

const EXTRA_NEWLINES = /\n{3,}/gu;

const collapse = (body: string): string =>
  body.replaceAll(EXTRA_NEWLINES, "\n\n").trim();

const HEADING = /^##\s+/u;

const headingText = (line: string): string | undefined =>
  HEADING.test(line) ? line.replace(HEADING, "").trim() : undefined;

/**
 * Drop the page's own install command and the `## Installation` heading it sat
 * under: the prompt's Install section replaces them.
 */
const stripInstallCommand = (body: string, name: string): string => {
  const command = `npx shadcn@latest add ${siteConfig.registryNamespace}/${name}`;

  const withoutFence = mapBlocks(body, (lines, isFence) => {
    if (!isFence) {
      return lines;
    }
    const content = lines.slice(1, -1).join("\n").trim();
    return content === command ? [] : lines;
  });

  const lines = withoutFence.split("\n");
  const kept: string[] = [];
  for (let index = 0; index < lines.length; index += 1) {
    if (headingText(lines[index]) !== "Installation") {
      kept.push(lines[index]);
      continue;
    }
    // Keep the heading only when the section still has content of its own.
    const rest = lines.slice(index + 1);
    const next = rest.findIndex((line) => line.trim().length > 0);
    if (next !== -1 && !HEADING.test(rest[next])) {
      kept.push(lines[index]);
    }
  }

  return collapse(kept.join("\n"));
};

const IMPORT = /^import\s[\s\S]*?from\s+"@\/[^"]+";$/gmu;
const QUOTED = /"[^"]+"/u;

const importedPath = (statement: string): string =>
  QUOTED.exec(statement)?.at(0)?.slice(1, -1) ?? "";

/**
 * audiocn items an example imports that the CLI does not install with this one,
 * such as `use-demo-signal` in a demo.
 */
const extraItems = (body: string, install: InstallPlan): RegistryItem[] => {
  const installed = new Set([
    install.item.name,
    ...install.registryItems.map((entry) => entry.name),
  ]);
  const found = new Map<string, RegistryItem>();

  for (const match of body.matchAll(IMPORT)) {
    const item = itemForImportPath(importedPath(match[0]));
    if (item && !installed.has(item.name) && !found.has(item.name)) {
      found.set(item.name, item);
    }
  }

  return [...found.values()];
};

const installList = (install: InstallPlan): string[] => {
  const lines: string[] = [];

  if (install.registryItems.length > 0) {
    const entries = install.registryItems.map(
      (entry) =>
        `\`${siteConfig.registryNamespace}/${entry.name}\` (${filesSummary(entry)})`
    );
    lines.push(`- audiocn items: ${entries.join(", ")}`);
  }

  if (install.shadcnItems.length > 0) {
    const entries = install.shadcnItems.map((name) => `\`${name}\``);
    lines.push(`- shadcn/ui components: ${entries.join(", ")}`);
  }

  if (install.npmDependencies.length > 0) {
    const entries = install.npmDependencies.map((name) => `\`${name}\``);
    lines.push(`- npm packages: ${entries.join(", ")}`);
  }

  if (install.cssVariables.length > 0) {
    const base = install.cssVariables.filter(
      (name) => !name.endsWith("-foreground")
    );
    const hasPairs = base.length < install.cssVariables.length;
    const pairs = hasPairs ? ", each with a matching `-foreground` token" : "";
    lines.push(
      `- CSS variables added to the global stylesheet: ${base.map((name) => `\`${name}\``).join(", ")}${pairs}`
    );
  }

  return lines;
};

export interface AiPromptInput {
  /** The page title, such as `Bar Visualizer`. */
  title: string;
  description?: string;
  /** The docs pathname, such as `/docs/components/bar-visualizer`. */
  pathname: string;
  /** The page body as Markdown, from `getText("processed")`. */
  body: string;
  install: InstallPlan;
}

/** The document the "Copy prompt for AI" button copies, and `<page>.md` serves. */
export const buildAiPrompt = ({
  title,
  description,
  pathname,
  body,
  install,
}: AiPromptInput): string => {
  const { item } = install;
  const noun = nounFor(item);
  const command = `npx shadcn@latest add ${siteConfig.registryNamespace}/${item.name}`;
  const pageBody = stripInstallCommand(body, item.name);
  const dependsOn = installList(install);
  const extra = extraItems(pageBody, install);

  const sections: string[] = [
    `# Add ${title} from audiocn to this project`,
    `> Source: ${absoluteUrl(pathname)}\n> ${LIBRARY_INTRO}`,
  ];

  if (description) {
    sections.push(description);
  }

  sections.push("## Install");

  if (dependsOn.length > 0) {
    sections.push(
      `Use the shadcn CLI. Do not copy the ${noun} by hand: it depends on shared files that the CLI installs with it.`
    );
  } else {
    sections.push(`Use the shadcn CLI.`);
  }

  sections.push(
    [
      "1. The project needs React 19.2 or later, Tailwind CSS v4 and shadcn/ui. If there is no `components.json`, run `npx shadcn@latest init` first.",
      "2. Register the audiocn registry in `components.json`:",
      "",
      "   ```json",
      `   { "registries": { "${siteConfig.registryNamespace}": "${siteConfig.registryUrl}" } }`,
      "   ```",
      "",
      "3. Add it with the project's package manager (`pnpm dlx`, `yarn dlx`, `bunx --bun` or `npx`):",
      "",
      "   ```bash",
      `   ${command}`,
      "   ```",
    ].join("\n")
  );

  if (dependsOn.length > 0) {
    sections.push(
      `This writes \`${primaryFile(item)}\` and installs what it depends on:\n\n${dependsOn.join("\n")}`
    );
  } else {
    sections.push(`This writes \`${primaryFile(item)}\`.`);
  }

  sections.push(
    "Paths follow the aliases in `components.json`, so a `src/` project gets `src/components/ui/…`. The components are built on [Base UI](https://base-ui.com); the CLI installs `@base-ui/react` next to Radix if the project uses Radix."
  );

  if (extra.length > 0) {
    const entries = extra.map(
      (entry) => `\`${siteConfig.registryNamespace}/${entry.name}\``
    );
    sections.push(
      `The examples below also import ${entries.join(", ")}, which the command above does not install. Add them only to run an example as written; a real app feeds the ${noun} from its own audio instead.`
    );
  }

  sections.push(
    `If the CLI cannot run, fetch ${registryItemUrl(item.name)} and every item in its \`registryDependencies\`, then write each \`files[].content\` to its \`files[].path\`.`,
    pageBody
  );

  const after = [`- Import it from \`${importPathFor(item)}\`.`];
  if (pageBody.includes("FrameSource")) {
    after.push(
      `- Feed it audio with a \`FrameSource\`: \`useAudioAnalyser\` for a \`MediaStream\`, media element or \`AudioNode\`, or \`useMicrophone\` for the microphone. The API reference above says which frame each prop takes. See ${absoluteUrl(markdownUrlFor("/docs/concepts/feeding-data"))}`
    );
  }
  if (pageBody.includes("## Accessibility")) {
    after.push("- Keep the accessibility notes above.");
  }
  after.push("- Run the project's typecheck and lint.");

  sections.push(
    `## After installing\n\n${after.join("\n")}`,
    [
      "## More from audiocn",
      "",
      "- Every docs page is available as Markdown by appending `.md` to its URL.",
      `- Index of all components, hooks and blocks: ${absoluteUrl("/llms.txt")}`,
      `- Full documentation in one file: ${absoluteUrl("/llms-full.txt")}`,
    ].join("\n")
  );

  return `${collapse(sections.join("\n\n"))}\n`;
};

export interface PageMarkdownInput {
  title: string;
  description?: string;
  pathname: string;
  body: string;
}

/** The `.md` twin of a page with no registry item, such as a concept guide. */
export const buildPageMarkdown = ({
  title,
  description,
  pathname,
  body,
}: PageMarkdownInput): string => {
  const sections = [
    `# ${title}`,
    `> Source: ${absoluteUrl(pathname)}\n> ${LIBRARY_INTRO}`,
  ];
  if (description) {
    sections.push(description);
  }
  sections.push(body);
  return `${collapse(sections.join("\n\n"))}\n`;
};

export interface CompactPromptInput {
  title: string;
  pathname: string;
  name: string;
}

/**
 * A short prompt for a chat URL, which caps out around 6 KB: it points at the
 * Markdown page instead of carrying it.
 */
export const buildCompactPrompt = ({
  title,
  pathname,
  name,
}: CompactPromptInput): string =>
  [
    `Add the audiocn ${title} to my project.`,
    `Read ${absoluteUrl(markdownUrlFor(pathname))} and follow its Install and Usage sections exactly.`,
    `Registry item: ${registryItemUrl(name)}.`,
    `Full index: ${absoluteUrl("/llms.txt")}.`,
  ].join(" ");
