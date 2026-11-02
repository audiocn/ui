import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { PageActions } from "@/components/docs/page-actions";

const MARKDOWN = "# Add Bar Visualizer from audiocn to this project\n";
const SOURCE = "export const BarVisualizer = () => null;";
const REGISTRY_URL = "https://audiocn.dev/r/bar-visualizer.json";
const SOURCE_URL = "/r/bar-visualizer.json";
const MARKDOWN_URL = "/docs/components/bar-visualizer.md";

const props = {
  compactPrompt: "Add the audiocn Bar Visualizer to my project.",
  installCommand: "npx shadcn@latest add @audiocn/bar-visualizer",
  markdownUrl: MARKDOWN_URL,
  registryUrl: REGISTRY_URL,
  sourceUrl: SOURCE_URL,
  title: "Bar Visualizer",
};

const respond = (body: string) =>
  ({ ok: true, status: 200, text: () => Promise.resolve(body) }) as Response;

// Only same-origin URLs answer: the canonical one redirects to `www` and a
// redirect carries no CORS headers, so the browser refuses it.
const bodies: Record<string, string> = {
  [MARKDOWN_URL]: MARKDOWN,
  [SOURCE_URL]: JSON.stringify({ files: [{ content: SOURCE }] }),
};

const fetchedTimes = (url: string) =>
  vi.mocked(fetch).mock.calls.filter(([input]) => String(input) === url).length;

const chatUrl = (base: string, params: Record<string, string>) =>
  `${base}?${new URLSearchParams(params).toString()}`;

const href = (name: string | RegExp) =>
  screen.getByRole("menuitem", { name }).getAttribute("href");

beforeEach(() => {
  vi.stubGlobal(
    "fetch",
    vi.fn((input: RequestInfo | URL) => {
      const body = bodies[String(input)];
      return body === undefined
        ? Promise.reject(new Error(`Unexpected fetch: ${input}`))
        : Promise.resolve(respond(body));
    })
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

/** `userEvent` stubs the clipboard, so the hook's real write path is used. */
const clipboard = () => navigator.clipboard.readText();

const openMenu = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.click(
    screen.getByRole("button", { name: "More actions for AI agents" })
  );
  await screen.findByRole("menu");
};

describe("PageActions", () => {
  it("copies the page's Markdown, having fetched it only once", async () => {
    const user = userEvent.setup();
    render(<PageActions {...props} />);
    const button = screen.getByRole("button", { name: "Copy prompt for AI" });

    // Hovering prefetches, so the click has nothing left to wait for.
    await user.hover(button);
    await user.click(button);

    await expect.poll(clipboard).toBe(MARKDOWN);
    expect(fetchedTimes(MARKDOWN_URL)).toBe(1);
  });

  it("copies the install command for the reader's package manager", async () => {
    localStorage.setItem("packageManager", JSON.stringify("bun"));
    const user = userEvent.setup();
    render(<PageActions {...props} />);

    await openMenu(user);
    await user.click(
      screen.getByRole("menuitem", { name: "Copy install command" })
    );

    await expect
      .poll(clipboard)
      .toBe("bunx --bun shadcn@latest add @audiocn/bar-visualizer");
  });

  it("copies the component source from this deployment's registry item", async () => {
    const user = userEvent.setup();
    render(<PageActions {...props} />);

    await openMenu(user);
    await user.click(
      screen.getByRole("menuitem", { name: "Copy component source" })
    );

    await expect.poll(clipboard).toBe(SOURCE);
    expect(fetch).toHaveBeenCalledWith(SOURCE_URL);
    expect(fetch).not.toHaveBeenCalledWith(REGISTRY_URL);
  });

  it("hands the chat tools a prompt, and v0 the registry item", async () => {
    const user = userEvent.setup();
    render(<PageActions {...props} />);
    await openMenu(user);

    expect(href("View as Markdown")).toBe(MARKDOWN_URL);
    expect(href(/Open in ChatGPT/u)).toBe(
      chatUrl("https://chatgpt.com/", {
        hints: "search",
        prompt: props.compactPrompt,
      })
    );
    expect(href(/Open in Claude/u)).toBe(
      chatUrl("https://claude.ai/new", { q: props.compactPrompt })
    );
    expect(href(/Open in v0/u)).toBe(
      chatUrl("https://v0.dev/chat/api/open", { url: REGISTRY_URL })
    );
  });

  it("shows the error state when the prompt cannot be fetched", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve({ ok: false, status: 500 } as Response))
    );
    const user = userEvent.setup();
    // A URL of its own: a document that was fetched stays cached.
    render(<PageActions {...props} markdownUrl="/docs/components/gone.md" />);

    await user.click(
      screen.getByRole("button", { name: "Copy prompt for AI" })
    );

    await expect
      .poll(() => document.querySelector('[data-slot="error-icon"]'))
      .not.toBeNull();
  });
});
