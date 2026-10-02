import { readFile } from "node:fs/promises";

import { expect, test } from "@playwright/test";

declare global {
  interface Window {
    brandCopiedText?: string;
  }
}

for (const width of [320, 1440]) {
  for (const pathname of ["/", "/docs"]) {
    test(`brand assets can be copied on ${pathname} at ${width}px`, async ({
      page,
    }) => {
      await page.setViewportSize({ height: 900, width });
      // Keep test writes inside the browser; do not overwrite the OS clipboard.
      await page.addInitScript(() => {
        Object.defineProperty(navigator, "clipboard", {
          value: {
            writeText: (text: string) => {
              window.brandCopiedText = text;
              return Promise.resolve();
            },
          },
        });
      });
      await page.goto(pathname);
      await page
        .getByRole("link", { exact: true, name: "audiocn" })
        .filter({ visible: true })
        .click({ button: "right" });
      const menu = page.getByRole("menu", { name: "Brand assets" });
      await expect(menu).toBeVisible();
      await expect(menu.getByRole("menuitem")).toHaveCount(5);
      await expect(menu.getByText("Brand Guidelines")).toHaveCount(0);
      const bounds = await menu.boundingBox();
      expect(bounds).not.toBeNull();
      expect(bounds?.x).toBeGreaterThanOrEqual(0);
      expect((bounds?.x ?? 0) + (bounds?.width ?? 0)).toBeLessThanOrEqual(
        width
      );
      await menu.getByRole("menuitem", { name: "Copy logo as SVG" }).click();
      await expect(
        page.getByText("Copied as SVG", { exact: true })
      ).toBeVisible();
      expect(await page.evaluate(() => window.brandCopiedText)).toBe(
        await readFile("public/brand/logo.svg", "utf-8")
      );
    });
  }
}

test("the wordmark copies portable outlines and clipboard failures show an error", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: (text: string) => {
          window.brandCopiedText = text;
          return Promise.resolve();
        },
      },
    });
  });
  await page.goto("/docs");
  const logo = page
    .getByRole("link", { exact: true, name: "audiocn" })
    .filter({ visible: true });
  await logo.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Copy wordmark as SVG" }).click();
  expect(await page.evaluate(() => window.brandCopiedText)).toBe(
    await readFile("public/brand/wordmark.svg", "utf-8")
  );
  await page.evaluate(() => {
    Object.defineProperty(navigator, "clipboard", {
      value: {
        writeText: () => Promise.reject(new Error("Permission denied")),
      },
    });
  });
  await logo.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Copy logo as SVG" }).click();
  await expect(
    page.getByText("Could not copy. Download the brand assets instead.", {
      exact: true,
    })
  ).toBeVisible();
});

for (const { label, filename, source } of [
  {
    filename: "audiocn-logo.svg",
    label: "Download logo SVG",
    source: "logo.svg",
  },
  {
    filename: "audiocn-logo.png",
    label: "Download logo PNG",
    source: "logo.png",
  },
  {
    filename: "audiocn-brand-assets.zip",
    label: "Download brand assets",
    source: "audiocn-brand-assets.zip",
  },
]) {
  test(`${label} downloads the current assets`, async ({ page }, testInfo) => {
    await page.goto("/");
    await page
      .getByRole("link", { exact: true, name: "audiocn" })
      .filter({ visible: true })
      .click({ button: "right" });
    const downloaded = page.waitForEvent("download");
    await page.getByRole("menuitem", { name: label }).click();
    const download = await downloaded;
    expect(download.suggestedFilename()).toBe(filename);
    const destination = testInfo.outputPath(filename);
    await download.saveAs(destination);
    expect(await readFile(destination)).toEqual(
      await readFile(`public/brand/${source}`)
    );
  });
}

test("the logo keeps home navigation and supports the keyboard context menu", async ({
  page,
}) => {
  await page.goto("/docs");
  const logo = page
    .getByRole("link", { exact: true, name: "audiocn" })
    .filter({ visible: true });
  await logo.focus();
  await page.keyboard.press("Shift+F10");
  await expect(page.getByRole("menu", { name: "Brand assets" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu", { name: "Brand assets" })).toBeHidden();
  await expect(logo).toBeFocused();
  await logo.click();
  await expect(page).toHaveURL(/\/$/u);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    "Audio UI, mixed and mastered."
  );
});

test.describe("mobile brand menu", () => {
  test.use({ hasTouch: true, viewport: { height: 900, width: 320 } });
  test("a long press opens the assets menu", async ({ page }) => {
    await page.goto("/docs");
    const logo = page
      .getByRole("link", { exact: true, name: "audiocn" })
      .filter({ visible: true });
    const bounds = await logo.boundingBox();
    expect(bounds).not.toBeNull();
    const session = await page.context().newCDPSession(page);
    await session.send("Input.dispatchTouchEvent", {
      touchPoints: [{ x: (bounds?.x ?? 0) + 8, y: (bounds?.y ?? 0) + 8 }],
      type: "touchStart",
    });
    await expect(
      page.getByRole("menu", { name: "Brand assets" })
    ).toBeVisible();
    await session.send("Input.dispatchTouchEvent", {
      touchPoints: [],
      type: "touchEnd",
    });
    await session.detach();
    await expect(page).toHaveURL(/\/docs$/u);
  });
});
