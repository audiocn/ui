import { readdirSync, statSync } from "node:fs";
import path from "node:path";

import { expect, test } from "@playwright/test";

import { siteConfig } from "../lib/site";

const DOCS_DIR = path.join(process.cwd(), "content/docs");

const collectPages = (dir: string, prefix = "/docs"): string[] =>
  readdirSync(dir).flatMap((entry) => {
    const full = path.join(dir, entry);
    if (statSync(full).isDirectory()) {
      return collectPages(full, `${prefix}/${entry}`);
    }
    if (!entry.endsWith(".mdx")) {
      return [];
    }
    const slug = entry.replace(/\.mdx$/u, "");
    return [slug === "index" ? prefix : `${prefix}/${slug}`];
  });

const pages = collectPages(DOCS_DIR);

test("the public sitemap includes every concrete page and robots permits indexing", async ({
  request,
}) => {
  const response = await request.get("/sitemap.xml");
  expect(response.status()).toBe(200);
  const xml = await response.text();
  const urls = [...xml.matchAll(/<loc>(?<url>[^<]+)<\/loc>/gu)].map(
    (match) => match.groups?.url
  );
  const expected = [
    siteConfig.url,
    ...pages.map((page) => new URL(page, siteConfig.url).href),
  ];
  expect(urls.toSorted()).toEqual(expected.toSorted());
  const robots = await request.get("/robots.txt");
  expect(robots.status()).toBe(200);
  expect(await robots.text()).toContain(
    `Sitemap: ${siteConfig.url}/sitemap.xml`
  );
  expect(await robots.text()).toContain("Allow: /");
});

test("docs pages keep their own canonical URL", async ({ page }) => {
  await page.goto("/docs/components/level-meter");
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute(
    "href",
    `${siteConfig.url}/docs/components/level-meter`
  );
});

test("missing docs pages offer a way back to the documentation", async ({
  page,
}) => {
  const response = await page.goto("/docs/does-not-exist");
  expect(response?.status()).toBe(404);
  await expect(
    page.getByRole("heading", { name: "Page not found" })
  ).toBeVisible();
  await page.getByRole("link", { name: "Browse documentation" }).click();
  await expect(
    page.getByRole("heading", { exact: true, name: "Introduction" })
  ).toBeVisible();
});

test.describe("every docs page", () => {
  for (const url of pages) {
    test(`renders ${url}`, async ({ page }) => {
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(error.message));
      page.on("console", (message) => {
        if (message.type() === "error") {
          errors.push(message.text());
        }
      });
      const response = await page.goto(url);
      expect(response?.status()).toBe(200);
      await expect(page.locator("h1").first()).toBeVisible();
      await page.waitForTimeout(500);
      expect(errors).toEqual([]);
    });
  }
});

test("the home page shows a live mixer", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1 })).toContainText("mixer");
  const meter = page.getByRole("meter", { name: "Microphone level" });
  await expect(meter).toBeVisible();
  await expect
    .poll(async () => Number(await meter.getAttribute("aria-valuenow")), {
      timeout: 10_000,
    })
    .toBeGreaterThan(-60);
});

test("level meters move with the demo signal", async ({ page }) => {
  await page.goto("/docs/components/level-meter");
  const meter = page.getByRole("meter", { name: "Program level" }).first();
  const first = await meter.getAttribute("aria-valuenow");
  await expect
    .poll(() => meter.getAttribute("aria-valuenow"), { timeout: 10_000 })
    .not.toBe(first);
  const level = await page
    .locator('[data-slot="level-meter-channel"]')
    .first()
    .evaluate((element) =>
      Number((element as HTMLElement).style.getPropertyValue("--meter-level"))
    );
  expect(level).toBeGreaterThan(0);
});

test("faders respond to the keyboard", async ({ page }) => {
  await page.goto("/docs/components/fader");
  const slider = page.getByRole("slider").first();
  await slider.focus();
  const before = await slider.getAttribute("aria-valuetext");
  await page.keyboard.press("Shift+ArrowUp");
  await expect(slider).not.toHaveAttribute("aria-valuetext", before ?? "");
});

test("the mixer block renders its strips", async ({ page }) => {
  await page.goto("/docs/blocks/system-audio-mixer");
  const strips = ["Microphone", "System audio", "Music", "Sounds", "Master"];
  await Promise.all(
    strips.map((name) =>
      expect(
        page.getByRole("group", { exact: true, name }).first()
      ).toBeVisible()
    )
  );
  await page.getByRole("tab", { name: "Console" }).first().click();
  await expect(page.locator('[data-slot="mixer"]').first()).toHaveAttribute(
    "data-orientation",
    "vertical"
  );
});

