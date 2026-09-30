<img src="public/brand/logo.png" alt="audiocn visualizer logo" width="96" height="96" />

# audiocn

Audio components for React, built the shadcn way: level meters, visualizers, faders, channel strips, a complete mixer, players and sound pads.

audiocn is a [shadcn registry](https://ui.shadcn.com/docs/registry). Components are copied into your project with the shadcn CLI, built on [Base UI](https://base-ui.com), styled with Tailwind CSS and themed with your shadcn tokens.

## Use it

Add the registry to `components.json`:

```json
{
  "registries": {
    "@audiocn": "https://audiocn.dev/r/{name}.json"
  }
}
```

Then add components:

```bash
npx shadcn@latest add @audiocn/level-meter
npx shadcn@latest add @audiocn/system-audio-mixer
```

Documentation: [audiocn.dev](https://audiocn.dev).

## Develop

This repository is the docs site, the registry source and the component source in one Next.js app.

| Path | What it is |
| --- | --- |
| `components/ui/` | Components (audiocn's and the shadcn ones the site uses) |
| `components/blocks/` | Blocks |
| `hooks/` | Hooks |
| `lib/audio/` | The audio core |
| `components/examples/` | Docs previews |
| `content/docs/` | Docs pages (Fumadocs MDX) |
| `registry.json` | The registry; `pnpm registry:build` writes `public/r/` |
| `plans/` | The plan and component specs |

```bash
pnpm install
pnpm dev              # docs site on http://localhost:3000
pnpm test             # unit and component tests (Vitest)
pnpm test:e2e         # browser tests (Playwright)
pnpm test:install     # installs every registry item into a fresh app and type-checks it
pnpm typecheck
pnpm check            # lint and format check (Ultracite)
pnpm build            # examples, registry and site
```

## Licence

[MIT](./license.md)
