/* oxlint-disable eslint/no-await-in-loop -- Capture sequentially so Next dev and the browser keep a bounded memory footprint. */
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { once } from "node:events";
import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  unlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";

import { chromium } from "@playwright/test";

const root = process.cwd();
const outputDirectory = path.join(root, "public/og");
const manifestPath = path.join(root, "lib/social-images.json");
const readmeDirectory = path.join(root, ".github/readme");
const readmeScale = 2;
const port = Number(process.env.AUDIOCN_SOCIAL_PORT ?? 3107);
const verify = process.argv.includes("--verify");
if (!Number.isInteger(port) || port < 1024 || port > 65_535) {
  throw new Error("AUDIOCN_SOCIAL_PORT must be a port between 1024 and 65535.");
}
const baseUrl = `http://127.0.0.1:${port}`;
const server = spawn(
  process.execPath,
  [
    "node_modules/next/dist/bin/next",
    "dev",
    "--hostname",
    "127.0.0.1",
    "--port",
    String(port),
  ],
  {
    cwd: root,
    env: { ...process.env, AUDIOCN_SOCIAL_CAPTURE: "1" },
    stdio: ["ignore", "pipe", "pipe"],
  }
);
let serverLog = "";
const collectLog = (chunk) => {
  serverLog = `${serverLog}${chunk.toString()}`.slice(-8000);
};
server.stdout.on("data", collectLog);
server.stderr.on("data", collectLog);
let browser;

const waitForServer = async () => {
  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    if (server.exitCode !== null) {
      throw new Error(`Social capture server exited.\n${serverLog}`);
    }
    try {
      const response = await fetch(`${baseUrl}/social-preview/home`, {
        signal: AbortSignal.timeout(1000),
      });
      if (response.ok) {
        return;
      }
    } catch {
      // The server may still be starting or compiling the capture page.
    }
    await delay(200);
  }
  throw new Error(`Social capture server did not become ready.\n${serverLog}`);
};

const capture = async (card, deviceScaleFactor = 1) => {
  const context = await browser.newContext({
    colorScheme: "dark",
    deviceScaleFactor,
    locale: "en-US",
    viewport: { height: 630, width: 1200 },
  });
  try {
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("console", (message) => {
      if (message.type() === "error") {
        errors.push(message.text());
      }
    });
    // Pause before navigation: every mounted signal starts at the same frame.
    await page.clock.install({ time: new Date("2026-01-01T00:00:00Z") });
    await page.clock.pauseAt(new Date("2026-01-01T00:00:01Z"));
    await page.goto(`${baseUrl}/social-preview/${card.id}`, {
      waitUntil: "load",
    });
    await page.locator('[data-social-ready="true"]').waitFor();
    await page
      .locator('[data-social-loaded="false"]')
      .waitFor({ state: "detached" });
    await page.locator("[data-loading]").waitFor({ state: "detached" });
    if (card.preview === "quick-popover") {
      await page
        .getByRole("button", { exact: true, name: "Audio settings" })
        .click({ force: true });
      await page.locator("[data-loading]").waitFor({ state: "detached" });
    }
    await page.evaluate(async () => {
      await document.fonts.ready;
      await Promise.all(Array.from(document.images, (image) => image.decode()));
    });
    // Force a native layout/paint before advancing the simulated RAF loop.
    // ResizeObserver can otherwise resize and clear a canvas after its last frame.
    await page
      .locator("[data-social-card]")
      .screenshot({ animations: "disabled" });
    await page.clock.runFor(1600);
    if (errors.length > 0) {
      throw new Error(`${card.id} failed to render: ${errors.join("; ")}`);
    }
    const bytes = await page.locator("[data-social-card]").screenshot({
      animations: "disabled",
      caret: "hide",
      scale: "device",
    });
    if (
      bytes.readUInt32BE(16) !== 1200 * deviceScaleFactor ||
      bytes.readUInt32BE(20) !== 630 * deviceScaleFactor
    ) {
      throw new Error(`${card.id} did not render at the requested scale.`);
    }
    if (bytes.length > 1_500_000 * deviceScaleFactor ** 2) {
      throw new Error(`${card.id} exceeds the image budget for its scale.`);
    }
    const hash = createHash("sha256").update(bytes).digest("hex").slice(0, 12);
    return { bytes, card, filename: `${card.id}-${hash}.png` };
  } finally {
    await context.close();
  }
};

