import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
    // Spawn-real-binary tests need a bit more breathing room than 5s.
    testTimeout: 15_000,
    hookTimeout: 15_000,
    pool: "forks",
    poolOptions: {
      forks: {
        singleFork: false,
      },
    },
  },
});
