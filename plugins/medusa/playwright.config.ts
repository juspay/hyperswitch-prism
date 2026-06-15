import { defineConfig, devices } from "@playwright/test"

// E2E tests run against the app/ harness: the Express server (port 3000) serves
// the built React client AND the /store, /admin, /hooks APIs same-origin, so the
// whole storefront → server → connector → admin flow is exercised in one origin.
//
// Credentials come from creds.json (see creds.example.json / app/helpers/creds.ts);
// connectors absent from it are skipped per-spec.
const PORT = 3000
const BASE_URL = `http://localhost:${PORT}`

export default defineConfig({
  testDir: "./app/e2e",
  // Live sandbox calls are slow; give each test room and retry flaky network.
  timeout: 120_000,
  expect: { timeout: 30_000 },
  retries: process.env.CI ? 2 : 1,
  // Serialise: the harness uses a single hardcoded cart (cart_test_01) and an
  // in-memory session store, so parallel specs would clobber each other.
  workers: 1,
  fullyParallel: false,
  reporter: [["html", { open: "never" }], ["list"]],

  use: {
    baseURL: BASE_URL,
    trace: "on-first-retry",
    video: "retain-on-failure",
    actionTimeout: 30_000,
    navigationTimeout: 30_000,
  },

  projects: [
    {
      name: "chromium-ui",
      testDir: "./app/e2e/ui",
      use: {
        ...devices["Desktop Chrome"],
        // Reduce the automation fingerprint — Stripe's Payment Element gates
        // confirmPayment behind hCaptcha, which challenges flagged browsers.
        launchOptions: {
          args: ["--disable-blink-features=AutomationControlled"],
        },
      },
    },
    {
      name: "api",
      testDir: "./app/e2e/api",
      use: { baseURL: BASE_URL },
    },
  ],

  // Build the plugin, the React package, and the client, then start the
  // harness. server:dev loads creds.json automatically.
  webServer: {
    command:
      "npm run build && npm run build --workspace=medusa-unified-payment-react && npm run client:build && npm run server:dev",
    url: `${BASE_URL}/health`,
    reuseExistingServer: !process.env.CI,
    timeout: 240_000,
    stdout: "pipe",
    stderr: "pipe",
  },
})