try {
  await waitForServer();
  const catalogResponse = await fetch(`${baseUrl}/api/social-cards`);
  if (!catalogResponse.ok) {
    throw new Error("Could not read the social card catalog.");
  }
  const socialCards = await catalogResponse.json();
  if (!Array.isArray(socialCards) || socialCards.length === 0) {
    throw new Error("The social card catalog is empty or invalid.");
  }
  browser = await chromium.launch({
    // Software rasterization keeps canvas antialiasing identical across captures.
    args: [
      "--disable-accelerated-2d-canvas",
      "--disable-gpu",
      "--deterministic-mode",
      "--run-all-compositor-stages-before-draw",
      "--disable-threaded-animation",
      "--disable-threaded-scrolling",
    ],
    channel: "chrome",
  });
  const captures = [];
  for (const card of socialCards) {
    const result = await capture(card);
    if (verify) {
      const repeated = await capture(card);
      if (!result.bytes.equals(repeated.bytes)) {
        const failureDirectory = await mkdtemp(
          path.join(tmpdir(), "audiocn-og-")
        );
        await writeFile(
          path.join(failureDirectory, `${card.id}-first.png`),
          result.bytes
        );
        await writeFile(
          path.join(failureDirectory, `${card.id}-repeat.png`),
          repeated.bytes
        );
        throw new Error(
          `${card.id} changed between two identical captures. Compare the PNGs in ${failureDirectory}.`
        );
      }
    }
    captures.push(result);
    console.log(
      `social image: ${card.id} (${captures.length}/${socialCards.length})${verify ? " — verified identical" : ""}`
    );
  }
  const readmeFiles = new Set(await readdir(readmeDirectory));
  const readmeCaptures = [];
  for (const card of socialCards) {
    const filename = `${path.basename(card.pathname) || "home"}.png`;
    if (readmeFiles.has(filename)) {
      const result = await capture(card, readmeScale);
      if (verify) {
        const repeated = await capture(card, readmeScale);
        if (!result.bytes.equals(repeated.bytes)) {
          throw new Error(`${filename} changed between two README captures.`);
        }
      }
      readmeCaptures.push({ bytes: result.bytes, filename });
      console.log(`README image: ${filename}`);
    }
  }
  const previous = JSON.parse(await readFile(manifestPath, "utf-8"));
  const manifest = {};
  await mkdir(outputDirectory, { recursive: true });
  for (const { bytes, card, filename } of captures) {
    await writeFile(path.join(outputDirectory, filename), bytes);
    manifest[card.pathname] = { alt: card.alt, url: `/og/${filename}` };
  }
  for (const { bytes, filename } of readmeCaptures) {
    await writeFile(path.join(readmeDirectory, filename), bytes);
  }
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  // Only remove assets owned by the previous manifest, after all captures succeed.
  const currentUrls = new Set(
    Object.values(manifest).map((image) => image.url)
  );
  for (const image of Object.values(previous)) {
    if (
      /^\/og\/[a-z0-9-]+\.png$/u.test(image.url) &&
      !currentUrls.has(image.url)
    ) {
      await unlink(path.join(root, "public", image.url.slice(1))).catch(
        (error) => {
          if (error.code !== "ENOENT") {
            throw error;
          }
        }
      );
    }
  }
  console.log(
    `Saved ${captures.length} social images, their manifest and ${readmeCaptures.length} README images.`
  );
} finally {
  await browser?.close();
  if (server.exitCode === null) {
    const stopped = once(server, "exit");
    server.kill("SIGTERM");
    await stopped;
  }
}
