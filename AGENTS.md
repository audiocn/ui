<!-- BEGIN:nextjs-agent-rules -->

# This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->

# audiocn

audiocn is a copy-and-paste React audio UI registry built the shadcn way. This repository contains the components, registry, and Next.js documentation site.

## Project map

- `components/ui/`: audio components and shared shadcn primitives.
- `components/blocks/`: complete audio features.
- `hooks/`: audio capture, playback, analysis, and state.
- `lib/audio/`: shared audio maths, types, and frame infrastructure.
- `components/examples/`: interactive documentation examples.
- `content/docs/`: Fumadocs MDX documentation.
- `registry.json`: installable items, files, and dependencies.
- `plans/`: design plans and component specifications.

Read the relevant docs and specifications for the task. Check older plans against the current implementation.

## Audio conventions

- Keep meters and visualizers independent of audio capture. They receive data; audio hooks and blocks connect the engine.
- Use frame sources and the shared frame loop for live rendering. Avoid routing animation-frame updates through React state.
- Meter values use dBFS; gain controls use dB. Preserve `-Infinity` for silence.
- Player volume uses 0..1.
- Clean up subscriptions, animation callbacks, audio nodes, and owned media resources.
- Preserve keyboard control, accessible names, theme tokens, and reduced-motion support.

For details, consult the relevant pages in `content/docs/concepts/`: `feeding-data.mdx`, `decibels.mdx`, `accessibility.mdx`, and `theming.mdx`.

## Registry changes

- Keep distributed code portable to consumer React projects. Avoid dependencies on the documentation app.
- Update `registry.json` when adding files or dependencies.
- Update documentation and examples when public APIs change.
- Generate `public/r/` with `pnpm registry:build`.
- Generate `components/docs/example-registry.tsx` with `pnpm examples:build`. Do not edit generated files manually.
- Preserve the consumer compatibility covered by `scripts/test-install.mjs`, including the Radix ES2022 fixture.

## Development and validation

Use pnpm and the existing package scripts.

- `pnpm dev`: run the documentation site.
- `pnpm fix`: apply Ultracite formatting and lint fixes.
- `pnpm check`: check formatting and lint.
- `pnpm typecheck`: generate MDX types and check TypeScript.
- `pnpm test`: run Vitest tests.
- `pnpm build`: build assets, examples, registry, and site.
- `pnpm test:e2e`: run Playwright against a production build; run `pnpm build` first.
- `pnpm test:install`: validate installation into fresh apps; run `pnpm registry:build` first.

Choose validation appropriate to the change. Add regression coverage for changed behavior. Use installation checks for registry dependency or portability changes, and browser checks for interaction or rendering changes.

Report what changed, what was verified, and any unresolved failures.

## Code standards

Follow the configured Ultracite rules and existing code patterns. Prefer explicit types, focused functions, semantic HTML, and existing UI primitives. Run `pnpm fix` before committing and review its diff for unrelated changes.
