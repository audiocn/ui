# Social images

These 1200 × 630 PNGs show audiocn's real React components in the dark Stone theme. They are committed assets: production serves them without a browser or image renderer.

Run `pnpm og:build` with Node 22.18+ and Google Chrome installed to regenerate them. The command starts an isolated local Next development server on port 3107, captures the cards, updates `lib/social-images.json`, removes superseded images owned by that manifest, and stops the server. Use `AUDIOCN_SOCIAL_PORT` to select another port. Stop a running `next dev` first because the capture uses the same `.next` directory.

Every public documentation page, the homepage and contributors page has a card. The catalog reads documentation titles and descriptions from Fumadocs, with specific compositions for all components and blocks and relevant component previews for hooks and guides. Edit `lib/social-catalog.ts` for mappings, featured copy and alt text. Edit `components/social/` for compositions.

The capture waits for hydration, generated demo audio, decoded waveforms, fonts and images, then advances a paused animation clock by exactly 1600 ms. Static signal frames and seeded demo signals supply the visuals; microphone and screen capture permissions are never requested. Each card gets a fresh browser context. Chrome uses software canvas rasterization and [deterministic rendering controls](https://github.com/GoogleChrome/chrome-launcher/blob/main/docs/chrome-flags-for-tools.md#rendering--gpu) for consistent antialiasing and compositing. Run `pnpm og:build --verify` to capture every card twice and require byte-for-byte identical results before saving any assets.

Filenames contain a hash of the PNG bytes so changed designs get new social cache URLs. Review the PNGs at full size and thumbnail size before committing the images and manifest together. Both `/social-preview/[id]` and `/api/social-cards` require `AUDIOCN_SOCIAL_CAPTURE=1` in development and always return 404 in production. They are excluded from the sitemap.
