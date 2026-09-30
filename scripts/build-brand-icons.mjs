import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const nextRequire = createRequire(require.resolve("next/package.json"));
const sharp = nextRequire("sharp");
const logo = await readFile(
  new URL("../public/brand/logo.png", import.meta.url)
);
const sizes = [16, 32, 48];
const frames = await Promise.all(
  sizes.map((size) =>
    sharp(logo)
      .resize(size, size)
      .flatten({ background: "#ffffff" })
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
await sharp(logo)
  .resize(64, 64)
  .flatten({ background: "#ffffff" })
  .png()
  .toFile(new URL("../app/icon.png", import.meta.url).pathname);
await sharp(logo)
  .resize(180, 180)
  .flatten({ background: "#ffffff" })
  .png()
  .toFile(new URL("../app/apple-icon.png", import.meta.url).pathname);
