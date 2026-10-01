import { expect, test } from "@playwright/test";

for (const width of [375, 1440]) {
  test.describe(`channel strip at ${width}px`, () => {
    test.use({ viewport: { height: 900, width } });

    test("keeps equal space above and below its content", async ({ page }) => {
      await page.goto("/docs/components/channel-strip");
      const strip = page.locator('[data-slot="channel-strip"]').first();
      await strip.scrollIntoViewIfNeeded();
      const spacing = await strip.evaluate((element) => {
        const bounds = element.getBoundingClientRect();
        const layout = element.querySelector(
          '[data-slot="channel-strip-layout"]'
        );
        if (!layout) {
          throw new Error("Channel strip layout is missing");
        }
        const parts = [...layout.children].map((part) =>
          part.getBoundingClientRect()
        );
        return {
          bottom: bounds.bottom - Math.max(...parts.map((part) => part.bottom)),
          top: Math.min(...parts.map((part) => part.top)) - bounds.top,
        };
      });
      expect(spacing.top).toBeGreaterThan(0);
      expect(spacing.top).toEqual(spacing.bottom);
    });

    test("places notices below the controls across the full row", async ({
      page,
    }) => {
      await page.goto("/docs/components/channel-strip");
      const notice = page.locator('[data-slot="channel-strip-notice"]').first();
      await notice.scrollIntoViewIfNeeded();
      const spacing = await notice.evaluate((element) => {
        const bounds = element.getBoundingClientRect();
        const layout = element.parentElement;
        if (!layout) {
          throw new Error("Channel strip layout is missing");
        }
        const layoutBounds = layout.getBoundingClientRect();
        const parts = [...layout.children]
          .filter((part) => part !== element)
          .map((part) => part.getBoundingClientRect());
        return {
          bottom: layoutBounds.bottom - bounds.bottom,
          gap: bounds.top - Math.max(...parts.map((part) => part.bottom)),
          left: bounds.left - layoutBounds.left,
          right: layoutBounds.right - bounds.right,
        };
      });
      expect(spacing.bottom).toBeCloseTo(0);
      expect(spacing.gap).toBeGreaterThanOrEqual(6);
      expect(spacing.left).toBeCloseTo(0);
      expect(spacing.right).toBeCloseTo(0);
    });
  });
}
