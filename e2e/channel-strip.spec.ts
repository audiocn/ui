import { expect, test } from "@playwright/test";

test("horizontal mixer tracks are visually centred in each strip", async ({
  page,
}) => {
  await page.setViewportSize({ height: 900, width: 1440 });
  await page.goto("/docs/components/mixer");
  const mixer = page.locator('[data-slot="mixer"]').first();
  await mixer.scrollIntoViewIfNeeded();
  await expect(mixer.locator('[data-slot="channel-strip"]')).toHaveCount(4);
  await expect(async () => {
    const offsets = await mixer
      .locator('[data-slot="channel-strip"]')
      .evaluateAll((strips) =>
        strips.map((strip) => {
          const bounds = strip.getBoundingClientRect();
          const tracks = [
            ...strip.querySelectorAll(
              '[data-slot="level-meter-track"], [data-slot="fader-track"]'
            ),
          ].map((track) => track.getBoundingClientRect());
          const top = Math.min(...tracks.map((track) => track.top));
          const bottom = Math.max(...tracks.map((track) => track.bottom));
          return (top + bottom) / 2 - (bounds.top + bounds.bottom) / 2;
        })
      );
    for (const offset of offsets) {
      expect(Math.abs(offset)).toBeLessThan(0.5);
    }
  }).toPass();
});

test("composed faders keep their Reset button clear of the track", async ({
  page,
}) => {
  await page.goto("/docs/components/mixer");
  const stripFaderClass = await page
    .locator('[data-slot="channel-strip-fader"]')
    .first()
    .getAttribute("class");
  if (!stripFaderClass) {
    throw new Error("Channel strip fader styles are missing");
  }

  await page.goto("/docs/components/fader");
  const fader = page.locator('[data-slot="fader"]').first();
  await fader.evaluate((element, className) => {
    const wrapper = document.createElement("div");
    wrapper.className = className;
    element.before(wrapper);
    wrapper.append(element);
  }, stripFaderClass);

  const reset = fader.locator('[data-slot="fader-reset"]');
  await expect(reset).toBeEnabled();
  await reset.click({ position: { x: 4, y: 22 }, timeout: 5000 });
  await expect(reset).toBeDisabled();
});

for (const width of [375, 1440]) {
  test.describe(`channel strip at ${width}px`, () => {
    test.use({ viewport: { height: 900, width } });

    for (const component of ["mixer", "channel-strip"]) {
      test(`${component} console tracks and readout align with the button centre`, async ({
        page,
      }) => {
        await page.goto(`/docs/components/${component}`);
        const strip = page
          .locator('[data-slot="channel-strip"][data-orientation="vertical"]')
          .first();
        await strip.scrollIntoViewIfNeeded();
        const offsets = await strip.evaluate((element) => {
          const controls = element.querySelector(
            '[data-slot="channel-strip-controls"]'
          );
          const value = element.querySelector(
            '[data-slot="channel-strip-value"]'
          );
          if (!(controls && value)) {
            throw new Error("Strip controls or readout is missing");
          }
          const buttons = controls.querySelectorAll("button");
          const mute = buttons[0]?.getBoundingClientRect();
          const solo = buttons[1]?.getBoundingClientRect();
          if (!(mute && solo)) {
            throw new Error("Mute or Solo button is missing");
          }
          const centre = (mute.right + solo.left) / 2;
          const tracks = [
            ...element.querySelectorAll(
              '[data-slot="level-meter-track"], [data-slot="fader-track"]'
            ),
          ].map((track) => track.getBoundingClientRect());
          const left = Math.min(...tracks.map((track) => track.left));
          const right = Math.max(...tracks.map((track) => track.right));
          const text = document.createRange();
          text.selectNodeContents(value);
          const readout = text.getBoundingClientRect();
          return {
            readout: (readout.left + readout.right) / 2 - centre,
            tracks: (left + right) / 2 - centre,
          };
        });
        expect.soft(Math.abs(offsets.tracks)).toBeLessThan(0.5);
        expect.soft(Math.abs(offsets.readout)).toBeLessThan(0.5);
      });
    }

    for (const orientation of ["horizontal", "vertical"]) {
      test(`${orientation} fader track reaches both meter edges`, async ({
        page,
      }) => {
        await page.goto("/docs/components/channel-strip");
        const strip = page
          .locator(
            `[data-slot="channel-strip"][data-orientation="${orientation}"]`
          )
          .first();
        await strip.scrollIntoViewIfNeeded();
        const meter = strip.locator('[data-slot="level-meter-track"]').first();
        const track = strip.locator('[data-slot="fader-track"]');
        const meterBounds = await meter.boundingBox();
        const trackBounds = await track.boundingBox();
        if (!(meterBounds && trackBounds)) {
          throw new Error("Meter or fader track is missing");
        }
        const axis = orientation === "horizontal" ? "x" : "y";
        const length = orientation === "horizontal" ? "width" : "height";
        expect(trackBounds[axis]).toBeCloseTo(meterBounds[axis], 1);
        expect(trackBounds[axis] + trackBounds[length]).toBeCloseTo(
          meterBounds[axis] + meterBounds[length],
          1
        );

        const slider = strip.getByRole("slider");
        const checkEndpoint = async (key: "Home" | "End") => {
          await expect(async () => {
            await slider.press(key);
            await expect(slider).toHaveAttribute(
              "aria-valuenow",
              key === "Home" ? "0" : "1"
            );
          }).toPass();
          const thumbBounds = await strip
            .locator('[data-slot="fader-thumb"]')
            .boundingBox();
          if (!thumbBounds) {
            throw new Error("Fader thumb is missing");
          }
          const atStart =
            orientation === "horizontal" ? key === "Home" : key === "End";
          expect(thumbBounds[axis] + thumbBounds[length] / 2).toBeCloseTo(
            meterBounds[axis] + (atStart ? 0 : meterBounds[length]),
            1
          );
        };
        await checkEndpoint("Home");
        await checkEndpoint("End");
      });
    }

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
