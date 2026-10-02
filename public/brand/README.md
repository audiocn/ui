# audiocn logo

Browser icons use a round black background, a vertically centered white knob and heavier strokes for clarity at small sizes. The Apple icon uses the same mark on solid black, with the system supplying its rounded mask.

The approved mark is a thick, solid black open circular track with one diagonal indicator and no knob body. `logo.svg` is the transparent vector master used by the site and social cards. It recreates the selected raster concept using two rounded strokes, with no embedded bitmap. The site inverts the mark in dark mode for contrast.

Run `pnpm brand:build` to regenerate the SVG browser icon, 16/32/48px ICO fallback, 64px PNG browser icon, and 180px Apple touch icon from this master. The normal build runs this too.

Run `pnpm og:build` to refresh every social card and the existing README cards at 2x resolution after a branding change. `pnpm og:build --verify` also checks repeated captures are identical.

The vector geometry was recreated from the selected wide-opening knob concept with the heavier stroke. Its raster reference was previously prepared with the built-in image generation tool. Original reference preparation prompt:

> Use case: background-extraction. Edit target: the approved minimal open-track knob logo in the supplied image. Prepare this EXACT selected mark for application branding. Remove every white background area outside and inside the track, including the open center and gaps, making those areas genuinely transparent. Preserve exactly the TWO solid black elements: the heavy continuous circular open arc with its wide bottom opening and rounded ends, and the heavy straight indicator pointing diagonally up-right. Preserve their precise shapes, weight, proportions, angle, rounded caps, spacing, placement and circular geometry. Do not add a knob body or inner disk, connect the indicator to the track, close the opening, add ticks, text, shading or shadows. Keep solid black opaque shapes and smooth antialiased edges. Center the same design in a square transparent canvas with roughly 8 percent horizontal margin on each side; keep the design's aspect ratio and circular center alignment. This is faithful background removal and asset preparation, not a redesign.
