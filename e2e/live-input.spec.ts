import { expect, test } from "@playwright/test";

/** A microphone playing a steady tone at amplitude 0.5, about −6 dBFS. */
const fakeMicrophone = () => {
  Object.defineProperty(navigator.mediaDevices, "getUserMedia", {
    value: async () => {
      const context = new AudioContext();
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      gain.gain.value = 0.5;
      const destination = context.createMediaStreamDestination();
      oscillator.connect(gain).connect(destination);
      oscillator.start();
      await context.resume();
      return destination.stream;
    },
  });
};

test("the navbar mic switch drives the previews with the microphone", async ({
  page,
}) => {
  await page.addInitScript(fakeMicrophone);
  await page.goto("/docs/components/level-meter");
  const mic = page.getByRole("switch", { name: "Mic" });
  const readout = page.locator('[data-slot="db-readout"]').first();
  await mic.click();
  await expect(mic).toBeChecked();
  await expect(readout).toHaveText("−6.0 dB");

  // The microphone stays on across client-side navigation.
  await page.getByRole("link", { exact: true, name: "Spectrum" }).click();
  await expect(page).toHaveURL(/\/docs\/components\/spectrum$/u);
  await expect(page.getByRole("switch", { name: "Mic" })).toBeChecked();

  await page.getByRole("link", { exact: true, name: "Level Meter" }).click();
  await expect(readout).toHaveText("−6.0 dB");
  await page.getByRole("switch", { name: "Mic" }).click();
  await expect(readout).not.toHaveText("−6.0 dB");
});

test("a blocked microphone explains itself and turns the switch off", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator.mediaDevices, "getUserMedia", {
      value: () =>
        Promise.reject(new DOMException("Denied", "NotAllowedError")),
    });
  });
  await page.goto("/");
  const mic = page.locator("#nd-nav").getByRole("switch", { name: "Mic" });
  await mic.click();
  await expect(page.getByText("Microphone access is blocked.")).toBeVisible();
  await expect(mic).not.toBeChecked();
});
