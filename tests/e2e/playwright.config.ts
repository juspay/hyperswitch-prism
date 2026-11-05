import { defineConfig } from "@playwright/test";
import { loadConnectors } from "./framework/connector-registry.js";
import type { ScenarioFixtures } from "./framework/fixtures.js";

const connectors = await loadConnectors();

export default defineConfig<ScenarioFixtures>({
  testDir: "./scenarios",
  testMatch: "**/*.spec.ts",
  testIgnore: "**/.runtime/**",
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
  projects: connectors.map((connector) => ({
    name: `${connector.id}/${connector.merchantVariant}`,
    use: { connector },
  })),
  // API traces may contain credentials or payment-method data.
  use: { trace: "off", screenshot: "off", video: "off" },
});
