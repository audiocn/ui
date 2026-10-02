import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const nextRequire = createRequire(require.resolve("next/package.json"));
const sharp = nextRequire("sharp");
const svgDensity = 144;
const logo = await readFile(
  new URL("../public/brand/logo.svg", import.meta.url)
);
const icon = logo
  .toString()
  .replace('stroke="#000"', 'stroke="#fff"')
  .replace('stroke-width="8.6"', 'stroke-width="11"')
  .replace('stroke-width="8"', 'stroke-width="10.5"')
  .replace(
    /(?<root><svg\b[^>]*>)/u,
    '$<root>\n  <circle cx="64" cy="64" r="64" fill="#000" stroke="none" />\n  <g transform="translate(0 0.5)">'
  )
  .replace("</svg>", "  </g>\n</svg>");
await writeFile(new URL("../app/icon.svg", import.meta.url), icon);
const sizes = [16, 32, 48];
const frames = await Promise.all(
  sizes.map((size) =>
    sharp(Buffer.from(icon), { density: svgDensity })
      .resize(size, size)
      .ensureAlpha()
      .png()
      .toBuffer()
  )
);
const headerSize = 6;
const entrySize = 16;
const header = Buffer.alloc(headerSize + entrySize * frames.length);
header.writeUInt16LE(1, 2);
header.writeUInt16LE(frames.length, 4);
let offset = header.length;

for (const [index, frame] of frames.entries()) {
  const entry = headerSize + entrySize * index;
  header[entry] = sizes[index];
  header[entry + 1] = sizes[index];
  header.writeUInt16LE(1, entry + 4);
  header.writeUInt16LE(32, entry + 6);
  header.writeUInt32LE(frame.length, entry + 8);
  header.writeUInt32LE(offset, entry + 12);
  offset += frame.length;
}

await writeFile(
  new URL("../app/favicon.ico", import.meta.url),
  Buffer.concat([header, ...frames])
);
await sharp(Buffer.from(icon), { density: svgDensity })
  .resize(64, 64)
  .png()
  .toFile(new URL("../app/icon.png", import.meta.url).pathname);
await sharp(Buffer.from(icon), { density: svgDensity })
  .resize(180, 180)
  .flatten({ background: "#000000" })
  .png()
  .toFile(new URL("../app/apple-icon.png", import.meta.url).pathname);
