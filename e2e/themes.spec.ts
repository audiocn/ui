import { expect, test } from "@playwright/test";

const pages = [
  "/",
  "/docs/components/channel-toggle",
  "/docs/components/db-readout",
  "/docs/components/electric-bar-visualizer",
  "/docs/components/electric-waveform",
  "/docs/blocks/soundboard",
];

for (const width of [320, 1440]) {
  for (const theme of ["Stone", "Ocean", "Rose", "Mono"]) {
    for (const mode of ["light", "dark"] as const) {
      test.describe(`${theme} ${mode} at ${width}px`, () => {
        test.use({ colorScheme: mode, viewport: { height: 1000, width } });
        for (const url of pages) {
          test(`${url} fits its saved theme`, async ({ page }) => {
            await page.addInitScript((selected) => {
              window.localStorage.setItem("audiocn-theme", selected);
            }, theme.toLowerCase());
            await page.goto(url);
            const html = page.locator("html");
            await (theme === "Stone"
              ? expect(html).not.toHaveAttribute("data-theme")
              : expect(html).toHaveAttribute(
                  "data-theme",
                  theme.toLowerCase()
                ));
            await expect(page.locator("html")).toHaveClass(
              new RegExp(`\\b${mode}\\b`, "u")
            );
            await page.waitForLoadState("networkidle");
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
}
