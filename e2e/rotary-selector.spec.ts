import { expect, test } from "@playwright/test";
import type { Locator, Page } from "@playwright/test";

const PAGE = "/docs/components/rotary-selector";

/** The named dial, scrolled into view, with its box and view box mapping. */
const selector = async (page: Page, name: string) => {
  await page.goto(PAGE);
  const dial = page.getByRole("slider", { exact: true, name });
  await dial.scrollIntoViewIfNeeded();
  const box = await dial.boundingBox();
  if (!box) {
    throw new Error(`The ${name} selector is not visible.`);
  }
  /** A point in client coordinates, from the dial's 100 × 100 view box. */
  const at = (x: number, y: number) =>
    [box.x + (box.width * x) / 100, box.y + (box.height * y) / 100] as const;
  /** A point at a radius, clockwise in degrees from 12 o'clock. */
  const around = (degrees: number, radius = 35) => {
    const radians = (degrees * Math.PI) / 180;
    return at(50 + radius * Math.sin(radians), 50 - radius * Math.cos(radians));
  };
  return { around, at, dial };
};

const drag = async (
  page: Page,
  [x, y]: readonly [number, number],
  dx: number,
  dy: number
) => {
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + dx, y + dy, { steps: 8 });
  await page.mouse.up();
};

/** Moves the pointer along an arc, ten degrees at a time. */
const circle = async (
  page: Page,
  around: (degrees: number) => readonly [number, number],
  from: number,
  to: number
) => {
  const direction = Math.sign(to - from);
  for (let angle = from + direction * 10; ; angle += direction * 10) {
    const reached = direction > 0 ? angle >= to : angle <= to;
    // The pointer moves one point at a time, in order.
    // eslint-disable-next-line no-await-in-loop
    await page.mouse.move(...around(reached ? to : angle));
    if (reached) {
      return;
    }
  }
};

const expectIndex = (dial: Locator, index: number) =>
  expect(dial).toHaveAttribute("aria-valuenow", String(index));

test("rotary selectors drag only from the circle around the cap", async ({
  page,
}) => {
  const { at, dial } = await selector(page, "Waveform");
  const expectIgnored = async (point: readonly [number, number]) => {
    await drag(page, point, 0, 48);
    await expect(dial).not.toBeFocused();
    await expectIndex(dial, 0);
  };

  await expectIgnored(at(2, 2));
  await expectIgnored(at(98, 2));
  await expectIgnored(at(2, 98));
  await expectIgnored(at(98, 98));
  // The sine's leader, where it crosses the dial's box at radius 57.
  await expectIgnored(at(9.7, 9.7));
  // Down is anticlockwise: the next index, since these positions run anticlockwise.
  await drag(page, at(50, 50), 0, 24);
  await expectIndex(dial, 1);
  await expect(dial).toBeFocused();
});

for (const direction of ["Vertical", "Horizontal"] as const) {
  test(`rotary selectors step every 24 px of ${direction.toLowerCase()} drag`, async ({
    page,
  }) => {
    const { at, dial } = await selector(page, direction);
    const [x, y] = at(50, 50);
    const along = (pixels: number) =>
      direction === "Vertical"
        ? ([x, y - pixels] as const)
        : ([x + pixels, y] as const);

    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(...along(20), { steps: 4 });
    await expectIndex(dial, 0);
    await page.mouse.move(...along(50), { steps: 4 });
    await expectIndex(dial, 2);
    // Past the last position it stays there.
    await page.mouse.move(...along(200), { steps: 8 });
    await expectIndex(dial, 4);
    await page.mouse.up();
    await expect(dial).toHaveAttribute("aria-valuetext", "Step 5");
  });
}

test("circled rotary selectors follow the pointer and hold an end in the gap", async ({
  page,
}) => {
  // Positions at -60°, -30°, 0°, 30° and 60°: the gap is the bottom 210°.
  const { around, dial } = await selector(page, "Circular");
  await page.mouse.move(...around(0));
  await page.mouse.down();
  await circle(page, around, 0, 30);
  await expectIndex(dial, 3);
  await circle(page, around, 30, 240);
  await expectIndex(dial, 4);
  // On around to the far side, it stays at the end, like a stop.
  await circle(page, around, 240, 300);
  await expectIndex(dial, 4);
  // Back the way it came, it follows again once past the end.
  await circle(page, around, 300, 60);
  await expectIndex(dial, 4);
  await circle(page, around, 60, -30);
  await expectIndex(dial, 1);
  await page.mouse.up();
});

test("a press on a position label selects it, and never drags", async ({
  page,
}) => {
  const { at, dial } = await selector(page, "Waveform");
  // The saw's glyph, left of the end of its leader.
  const glyph = at(-31, 65.8);
  await page.mouse.move(...glyph);
  await page.mouse.down();
  await page.mouse.move(glyph[0], glyph[1] + 48, { steps: 8 });
  await expect(dial).not.toHaveAttribute("data-dragging");
  await expectIndex(dial, 0);
  await page.mouse.move(...glyph, { steps: 8 });
  await page.mouse.up();
  await expect(dial).toHaveAttribute("aria-valuetext", "Saw");
});

test("rotary selectors select a position from its label", async ({ page }) => {
  const { dial } = await selector(page, "Instrument select");
  const preview = page
    .locator('[data-slot="component-preview"]')
    .filter({ has: dial });
  await preview
    .locator('[data-slot="rotary-selector-position-label"]', { hasText: "CB" })
    .click();
  await expect(dial).toHaveAttribute("aria-valuetext", "CB Cowbell");
  await expect(dial).toBeFocused();
  await dial.click({ modifiers: ["Alt"] });
  await expect(dial).toHaveAttribute("aria-valuetext", "BD Bass drum");
});

const scrollY = (page: Page) => page.evaluate(() => window.scrollY);

test("a focused rotary selector steps once per wheel notch and wraps", async ({
  page,
}) => {
  const { at, dial } = await selector(page, "Instrument select");
  await page.mouse.move(...at(50, 50));
  await dial.focus();
  const before = await scrollY(page);
  await page.mouse.wheel(0, -100);
  await expectIndex(dial, 2);
  await page.keyboard.press("Home");
  await page.mouse.wheel(0, 100);
  await expectIndex(dial, 11);
  expect(await scrollY(page)).toBe(before);
});

test("an unfocused rotary selector lets the wheel scroll the page", async ({
  page,
}) => {
  const { at, dial } = await selector(page, "Instrument select");
  await page.mouse.move(...at(50, 50));
  // The window sees the wheel last, after any listener on the dial.
  await page.evaluate(() => {
    window.addEventListener(
      "wheel",
      (event) => {
        document.body.dataset.wheelPrevented = String(event.defaultPrevented);
      },
      { once: true }
    );
  });
  const before = await scrollY(page);
  await page.mouse.wheel(0, 100);
  await expect(page.locator("body")).toHaveAttribute(
    "data-wheel-prevented",
    "false"
  );
  await expect.poll(() => scrollY(page)).toBeGreaterThan(before);
  await expectIndex(dial, 1);
});
