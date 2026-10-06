"use client";

import {
  CaretDownIcon,
  FileTsIcon,
  MarkdownLogoIcon,
  OpenAiLogoIcon,
  SparkleIcon,
  TerminalIcon,
} from "@phosphor-icons/react";
import { useRef } from "react";
import { toast } from "sonner";

import {
  convertNpmCommand,
  usePackageManager,
} from "@/components/code-block-command";
import { CopyStateIcon } from "@/components/copy-button";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useCopyToClipboard } from "@/hooks/use-copy-to-clipboard";

const documents = new Map<string, string>();

const fetchText = async (url: string): Promise<string> => {
  const cached = documents.get(url);
  if (cached !== undefined) {
    return cached;
  }
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch ${url}: ${response.status}`);
  }
  const text = await response.text();
  documents.set(url, text);
  return text;
};

const firstFileContent = (payload: unknown): string | undefined => {
  if (typeof payload !== "object" || payload === null) {
    return undefined;
  }
  const { files } = payload as { files?: unknown };
  if (!Array.isArray(files) || typeof files[0] !== "object" || !files[0]) {
    return undefined;
  }
  const { content } = files[0] as { content?: unknown };
  return typeof content === "string" ? content : undefined;
};

const fetchRegistrySource = async (url: string): Promise<string> => {
  const content = firstFileContent(JSON.parse(await fetchText(url)));
  if (content === undefined) {
    throw new Error(`No source in ${url}`);
  }
  return content;
};

// Decorative: each item is labelled by the text beside the icon.
const AnthropicIcon = () => (
  <svg aria-hidden="true" fill="currentColor" viewBox="0 0 24 24">
    <path d="M17.3041 3.541h-3.6718l6.696 16.918H24Zm-10.6082 0L0 20.459h3.7442l1.3693-3.5527h7.0052l1.3693 3.5528h3.7442L10.5363 3.5409Zm-.3712 10.2232 2.2914-5.9456 2.2914 5.9456Z" />
  </svg>
);

const VercelIcon = () => (
  <svg aria-hidden="true" fill="currentColor" viewBox="0 0 24 24">
    <path d="M12 1 24 22H0Z" />
  </svg>
);

const chatUrl = (base: string, params: Record<string, string>): string =>
  `${base}?${new URLSearchParams(params).toString()}`;

export interface PageActionsProps {
  /** The page title, such as `Bar Visualizer`. */
  title: string;
  /** The Markdown twin of this page, such as `/docs/components/knob.md`. */
  markdownUrl: string;
  /** The registry item JSON, such as `https://audiocn.dev/r/knob.json`. */
  registryUrl: string;
  /** The same item on this deployment, such as `/r/knob.json`. */
  sourceUrl: string;
  /** The npx form of the install command, switched per package manager. */
  installCommand: string;
  /** The short prompt handed to a chat URL, which cannot carry the page. */
  compactPrompt: string;
}

/**
 * "Copy prompt for AI" and the actions behind it, for a page that documents a
 * registry item. The prompt is fetched from `markdownUrl` rather than passed
 * in, to keep it out of the page payload.
 */
export const PageActions = ({
  title,
  markdownUrl,
  registryUrl,
  sourceUrl,
  installCommand,
  compactPrompt,
}: PageActionsProps) => {
  const [packageManager] = usePackageManager();
  const success = useRef("Copied");
  const { copy, state } = useCopyToClipboard({
    onCopyError: () => toast.error("Could not copy to clipboard"),
    onCopySuccess: () => toast.success(success.current),
  });

  const run = async (
    message: string,
    text: string | (() => Promise<string>)
  ) => {
    success.current = message;
    await copy(text);
  };

  const copyPrompt = () =>
    run(`Prompt for ${title} copied`, () => fetchText(markdownUrl));

  const prefetch = async () => {
    try {
      await fetchText(markdownUrl);
    } catch {
      // A prefetch that fails is retried, and reported, on click.
    }
  };

  const commands = convertNpmCommand(installCommand);
  const command =
    packageManager === "prompt" ? commands.npm : commands[packageManager];

  return (
    <div className="not-prose mb-6 flex items-center gap-1">
      <Button
        onClick={copyPrompt}
        onFocus={prefetch}
        onMouseEnter={prefetch}
        size="sm"
        variant="secondary"
      >
        <CopyStateIcon idleIcon={<SparkleIcon />} state={state} />
        Copy prompt for AI
      </Button>

      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <Button
              aria-label="More actions for AI agents"
              size="icon-sm"
              variant="secondary"
            />
          }
        >
          <CaretDownIcon />
        </DropdownMenuTrigger>

        <DropdownMenuContent align="start" className="w-64">
          <DropdownMenuItem onClick={copyPrompt}>
            <SparkleIcon />
            Copy prompt for AI
          </DropdownMenuItem>
          <DropdownMenuItem
            onClick={() => run("Install command copied", command)}
          >
            <TerminalIcon />
            Copy install command
          </DropdownMenuItem>
          <DropdownMenuItem
            onClick={() =>
              run("Component source copied", () =>
                fetchRegistrySource(sourceUrl)
              )
            }
          >
            <FileTsIcon />
            Copy component source
          </DropdownMenuItem>
          <DropdownMenuItem
            render={
              <a
                aria-label="View as Markdown"
                href={markdownUrl}
                rel="noreferrer"
                target="_blank"
              />
            }
          >
            <MarkdownLogoIcon />
            View as Markdown
          </DropdownMenuItem>

          <DropdownMenuSeparator />

          <DropdownMenuItem
            render={
              <a
                aria-label="Open in ChatGPT"
                href={chatUrl("https://chatgpt.com/", {
                  hints: "search",
                  prompt: compactPrompt,
                })}
                rel="noreferrer"
                target="_blank"
              />
            }
          >
            <OpenAiLogoIcon />
            Open in ChatGPT
          </DropdownMenuItem>
          <DropdownMenuItem
            render={
              <a
                aria-label="Open in Claude"
                href={chatUrl("https://claude.ai/new", { q: compactPrompt })}
                rel="noreferrer"
                target="_blank"
              />
            }
          >
            <AnthropicIcon />
            Open in Claude
          </DropdownMenuItem>
          <DropdownMenuItem
            render={
              <a
                aria-label="Open in v0"
                href={chatUrl("https://v0.dev/chat/api/open", {
                  url: registryUrl,
                })}
                rel="noreferrer"
                target="_blank"
              />
            }
          >
            <VercelIcon />
            Open in v0
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
};
