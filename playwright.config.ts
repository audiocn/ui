import { defineConfig, devices } from "@playwright/test";

const PORT = Number(process.env.AUDIOCN_PORT ?? 3100);

export default defineConfig({
  forbidOnly: Boolean(process.env.CI),
  fullyParallel: true,
  projects: [
    {
      name: "chrome",
      use: { ...devices["Desktop Chrome"], channel: "chrome" },
    },
  ],
  reporter: [["list"]],
  retries: 0,
  testDir: "./e2e",
  timeout: 60_000,
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    trace: "retain-on-failure",
  },
  webServer: {
    command: `pnpm start --hostname 127.0.0.1 --port ${PORT}`,
    reuseExistingServer: true,
    timeout: 120_000,
    url: `http://127.0.0.1:${PORT}`,
  },
});
