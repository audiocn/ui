import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { docsPages } from "./routes";

test("denied system capture explains how to try again", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator.mediaDevices, "getDisplayMedia", {
      value: () =>
        Promise.reject(new DOMException("Denied", "NotAllowedError")),
    });
  });
  await page.goto("/docs/blocks/system-audio-settings");
  const capture = page.getByRole("switch", { name: "Capture system audio" });
  await capture.click();
  const alert = page
    .getByRole("alert")
    .filter({ hasText: "Capture was not started" });
  await expect(alert).toBeVisible();
  await expect(alert).toContainText("Turn capture on again");
  await expect(capture).not.toBeChecked();
  await expect(capture).toBeEnabled();
});

test("a share without audio explains the missing audio option", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator.mediaDevices, "getDisplayMedia", {
      value: () => Promise.resolve(new MediaStream()),
    });
  });
  await page.goto("/docs/blocks/system-audio-settings");
  await page.getByRole("switch", { name: "Capture system audio" }).click();
  await expect(
    page.getByRole("alert").filter({ hasText: "No audio was shared" })
  ).toBeVisible();
});

for (const shortcut of ["Control+k", "Meta+k"]) {
  test(`${shortcut} opens docs search and typing does not switch theme`, async ({
    page,
  }) => {
    await page.goto("/docs");
    await page.keyboard.press(shortcut);
    const search = page.getByRole("combobox", { exact: true, name: "Search" });
    await expect(search).toBeFocused();
    const mode = await page.locator("html").getAttribute("class");
    await search.press("d");
    await expect(search).toHaveValue("d");
    await expect(page.locator("html")).toHaveAttribute("class", mode ?? "");
    await search.fill("level meter");
    await expect(page.getByRole("option").first()).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(search).not.toBeVisible();
  });
}

const checkCodeTabs = async (page: Page, index = 0): Promise<void> => {
  const tabs = page.getByRole("tab", { exact: true, name: "Code" });
  if (index >= (await tabs.count())) {
    return;
  }
  await tabs.nth(index).click();
  await expect(tabs.nth(index)).toHaveAttribute("aria-selected", "true");
  const overflow = await page.evaluate(
    () =>
      document.documentElement.scrollWidth -
      document.documentElement.clientWidth
  );
  expect(overflow).toBeLessThanOrEqual(0);
  await checkCodeTabs(page, index + 1);
};

test.describe("alternate preview tabs", () => {
  test.use({ viewport: { height: 820, width: 320 } });
  for (const url of docsPages.filter(
    (route) =>
      route.startsWith("/docs/components/") || route.startsWith("/docs/blocks/")
  )) {
    test(`${url} code tabs fit and unmount safely`, async ({ page }) => {
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(error.message));
      await page.goto(url);
      await page.waitForLoadState("networkidle");
      await checkCodeTabs(page);
      expect(errors).toEqual([]);
      await expect(page.locator("h1").first()).toBeVisible();
    });
  }
});
