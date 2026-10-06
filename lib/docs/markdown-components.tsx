import { asMarkdown, md } from "fumadocs-core/server";
import type { ReactNode } from "react";

import { readSource } from "@/lib/docs/read-source";

/**
 * Markdown forms of the MDX components used in `content/docs`, for
 * `page.data.getText("processed", { components })`.
 *
 * A component opts in by calling `asMarkdown()`; one that does not is
 * serialized as JSX syntax instead, which is what these replace. Returning a
 * string from a component is intentional: `renderToMarkdown` walks strings.
 */

type PropRow = [
  name: string,
  type: string,
  defaultValue: string | null,
  description: string | null,
];

const EMPTY_CELL = "—";

/** Escape the pipes and newlines that would break out of a table cell. */
const cell = (value: string | null): string =>
  value === null || value.length === 0
    ? EMPTY_CELL
    : value.replaceAll("|", "\\|").replaceAll("\n", " ");

const code = (value: string): string => `\`${value}\``;

const fence = (lang: string, body: string, title?: string): string => {
  const meta = title === undefined ? "" : ` title="${title}"`;
  return `\n\`\`\`${lang}${meta}\n${body}\n\`\`\`\n\n`;
};

const ComponentPreview = async ({ name }: { name: string }) => {
  asMarkdown();
  const path = `components/examples/${name}.tsx`;
  return fence("tsx", await readSource(path), path);
};

const ComponentSource = async ({
  path,
  title,
}: {
  path: string;
  title?: string;
}) => {
  asMarkdown();
  const lang = path.endsWith(".css") ? "css" : "tsx";
  return fence(lang, await readSource(path), title ?? path);
};

/** The `command` prop is already the npm/npx form the remark plugin extracted. */
const InstallCommand = ({ command }: { command: string }) => {
  asMarkdown();
  return fence("bash", command);
};

const PropsTable = ({ rows }: { rows: PropRow[] }) => {
  asMarkdown();
  if (rows.length === 0) {
    return "\nNo props.\n\n";
  }
  const body = rows
    .map(([name, type, defaultValue, description]) => {
      const columns = [
        cell(code(name)),
        cell(code(type)),
        cell(defaultValue === null ? null : code(defaultValue)),
        cell(description),
      ];
      return `| ${columns.join(" | ")} |`;
    })
    .join("\n");
  const header =
    "| Prop | Type | Default | Description |\n| --- | --- | --- | --- |";
  return `\n${header}\n${body}\n\n`;
};

const Callout = ({
  children,
  title,
}: {
  children?: ReactNode;
  title?: ReactNode;
}) => {
  asMarkdown();
  const quote = md.linePrefix("> ");
  return title === undefined
    ? quote`${children}`
    : quote`**${title}**\n\n${children}`;
};

/** Keeps the children, drops the wrapper: the markdown needs no tabs or steps. */
const Passthrough = ({ children }: { children?: ReactNode }) => {
  asMarkdown();
  return md`${children}`;
};

const Tab = ({ children, value }: { children?: ReactNode; value?: string }) => {
  asMarkdown();
  return value === undefined
    ? md`${children}`
    : md`**${value}**\n\n${children}`;
};

export const markdownComponents = {
  Callout,
  ComponentPreview,
  ComponentSource,
  InstallCommand,
  PropsTable,
  Step: Passthrough,
  Steps: Passthrough,
  Tab,
  Tabs: Passthrough,
  TypeTable: Passthrough,
};
