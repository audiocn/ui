import { expect, test } from "@playwright/test";

const SAMPLE_MS = 1500;
const SAMPLE_EVERY_MS = 50;
/** Sub-pixel rounding is fine; a character of drift is not. */
const TOLERANCE_PX = 0.5;

interface SamplerWindow {
  widthSampler: { timer: number; widths: Set<number>[] };
}

test("level meters keep their width while the level moves", async ({
  page,
}) => {
  await page.goto("/docs/components/level-meter");
  const preview = page.locator('[data-slot="component-preview"]').first();
  await expect(preview.getByRole("meter").first()).toBeVisible();
  await preview.evaluate((element, every) => {
    const parts = [
      ...element.querySelectorAll<HTMLElement>(
        '[data-slot="level-meter-channels"], [data-slot="db-readout"]'
      ),
    ];
    const widths = parts.map(() => new Set<number>());
    const timer = window.setInterval(() => {
      for (const [index, part] of parts.entries()) {
        widths[index]?.add(Math.round(part.getBoundingClientRect().width));
      }
    }, every);
    Object.assign(window, { widthSampler: { timer, widths } });
  }, SAMPLE_EVERY_MS);
  await page.waitForTimeout(SAMPLE_MS);
  const drift = await page.evaluate(() => {
    const { timer, widths } = (window as unknown as SamplerWindow).widthSampler;
    window.clearInterval(timer);
    return Math.max(
      ...widths.map((set) => Math.max(...set) - Math.min(...set))
    );
  });
  expect(drift).toBeLessThanOrEqual(TOLERANCE_PX);
});

test("fader values keep their width across the whole range", async ({
  page,
}) => {
  await page.goto("/docs/components/fader#console-faders");
  const preview = page.locator('[data-slot="component-preview"]').nth(1);
  await preview.scrollIntoViewIfNeeded();
  const layout = () =>
    preview.evaluate((element) =>
      [
        ...element.querySelectorAll<HTMLElement>(
          '[data-slot="fader"], [data-slot="fader-value"]'
        ),
      ]
        .map((part) => {
          const rect = part.getBoundingClientRect();
          return `${Math.round(rect.left)}:${Math.round(rect.width)}`;
        })
        .join(" ")
    );
  const before = await layout();
  const slider = preview.getByRole("slider").first();
  await slider.focus();
  // Presses run in order, each followed by a layout snapshot.
  const pressAll = async (keys: string[]): Promise<string[]> => {
    const [key, ...rest] = keys;
    if (!key) {
      return [];
    }
    await slider.press(key);
    const snapshot = await layout();
    return [snapshot, ...(await pressAll(rest))];
  };
  const snapshots = await pressAll([
    "Home",
    "PageUp",
    "PageUp",
    "End",
    "PageDown",
  ]);
  expect(new Set(snapshots)).toEqual(new Set([before]));
});

test("knobs turn finer with Shift, reset with Alt+click and take typed values", async ({
  page,
}) => {
  await page.goto("/docs/components/knob");
  const preview = page.locator('[data-slot="component-preview"]').first();
  await preview.scrollIntoViewIfNeeded();
  const dial = preview.getByRole("slider").first();
  const box = await dial.boundingBox();
  if (!box) {
    throw new Error("The knob is not visible.");
  }
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;

  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.keyboard.down("Shift");
  await page.mouse.move(x, y - 40, { steps: 8 });
  // 40px of a 200px sensitivity over 48 dB is 9.6 dB, and a tenth with Shift.
  await expect(dial).toHaveAttribute("aria-valuenow", "1");
  await page.keyboard.up("Shift");
  await page.mouse.move(x, y - 80, { steps: 8 });
  await expect(dial).toHaveAttribute("aria-valuenow", "11");
  await page.mouse.up();

  await page.keyboard.down("Alt");
  await page.mouse.click(x, y);
  await page.keyboard.up("Alt");
  await expect(dial).toHaveAttribute("aria-valuenow", "0");

  await preview.locator('[data-slot="knob-value"]').first().dblclick();
  const input = preview.getByRole("textbox", { name: "Value" });
  await input.fill("-6");
  await input.press("Enter");
  await expect(dial).toHaveAttribute("aria-valuenow", "-6");
  await expect(dial).toBeFocused();

  await preview.getByText("Pan", { exact: true }).dblclick();
  await preview.getByRole("textbox", { name: "Value" }).fill("L40");
  await page.keyboard.press("Enter");
  await expect(preview.getByRole("slider").nth(1)).toHaveAttribute(
    "aria-valuetext",
    "L40"
  );
});
