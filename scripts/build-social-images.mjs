/* oxlint-disable eslint/no-await-in-loop -- Capture sequentially so Next dev and the browser keep a bounded memory footprint. */
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { once } from "node:events";
import { mkdir, readFile, unlink, writeFile } from "node:fs/promises";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";

import { chromium } from "@playwright/test";

import { socialCards } from "../lib/social-catalog.ts";

const root = process.cwd();
const outputDirectory = path.join(root, "public/og");
const manifestPath = path.join(root, "lib/social-images.json");
const port = Number(process.env.AUDIOCN_SOCIAL_PORT ?? 3107);
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

const capture = async (card) => {
  const context = await browser.newContext({
    colorScheme: "dark",
    deviceScaleFactor: 1,
    locale: "en-US",
    viewport: { height: 630, width: 1200 },
  });
  try {
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    // Pause before navigation: every mounted signal starts at the same frame.
    await page.clock.install({ time: new Date("2026-01-01T00:00:00Z") });
    await page.clock.pauseAt(new Date("2026-01-01T00:00:01Z"));
    await page.goto(`${baseUrl}/social-preview/${card.id}`, {
      waitUntil: "load",
    });
    await page.locator('[data-social-ready="true"]').waitFor();
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
      scale: "css",
    });
    if (bytes.readUInt32BE(16) !== 1200 || bytes.readUInt32BE(20) !== 630) {
      throw new Error(`${card.id} did not render at 1200 × 630.`);
    }
    if (bytes.length > 1_500_000) {
      throw new Error(`${card.id} exceeds the 1.5 MB social image budget.`);
    }
    const hash = createHash("sha256").update(bytes).digest("hex").slice(0, 12);
    return { bytes, card, filename: `${card.id}-${hash}.png` };
  } finally {
    await context.close();
  }
};

try {
  await waitForServer();
  browser = await chromium.launch({ channel: "chrome" });
  const captures = [];
  for (const card of socialCards) {
    captures.push(await capture(card));
    console.log(
      `social image: ${card.id} (${captures.length}/${socialCards.length})`
    );
  }
  const previous = JSON.parse(await readFile(manifestPath, "utf-8"));
  const manifest = {};
  await mkdir(outputDirectory, { recursive: true });
  for (const { bytes, card, filename } of captures) {
    await writeFile(path.join(outputDirectory, filename), bytes);
    manifest[card.pathname] = { alt: card.alt, url: `/og/${filename}` };
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
  console.log(`Saved ${captures.length} social images and their manifest.`);
} finally {
  await browser?.close();
  if (server.exitCode === null) {
    const stopped = once(server, "exit");
    server.kill("SIGTERM");
    await stopped;
  }
}
