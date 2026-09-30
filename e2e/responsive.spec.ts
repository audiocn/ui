import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { publicPages } from "./routes";

const PAGES = publicPages;

const PHONE = { height: 667, width: 375 };
const VIEWPORTS = [
  { height: 820, width: 320 },
  PHONE,
  { height: 1024, width: 768 },
  { height: 900, width: 1440 },
];

/** Elements that leave their preview box without a scroller or clip in between. */
const escapingPreviews = (page: Page) =>
  page.evaluate(() => {
    const escaping: string[] = [];
    for (const box of document.querySelectorAll<HTMLElement>(
      '[data-slot="component-preview"]'
    )) {
      const bounds = box.getBoundingClientRect();
      for (const element of box.querySelectorAll<HTMLElement>("*")) {
        const rect = element.getBoundingClientRect();
        // Skip unrendered nodes and one-pixel visually hidden inputs.
        const tooSmall = rect.width <= 1 || rect.height <= 1;
        const inside =
          rect.left >= bounds.left - 1 && rect.right <= bounds.right + 1;
        if (tooSmall || inside) {
          continue;
        }
        let clipped = false;
        for (
          let parent = element.parentElement;
          parent && parent !== box;
          parent = parent.parentElement
        ) {
          if (getComputedStyle(parent).overflowX !== "visible") {
            clipped = true;
            break;
          }
        }
        if (!clipped) {
          escaping.push(
            `${element.dataset.slot ?? element.tagName} ${Math.round(rect.left)}..${Math.round(rect.right)}`
          );
        }
      }
    }
    return escaping;
  });

for (const viewport of VIEWPORTS) {
  test.describe(`at ${viewport.width}px`, () => {
    test.use({ hasTouch: viewport.width < 1024, viewport });

    for (const url of PAGES) {
      test(`${url} fits the screen`, async ({ page }) => {
        await page.goto(url);
        await page.waitForLoadState("networkidle");
        const overflow = await page.evaluate(
          () =>
            document.documentElement.scrollWidth -
            document.documentElement.clientWidth
        );
        expect(overflow).toBeLessThanOrEqual(0);
        expect(await escapingPreviews(page)).toEqual([]);
      });
    }
  });
}

test.describe("on a phone", () => {
  test.use({ hasTouch: true, viewport: PHONE });

  test("channel strip rows stack their header above a full-width fader", async ({
    page,
  }) => {
    await page.goto("/docs/components/mixer");
    const strip = page
      .getByRole("group", { exact: true, name: "Microphone" })
      .first();
    await strip.scrollIntoViewIfNeeded();
    const header = await strip
      .locator('[data-slot="channel-strip-header"]')
      .boundingBox();
    const fader = await strip
      .locator('[data-slot="channel-strip-fader"]')
      .boundingBox();
    expect(header && fader).toBeTruthy();
    if (header && fader) {
      expect(header.y + header.height).toBeLessThanOrEqual(fader.y);
      expect(fader.width).toBeGreaterThan(150);
    }
  });

  test("console strips keep their width and the channels scroll", async ({
    page,
  }) => {
    await page.goto("/docs/components/mixer");
    const strip = page.getByRole("group", { exact: true, name: "In 1" });
    await strip.scrollIntoViewIfNeeded();
    const box = await strip.boundingBox();
    expect(box?.width ?? 0).toBeGreaterThanOrEqual(96);
    const scrolls = await page
      .locator('[data-slot="mixer-channels"]')
      .filter({ has: strip })
      .evaluate((element) => element.scrollWidth > element.clientWidth);
    expect(scrolls).toBe(true);
  });

  test("sound pad labels clear their hotkeys and the grid drops columns", async ({
    page,
  }) => {
    await page.goto("/docs/components/sound-pad");
    const grid = page.locator('[data-slot="sound-pad-grid"]').first();
    await grid.scrollIntoViewIfNeeded();
    const result = await grid.evaluate((element) => {
      const pads = [
        ...element.querySelectorAll<HTMLElement>("[data-sound-pad]"),
      ];
      const columns = new Set(
        pads.map((pad) => Math.round(pad.getBoundingClientRect().left))
      ).size;
      const overlapping = pads.filter((pad) => {
        const label = pad
          .querySelector('[data-slot="sound-pad-label"]')
          ?.getBoundingClientRect();
        const key = pad
          .querySelector('[data-slot="sound-pad-shortcut"]')
          ?.getBoundingClientRect();
        if (!(label && key)) {
          return false;
        }
        return (
          label.left < key.right &&
          label.right > key.left &&
          label.top < key.bottom &&
          label.bottom > key.top
        );
      }).length;
      return { columns, overlapping };
    });
    expect(result.columns).toBeLessThan(4);
    expect(result.overlapping).toBe(0);
  });

  test("soundboard pads keep their accent colours", async ({ page }) => {
    await page.goto("/docs/blocks/soundboard");
    const pad = page.locator("[data-sound-pad]").first();
    await expect(pad).toBeVisible({ timeout: 15_000 });
    const accent = await pad.evaluate((element) =>
      element.style.getPropertyValue("--pad-accent")
    );
    expect(accent).toContain("oklch");
  });

  test("sliders have a touch-sized grab area", async ({ page }) => {
    await page.goto("/docs/components/fader");
    const control = page.locator('[data-slot="fader-control"]').first();
    await control.scrollIntoViewIfNeeded();
    const height = await control.evaluate((element) => {
      const hitArea = getComputedStyle(element, "::before");
      return (
        element.getBoundingClientRect().height -
        Number(hitArea.top.replace("px", "")) -
        Number(hitArea.bottom.replace("px", ""))
      );
    });
    expect(height).toBeGreaterThanOrEqual(24);
  });

  test("dB scale labels never overlap", async ({ page }) => {
    await page.goto("/docs/components/db-scale");
    await page.waitForLoadState("networkidle");
    const overlaps = await page.evaluate(() => {
      let count = 0;
      for (const scale of document.querySelectorAll<HTMLElement>(
        '[data-slot="db-scale"]'
      )) {
        const horizontal = scale.dataset.orientation !== "vertical";
        const spans = [
          ...scale.querySelectorAll<HTMLElement>(
            '[data-slot="db-scale-label"]:not([data-hidden])'
          ),
        ]
          .map((label) => label.getBoundingClientRect())
          .map((rect) =>
            horizontal ? [rect.left, rect.right] : [rect.top, rect.bottom]
          )
          .toSorted((a, b) => (a[0] ?? 0) - (b[0] ?? 0));
        for (let index = 1; index < spans.length; index += 1) {
          if ((spans[index]?.[0] ?? 0) < (spans[index - 1]?.[1] ?? 0)) {
            count += 1;
          }
        }
      }
      return count;
    });
    expect(overlaps).toBe(0);
  });
});
