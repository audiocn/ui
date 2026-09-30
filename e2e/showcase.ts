import { expect } from "@playwright/test";
import type { Page } from "@playwright/test";

/** Every tile in the home page showcase. */
export const SHOWCASE_TILE_COUNT = 14;

const SCROLL_STEP = 300;
const OBSERVER_SETTLE_MS = 50;

const scrollFrom = async (page: Page, top: number): Promise<void> => {
  const height = await page.evaluate(
    () => document.documentElement.scrollHeight
  );
  if (top > height) {
    return;
  }
  await page.evaluate((y) => window.scrollTo(0, y), top);
  // A few frames for the intersection observers to see the new position.
  await page.waitForTimeout(OBSERVER_SETTLE_MS);
  await scrollFrom(page, top + SCROLL_STEP);
};

/**
 * Tiles load as they near the viewport, so step down the page until every
 * one has replaced its placeholder, then return to the top.
 */
export const mountShowcase = async (page: Page) => {
  const cards = page.locator('[data-slot="showcase-card"]');
  await expect(cards).toHaveCount(SHOWCASE_TILE_COUNT);
  await scrollFrom(page, 0);
  await expect(cards.locator('[data-slot="skeleton"]')).toHaveCount(0, {
    timeout: 15_000,
  });
  await page.evaluate(() => window.scrollTo(0, 0));
};
