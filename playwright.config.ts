import { defineConfig, devices } from "@playwright/test";

const PORT = 3100;

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
    baseURL: `http://localhost:${PORT}`,
    trace: "retain-on-failure",
  },
  webServer: {
    command: `pnpm start --port ${PORT}`,
    reuseExistingServer: true,
    timeout: 120_000,
    url: `http://localhost:${PORT}`,
  },
});
