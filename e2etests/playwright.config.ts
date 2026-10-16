import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./otp",
  testMatch: "**/*.spec.ts",
  fullyParallel: false,
  workers: 1,
  retries: 0, // A retry can submit another financial transaction.
  timeout: 240_000,
  expect: { timeout: 90_000 },
  reporter: [
    ["list"],
    ["html", { open: "never" }],
    ["junit", { outputFile: "test-results/results.xml" }],
  ],
  // API traces contain credentials and card data. Keep reports limited to explicit safe evidence.
  use: { trace: "off", screenshot: "off", video: "off" },
});