test("sound pads play synthesised demo audio", async ({ page }) => {
  await page.goto("/docs/components/sound-pad");
  const pad = page.getByRole("button", { name: /Airhorn/u }).first();
  await expect(pad).not.toHaveAttribute("data-loading", "", {
    timeout: 15_000,
  });
  await pad.click();
  await expect(pad).toHaveAttribute("data-playing", "");
});

test("soundboard removal is reversible without losing pad order", async ({
  page,
}) => {
  await page.goto("/docs/blocks/soundboard");
  const board = page.locator('[data-slot="soundboard"]');
  const pads = board.locator("[data-sound-pad]");
  await expect(pads.first()).toBeVisible();
  const before = await pads.allTextContents();
  await pads.first().click({ button: "right" });
  await page.getByRole("menuitem", { exact: true, name: "Remove" }).click();
  await expect(pads).toHaveCount(before.length - 1);
  await expect(board.getByRole("status")).toContainText("You can undo");
  await board.getByRole("button", { name: "Undo removal" }).click();
  await expect(pads).toHaveCount(before.length);
  expect(await pads.allTextContents()).toEqual(before);
  await expect(board.getByRole("status")).toContainText("Restored");
});

test("soundboard rejects non-audio files with inline feedback", async ({
  page,
}) => {
  await page.goto("/docs/blocks/soundboard");
  const board = page.locator('[data-slot="soundboard"]');
  const pads = board.locator("[data-sound-pad]");
  await expect(pads.first()).toBeVisible();
  const count = await pads.count();
  await board.getByLabel("Add audio files").setInputFiles({
    buffer: Buffer.from("not audio"),
    mimeType: "text/plain",
    name: "notes.txt",
  });
  await expect(board.getByRole("status")).toContainText("No new sounds added");
  await expect(pads).toHaveCount(count);
});

test("the theme picker switches themes", async ({ page }) => {
  await page.goto("/docs");
  await page.getByRole("combobox", { name: "Theme" }).first().click();
  await page.getByRole("option", { name: "Ocean" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "ocean");
});

interface PerfWindow {
  audiocnCommits: { count: number };
  audiocnFrames: { commitsAtStart: number; count: number; start: number };
}

test("a 16-strip console meters at full frame rate with no React commits", async ({
  page,
}) => {
  // A stand-in for React DevTools: React reports every commit to this hook.
  await page.addInitScript(() => {
    const commits = { count: 0 };
    Object.assign(window, {
      __REACT_DEVTOOLS_GLOBAL_HOOK__: {
        inject: () => 1,
        isDisabled: false,
        onCommitFiberRoot: () => {
          commits.count += 1;
        },
        supportsFiber: true,
      },
      audiocnCommits: commits,
    });
  });
  await page.goto("/docs/components/mixer");
  const meter = page.getByRole("meter", { name: "In 16 level" });
  await meter.scrollIntoViewIfNeeded();
  await expect(meter).toBeVisible();
  await expect
    .poll(async () => Number(await meter.getAttribute("aria-valuenow")), {
      timeout: 10_000,
    })
    .toBeGreaterThan(-60);
  // Let loading, scrolling and the table of contents settle.
  await page.waitForTimeout(1500);

  await page.evaluate(() => {
    const perf = window as unknown as PerfWindow;
    const frames = {
      commitsAtStart: perf.audiocnCommits.count,
      count: 0,
      start: performance.now(),
    };
    perf.audiocnFrames = frames;
    const tick = () => {
      frames.count += 1;
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
  await page.waitForTimeout(2000);
  const result = await page.evaluate(() => {
    const perf = window as unknown as PerfWindow;
    const frames = perf.audiocnFrames;
    return {
      commits: perf.audiocnCommits.count - frames.commitsAtStart,
      // Commits while the page loaded: proves React reports to the hook.
      commitsBeforeMeasuring: frames.commitsAtStart,
      fps: (frames.count * 1000) / (performance.now() - frames.start),
      meters: document.querySelectorAll('[data-slot="level-meter"]').length,
    };
  });

  expect(result.meters).toBeGreaterThanOrEqual(16);
  expect(result.commitsBeforeMeasuring).toBeGreaterThan(0);
  expect(result.commits).toBe(0);
  expect(result.fps).toBeGreaterThan(50);
});

test("the registry serves built items", async ({ request }) => {
  const response = await request.get("/r/level-meter.json");
  expect(response.ok()).toBe(true);
  const item = (await response.json()) as {
    name: string;
    files: { content: string }[];
    registryDependencies: string[];
  };
  expect(item.name).toBe("level-meter");
  expect(item.files[0]?.content).toContain("export const LevelMeter");
  expect(item.registryDependencies).toContain("@audiocn/core");
});

test("llms.txt lists the docs", async ({ request }) => {
  const response = await request.get("/llms.txt");
  expect(await response.text()).toContain("Level Meter");
});
