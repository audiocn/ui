import { expect, test } from "@playwright/test";

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
