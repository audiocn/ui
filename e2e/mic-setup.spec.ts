import { expect, test } from "@playwright/test";

test("blocked microphone access has visible guidance", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator.mediaDevices, "getUserMedia", {
      value: () =>
        Promise.reject(new DOMException("Denied", "NotAllowedError")),
    });
  });
  await page.goto("/docs/blocks/mic-setup");
  await page.getByRole("button", { name: "Turn on microphone" }).click();
  await expect(
    page.getByRole("alert").filter({ hasText: "Microphone blocked" })
  ).toBeVisible();
});
