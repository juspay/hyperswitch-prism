import { test as base } from "@playwright/test";
import { Checks } from "./checks.js";
import { Hyperswitch } from "./hyperswitch.js";
import { setupMerchant, type TestMerchant } from "./merchant-setup.js";
import { PaymentOperations } from "./operations.js";
import type {
  ConnectorDefinition,
  Scenario,
  ScenarioOverrides,
} from "./scenario.js";

// Standard Playwright fixtures own setup and cleanup; specs own payment calls.
export const test = base.extend<{
  connector: ConnectorDefinition;
  scenario: Scenario;
  hs: Hyperswitch;
  merchant: TestMerchant;
  payments: PaymentOperations;
  checks: Checks;
  overrides: ScenarioOverrides;
}>({
  connector: [
    async () => {
      throw new Error("Configure the connector with test.use({ connector })");
    },
    { option: true },
  ],
  scenario: [
    async () => {
      throw new Error(
        "Configure scenario metadata with test.use({ scenario })",
      );
    },
    { option: true },
  ],
  overrides: [
    async ({ connector, scenario }, use, info) => {
      const exclusion = connector.exclude?.[scenario.id];
      if (exclusion && !exclusion.reason.trim()) {
        throw new Error("Scenario exclusions require a reason");
      }
      test.skip(!!exclusion, exclusion?.reason);
      info.annotations.push({
        type: "Scenario",
        description: scenario.description,
      });
      await use(connector.overrides?.[scenario.id] ?? {});
    },
    { auto: true },
  ],
  hs: async ({}, use) => {
    const hs = await Hyperswitch.create();
    try {
      await use(hs);
    } finally {
      await hs.dispose();
    }
  },
  merchant: async ({ hs, connector }, use) => {
    await use(await setupMerchant(hs, connector));
  },
  checks: async ({}, use, info) => {
    await use(new Checks(info));
  },
  payments: async ({ hs, merchant, connector, checks }, use) => {
    await use(new PaymentOperations(hs, merchant, connector, checks));
  },
});
