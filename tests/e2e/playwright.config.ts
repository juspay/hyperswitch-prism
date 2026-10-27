import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./scenarios",
  testMatch: "**/*.spec.ts",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 180_000,
  expect: { timeout: 90_000 },
  reporter: [
    ["list"],
    ["html", { open: "never" }],
    ["junit", { outputFile: "test-results/results.xml" }],
  ],
  // API traces may contain credentials or payment-method data.
  use: { trace: "off", screenshot: "off", video: "off" },
});
