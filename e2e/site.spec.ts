import { readdirSync, statSync } from "node:fs";
import path from "node:path";

import { expect, test } from "@playwright/test";

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
    const slug = entry.replace(/\.mdx$/, "");
    return [slug === "index" ? prefix : `${prefix}/${slug}`];
  });

const pages = collectPages(DOCS_DIR);

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
    .poll(async () => meter.getAttribute("aria-valuenow"), { timeout: 10_000 })
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
  for (const name of [
    "Microphone",
    "System audio",
    "Music",
    "Sounds",
    "Master",
  ]) {
    await expect(
      page.getByRole("group", { exact: true, name }).first()
    ).toBeVisible();
  }
  await page.getByRole("tab", { name: "Console" }).first().click();
  await expect(page.locator('[data-slot="mixer"]').first()).toHaveAttribute(
    "data-orientation",
    "vertical"
  );
});

test("sound pads play synthesised demo audio", async ({ page }) => {
  await page.goto("/docs/components/sound-pad");
  const pad = page.getByRole("button", { name: /Airhorn/ }).first();
  await expect(pad).not.toHaveAttribute("data-loading", "", {
    timeout: 15_000,
  });
  await pad.click();
  await expect(pad).toHaveAttribute("data-playing", "");
});

test("the theme picker switches themes", async ({ page }) => {
  await page.goto("/docs");
  await page.getByRole("combobox", { name: "Theme" }).first().click();
  await page.getByRole("option", { name: "Ocean" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "ocean");
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
