# audiocn logo

Browser and Apple icons use a white background so the bars stay visible on light and dark system surfaces.

The approved mark is five solid black rounded visualizer bars, with the tallest bar in the center and shorter pairs on each side. `logo.png` is the transparent master used by the site and social preview. The site inverts the mark in dark mode for contrast.

Run `pnpm brand:build` to regenerate the 16/32/48px favicon, 64px browser icon, and 180px Apple touch icon from this master. The normal build runs this too.

The master was prepared with the built-in image generation tool from the selected five-bar concept. Asset preparation prompt:

> Use case: background-extraction. Edit target: the approved five-bar black audio visualizer logo in the provided image. Prepare THIS EXACT selected mark as a production logo asset. Remove the white canvas outside and between the five black rounded bars, making that background transparent. Preserve all five bars faithfully: the same widths, relative heights, vertical alignment, spacing and rounded corners, with the tallest bar in the center and symmetric shorter pairs. Do not redesign, add bars, change proportions, add texture, shading, text or shadows. Flat solid black bars, smooth edges. Center the unchanged complete mark in a tightly framed square with about 10 percent transparent horizontal margin on both sides; preserve the mark's original aspect ratio and leave symmetric vertical margin. Transparent PNG output.
