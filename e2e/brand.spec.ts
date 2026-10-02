import { expect, test } from "@playwright/test";

for (const width of [320, 1440]) {
  for (const colorScheme of ["light", "dark"] as const) {
    test.describe(`brand ${colorScheme} at ${width}px`, () => {
      test.use({ colorScheme, viewport: { height: 900, width } });
      for (const url of ["/", "/docs"]) {
        test(`the approved knob brands ${url}`, async ({ page }) => {
          await page.goto(url);
          const logo = page
            .locator('[data-slot="brand-logo"]')
            .filter({ visible: true });
          await expect(logo).toBeVisible();
          await expect(logo).toHaveAttribute("alt", "");
          await expect(
            page
              .getByRole("link", { exact: true, name: "audiocn" })
              .filter({ visible: true })
          ).toBeVisible();
          await expect
            .poll(() =>
              logo.evaluate(
                (element) =>
                  element instanceof HTMLImageElement &&
                  element.complete &&
                  element.naturalWidth > 0
              )
            )
            .toBe(true);
          await expect(page.locator("html")).toHaveClass(
            new RegExp(`\\b${colorScheme}\\b`, "u")
          );
          const filter = await logo.evaluate(
            (element) => getComputedStyle(element).filter
          );
          if (colorScheme === "dark") {
            expect(filter).toContain("invert(1)");
          } else {
            expect(filter).not.toContain("invert(1)");
          }
          expect(
            await page.evaluate(
              () =>
                document.documentElement.scrollWidth -
                document.documentElement.clientWidth
            )
          ).toBeLessThanOrEqual(0);
        });
      }
    });
  }
}

test("browser and Apple icons serve the approved logo assets", async ({
  page,
  request,
}) => {
  await page.goto("/");
  await expect(
    page.locator('link[rel="icon"][type="image/png"]')
  ).toHaveAttribute("href", /\/icon\.png/u);
  await expect(page.locator('link[rel="apple-touch-icon"]')).toHaveAttribute(
    "href",
    /\/apple-icon\.png/u
  );
  const icon = await request.get("/icon.png");
  expect(icon.status()).toBe(200);
  const iconBytes = await icon.body();
  expect([iconBytes.readUInt32BE(16), iconBytes.readUInt32BE(20)]).toEqual([
    64, 64,
  ]);
  const apple = await request.get("/apple-icon.png");
  expect(apple.status()).toBe(200);
  const appleBytes = await apple.body();
  expect([appleBytes.readUInt32BE(16), appleBytes.readUInt32BE(20)]).toEqual([
    180, 180,
  ]);
  const favicon = await request.get("/favicon.ico");
  expect(favicon.status()).toBe(200);
  const bytes = await favicon.body();
  expect(bytes.readUInt16LE(0)).toBe(0);
  expect(bytes.readUInt16LE(2)).toBe(1);
  expect(bytes.readUInt16LE(4)).toBe(3);
  expect([bytes[6], bytes[22], bytes[38]]).toEqual([16, 32, 48]);
});
