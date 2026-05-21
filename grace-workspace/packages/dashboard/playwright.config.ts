import { defineConfig } from "@playwright/test";

export const DASHBOARD_PORT = 31410;
export const FAKE_WS_PORT = 39142;

/**
 * The Vite dev server gets `VITE_WS_PORT=39142` and inlines that into
 * the bundle, so the dashboard opens its WS to ws://localhost:39142.
 * The fake supervisor in each test listens on that port. The two ports
 * are deliberately high/uncommon so they don't clash with the real
 * Byne supervisor (3142) or dev dashboard (3141) running on the host.
 */
export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  expect: { timeout: 5_000 },
  fullyParallel: false,
  workers: 1,
  reporter: process.env.CI ? "dot" : "list",
  use: {
    baseURL: `http://127.0.0.1:${DASHBOARD_PORT}`,
    trace: "on-first-retry",
    actionTimeout: 5_000,
  },
  webServer: {
    command: `pnpm vite --host 127.0.0.1 --port ${DASHBOARD_PORT} --strictPort`,
    url: `http://127.0.0.1:${DASHBOARD_PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
    env: { VITE_WS_PORT: String(FAKE_WS_PORT) },
    stdout: "ignore",
    stderr: "pipe",
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
