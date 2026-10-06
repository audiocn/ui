# 015 — Copy prompt for AI

Status: IMPLEMENTED.

A "Copy prompt for AI" button on every component, block and hook page. One click puts a complete, self-contained prompt on the clipboard that tells an AI agent exactly how to add that item to a codebase with the shadcn CLI and how to use it. A menu next to it offers the parts (install command, component source) and "Open in ChatGPT / Claude / v0".

Reference: [React Bits' "Copy for AI"](https://www.reactbits.dev/text-animations/text-loop) (`CopyForAIMenu.jsx`, `useAIExportActions.js`, `aiExport.js`, `buildPrompt` in `TabsLayout.jsx`). Theirs is a menu with _Copy prompt_, _Copy configured code_, _Copy component source_, _Copy install command_, then _Open in ChatGPT / Claude / v0_. The prompt is title, variant, usage, a props table, the full source, and numbered integration steps, ending with a pointer to `llms.txt`.

## What we take from React Bits, and what we change

- Keep: a page-level action in the header, the menu structure, the compact "open in" prompt that points at URLs instead of carrying the whole document (chat URLs cap around 6 KB), the `llms.txt` pointer.
- Change: **no full component source in the prompt**. audiocn is a shadcn registry; every component depends on `@audiocn/core` (13 files in `lib/audio`), hooks and six CSS tokens. An agent that pastes a single file gets a broken install. The prompt tells the agent to run the CLI and describes what the CLI will install, with the registry JSON URL as the fallback. Source stays one menu item away ("Copy component source").
- Change: build the prompt on the server from the real sources of truth (`registry.json`, the MDX, the example files) rather than scraping the DOM as React Bits does. The prompt is then identical for every reader, available at a URL agents can fetch, and testable.

## Design

### One markdown document per page

`buildAiPrompt(page, item)` in `lib/docs/ai-prompt.ts` returns one markdown document for a docs page. It is:

- served at the page URL with `.md` appended (`/docs/components/bar-visualizer.md`), and
- what "Copy prompt for AI" copies.

The button fetches the `.md` URL on click (prefetched on hover/focus, cached in memory) instead of receiving the document as a prop: it keeps 5–20 KB per page out of the RSC payload, and gives "Open in ChatGPT/Claude" a URL to point at. This is the same pattern as fumadocs' own `MarkdownCopyButton`. Safari only allows clipboard writes inside the click, so the copy uses `ClipboardItem` with a `text/plain` promise when the document is not cached yet.

Pages without a registry item (concepts, installation, index) get the `.md` route with the plain page body and no button.

### Page → registry item

The last slug segment is the registry item name for every component, block and hook page (verified: all 46 pages match an item in `registry.json`). `lib/docs/registry.ts` imports `registry.json`, exposes `getRegistryItem(name)` and `resolveInstall(item)`, which walks `registryDependencies` transitively and splits them into audiocn items (`@audiocn/*`), shadcn items (`card`, `badge`…), npm `dependencies` and the `cssVars` that `core` adds. The prompt lists all four so the agent knows what the CLI is about to write.

### Rendering the MDX body as markdown

`includeProcessedMarkdown: true` is already on, but `<ComponentPreview name="…" />`, `<PropsTable rows={[…]} />`, `<InstallCommand>` and `<Callout>` come out as raw JSX in the processed text (this is also why `llms-full.txt` is weak today). fumadocs 16.15 has `includeProcessedMarkdown: { output: "function" }`: JSX elements are kept with their real props and resolved from a `components` map at `getText("processed", { components })` time through `renderToMarkdown` and the `md` tagged template from `fumadocs-core/server`.

`lib/docs/markdown-components.tsx` (server-only) provides the markdown forms:

| MDX component | Markdown form |
| --- | --- |
| `ComponentPreview name` | ` ```tsx title="components/examples/<name>.tsx" ` with the example source via `readSource` |
| `PropsTable rows` | GFM table: Prop, Type, Default, Description |
| `InstallCommand command` | ` ```bash ` fence with the npx form |
| `Callout title` | blockquote, `**title**` first line (`md.linePrefix("> ")`) |
| `Steps`, `Step`, `Tabs`, `Tab`, `TypeTable` | children only (`Tab` prefixed with its label as bold) |

The same map is passed from `llms-full.txt`, so that route improves for free.

### Where the button lives

`app/docs/[[...slug]]/page.tsx` renders `<PageActions />` between `DocsDescription` and `DocsBody` when `getRegistryItem(lastSlug)` exists. Props: `markdownUrl`, `pageUrl`, `registryUrl`, `installCommand`, `title`, `sourcePath`. Nothing changes in the MDX files.

### `components/docs/page-actions.tsx` (client)

A button group:

- **Copy prompt for AI** (`SparkleIcon`, label, `CopyStateIcon` swap on done). Reuses `useCopyToClipboard`, extended to accept `() => Promise<string>` and write through `ClipboardItem` when given a promise.
- A caret trigger opening a dropdown (`components/ui/dropdown-menu.tsx`, added with `npx shadcn@latest add dropdown-menu`, Base UI):
  - Copy prompt
  - Copy install command — uses the package manager the reader picked in `usePackageManager()`, via `convertNpmCommand`
  - Copy component source — fetches `/r/<name>.json` (already static in `public/r`) and copies `files[0].content`
  - View as Markdown — link to the `.md` URL
  - ───
  - Open in ChatGPT — `https://chatgpt.com/?prompt=<compact>&hints=search`
  - Open in Claude — `https://claude.ai/new?q=<compact>`
  - Open in v0 — `https://v0.dev/chat/api/open?url=<registry json url>` (the standard shadcn "Open in v0")

The compact prompt: "Add the audiocn `<Title>` to my project. Read `<page>.md` and follow its Install and Usage sections exactly. Registry item: `<r url>`. Full index: `https://audiocn.dev/llms.txt`."

Toasts via the existing `sonner` on success/failure, same as other copy buttons on the site.

## The prompt

Generated for `bar-visualizer`; headings from the MDX body are kept as `##`.

````markdown
# Add Bar Visualizer from audiocn to this project

> Source: https://audiocn.dev/docs/components/bar-visualizer audiocn is a shadcn/ui registry of audio components for React: meters, visualizers, knobs, faders, players and mixers. The code is copied into the project and owned by it.

A row of bars driven by frequency bands, with idle, loading and mirrored modes.

## Install

Use the shadcn CLI. Do not copy the component by hand: it depends on shared files (`lib/audio/*`, hooks and CSS tokens) that the CLI installs with it.

1. The project needs React 19.2 or later, Tailwind CSS v4 and shadcn/ui. If there is no `components.json`, run `npx shadcn@latest init` first.
2. Register the audiocn registry in `components.json`:

   ```json
   { "registries": { "@audiocn": "https://audiocn.dev/r/{name}.json" } }
   ```

3. Add the component with the project's package manager (`pnpm dlx`, `yarn dlx`, `bunx --bun` or `npx`):

   ```bash
   npx shadcn@latest add @audiocn/bar-visualizer
   ```

This writes `components/ui/bar-visualizer.tsx` and installs what it depends on:

- audiocn items: `@audiocn/core` (`lib/audio/*`), `@audiocn/use-audio-config`, `@audiocn/use-frame-source`, `@audiocn/use-reduced-motion`, `@audiocn/use-visibility`
- npm packages: `@base-ui/react`
- CSS variables added to the global stylesheet: `--meter-ok`, `--meter-warn`, `--meter-clip`, `--channel-mute`, `--channel-solo`, `--channel-monitor`

Paths follow the aliases in `components.json`, so a `src/` project gets `src/components/ui/…`. The components are built on Base UI; the CLI installs `@base-ui/react` next to Radix if the project uses Radix.

If the CLI cannot run, fetch https://audiocn.dev/r/bar-visualizer.json and every item in its `registryDependencies`, and write each `files[].content` to its `files[].path`.

## Usage

<the MDX body from here on: Usage, Examples with each example's source, Theming, Accessibility, API reference as a table>

## After installing

- Import from `@/components/ui/bar-visualizer` and render it where the user asked.
- Feed it audio through a `FrameSource`: `useAudioAnalyser` for a stream or media element, `useMicrophone` for the mic, `useDemoSignal` for a demo. See https://audiocn.dev/docs/concepts/feeding-data.md
- Keep the accessibility notes above (labels, `aria-hidden` when decorative).
- Run the project's typecheck and lint.

## More from audiocn

- Every docs page is available as Markdown by appending `.md` to its URL.
- Index of all components, hooks and blocks: https://audiocn.dev/llms.txt
- Full documentation in one file: https://audiocn.dev/llms-full.txt
````

Per type, the template varies only in the install paragraph and the import line: hooks (`hooks/use-x.ts`, import from `@/hooks/use-x`), blocks (`components/blocks/x/x.tsx`, and the shadcn items they pull in are listed under "shadcn components"). The "Feed it audio" bullet is emitted only for items whose props include a `FrameSource`; the hook and block pages get the relevant one-liner from a small per-type table in `ai-prompt.ts`, not free text per page.

## Delivery

1. `source.config.ts`: `includeProcessedMarkdown: { output: "function" }`.
2. `lib/docs/markdown-components.tsx`: markdown forms for `ComponentPreview`, `PropsTable`, `InstallCommand`, `Callout`, `Steps`/`Step`, `Tabs`/`Tab`, `TypeTable`. `app/llms-full.txt/route.ts` passes them to `getText`.
3. `lib/docs/registry.ts`: typed access to `registry.json`, `getRegistryItem`, `resolveInstall` (transitive deps, npm deps, shadcn items, css vars), `importPathFor(item)`.
4. `lib/docs/ai-prompt.ts`: `buildAiPrompt({ title, description, url, body, item, install })` and `buildCompactPrompt`. Pure functions over strings and the registry item.
5. `app/llms.mdx/[[...slug]]/route.ts` with `generateStaticParams`, plus a rewrite in `next.config.ts` from `/docs/:path*.md` to `/llms.mdx/:path*` (the fumadocs-documented layout; check `node_modules/next/dist/docs` for `rewrites` and `route.md` before writing). Returns `text/markdown; charset=utf-8`.
6. `components/ui/dropdown-menu.tsx` via `npx shadcn@latest add dropdown-menu`.
7. `hooks/use-copy-to-clipboard.ts`: accept `() => Promise<string>`, write with `ClipboardItem` when the text is a promise.
8. `components/docs/page-actions.tsx`: the button and menu described above. Phosphor icons: `SparkleIcon`, `CopyIcon`, `TerminalIcon`, `FileTsIcon`, `FileTextIcon`, `OpenAiLogoIcon`, `CaretDownIcon`; inline SVGs for Claude and v0 as in fumadocs' `page-actions.js`.
9. `app/docs/[[...slug]]/page.tsx`: render `PageActions` for pages with a registry item.
10. `app/llms.txt/route.ts`: one line noting the `.md` URLs.
11. `content/docs/installation.mdx`: a short "For AI agents" paragraph pointing at the button and `.md` URLs.

## Tests

- `lib/docs/ai-prompt.test.ts`: for a fixture item and body, the prompt contains the registry step, the install command, the transitive item list, the css vars only when `core` is in the closure, the import path per type, and no raw JSX.
- `lib/docs/markdown-components.test.tsx`: `renderToMarkdown` of each form: `PropsTable` rows become a GFM table with `—` for null defaults; `ComponentPreview` inlines the example file in a fenced block; `Callout` is a blockquote.
- `lib/docs/registry.test.ts`: `resolveInstall("music-player")` includes `@audiocn/core` through `audio-player`, lists `card`/`empty`/`label`/`switch`/`toggle` as shadcn items and `@phosphor-icons/react` as npm.
- `components/docs/page-actions.test.tsx`: click copies the fetched document (mock `fetch` and `navigator.clipboard`), the menu's install item respects the stored package manager, "Open in v0" links to the registry JSON.
- `e2e/ai-prompt.spec.ts`: on `/docs/components/bar-visualizer` the button is visible and `/docs/components/bar-visualizer.md` returns 200 markdown containing `npx shadcn@latest add @audiocn/bar-visualizer` and `components/examples/bar-visualizer-demo.tsx`; `/docs/concepts/decibels` has no button; the clipboard holds the document after a click (`context.grantPermissions(["clipboard-read", "clipboard-write"])`).
- `pnpm typecheck`, `pnpm check`, `pnpm test`, `pnpm test:e2e`; eyeball `llms-full.txt` for the bar-visualizer section.

## As built

Differences from the plan above, all discovered while writing it:

- `includeProcessedMarkdown` also needs `headingIds: false`. With them on, every heading comes out as `## Installation [#installation]`, which is noise in a prompt and defeats the install-section stripping below.
- The MDX body already carries the install command, so `buildAiPrompt` strips the fence whose content is `npx shadcn@latest add @audiocn/<name>`, and the `## Installation` heading it sat under when that leaves the section empty. The prompt's own Install section replaces them. Hook pages have the bare fence with no heading; both shapes are handled.
- `PropsTable` cells escape `|`, or a union type such as `FrameSource<VisualFrame> | null` breaks the table.
- The prompt names the audiocn items an example imports that the command does not install, such as `@audiocn/use-demo-signal` in a demo, and says to add them only to run the example as written. Without this an agent copies a demo that imports a file it does not have.
- Pages with no registry item get `buildPageMarkdown`: the body with a source header, no install steps.
- The `## Examples` section is lifted out of the prompt and each example becomes a link under a closing `## Resources`, which also absorbed `## More from audiocn`. On `bar-visualizer` the four inlined demo files were 3.6 KB, 41% of the document, and most of each one is layout for the docs page rather than something to copy; the API reference documents the same props. `## Usage` stays inline: it is 321 bytes and it is the import plus the minimal render. Anchors are slugged the way the page slugs its headings, and the section walker tracks which `##` it is under so the `###` headings in an API reference are not mistaken for examples.
- The dropdown menu trigger renders a `Button` through Base UI's `render` prop. Styling the trigger with `buttonVariants()` instead trips `shadcn/require-static-classes`.
- `components/ui/dropdown-menu.tsx` is stock shadcn output, so it joins `SHADCN_FILES` in `oxlint.config.ts` rather than being rewritten in house style.
- The TypeScript target is ES2017, which rules out named capture groups, so the regexes in `ai-prompt.ts` capture nothing and the matched text is sliced instead.
- "Copy component source" fetches `/r/<name>.json` on the deployment being read, not `siteConfig.registryUrl`. The canonical URL 308-redirects to `www`, and a redirect response carries no CORS headers, so the browser refuses the chain and the fetch throws from every origin but `www.audiocn.dev`. The absolute URL is still right for "Open in v0" and the prompt's fallback instructions, which are fetched server-side. Anything the browser fetches from this feature has to be same-origin.

## Later, not in this plan

- "Copy configured code" (React Bits' prop playground export). audiocn pages have no prop playground; nothing to configure yet.
- An MCP server for the registry. `.md` URLs and `llms.txt` cover agents with web access; an MCP would be the next step for the ones without.
- A `skills` item (an `AGENTS.md`-style guide installed through the registry) once the prompt text has settled.
