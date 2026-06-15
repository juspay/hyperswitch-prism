/** @type {import('ts-jest').JestConfigWithTsJest} */
module.exports = {
  preset: "ts-jest",
  testEnvironment: "node",
  roots: ["<rootDir>/medusa-unified-payment/src", "<rootDir>/__tests__"],
  testMatch: [
    "**/__tests__/**/*.test.ts",
    "**/?(*.)+(spec|test).ts",
  ],
  transform: {
    "^.+\\.ts$": [
      "ts-jest",
      {
        tsconfig: {
          isolatedModules: true,
        },
      },
    ],
  },
  moduleFileExtensions: ["ts", "js", "json"],
  collectCoverageFrom: [
    "medusa-unified-payment/src/**/*.ts",
    "!medusa-unified-payment/src/**/*.d.ts",
    "!__tests__/**",
  ],
  coverageThreshold: {
    global: {
      branches: 80,
      functions: 80,
      lines: 80,
      statements: 80,
    },
  },
  testTimeout: 30000,
}
