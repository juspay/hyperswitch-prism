import { test as base, type TestType } from "@playwright/test";
import { Checks } from "./checks.js";
import { Hyperswitch } from "./hyperswitch.js";
import { setupMerchant, type TestMerchant } from "./merchant-setup.js";
import { PaymentOperations } from "./operations.js";
import type {
  ConnectorDefinition,
  Scenario,
  ScenarioOverrides,
} from "./scenario.js";

type ScenarioFixtures = {
  connector: ConnectorDefinition;
  scenario: Scenario;
  merchant: Pick<
    TestMerchant,
    "merchantId" | "profileId" | "merchantConnectorId"
  >;
  payments: PaymentOperations;
  checks: Pick<Checks, "fields" | "oneOf">;
};

type InternalFixtures = {
  _hs: Hyperswitch;
  _merchant: TestMerchant;
  _overrides: ScenarioOverrides;
  _checks: Checks;
};

// Setup fixtures have no payment journey. Raw HTTP and API keys remain internal.
const fixtures = base.extend<ScenarioFixtures & InternalFixtures>({
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
  _overrides: [
    async ({ connector, scenario }, use, info) => {
      const exclusion = connector.exclude?.[scenario.id];
      if (exclusion && !exclusion.reason.trim()) {
        throw new Error("Scenario exclusions require a reason");
      }
      base.skip(!!exclusion, exclusion?.reason);
      info.annotations.push({
        type: "Scenario",
        description: scenario.description,
      });
      const overrides = connector.overrides?.[scenario.id] ?? {};
      if (Object.keys(overrides.requests ?? {}).length) {
        info.annotations.push({
          type: "Connector request overrides",
          description: Object.keys(overrides.requests!).join(", "),
        });
      }
      await use(overrides);
    },
    { auto: true },
  ],
  _hs: async ({}, use) => {
    const hs = await Hyperswitch.create();
    try {
      await use(hs);
    } finally {
      await hs.dispose();
    }
  },
  _merchant: async ({ _hs, connector }, use) => {
    await use(await setupMerchant(_hs, connector));
  },
  merchant: async ({ _merchant }, use) => {
    await use({
      merchantId: _merchant.merchantId,
      profileId: _merchant.profileId,
      merchantConnectorId: _merchant.merchantConnectorId,
    });
  },
  _checks: async ({}, use, info) => {
    await use(new Checks(info));
  },
  checks: async ({ _checks }, use) => {
    await use({
      fields: _checks.fields.bind(_checks),
      oneOf: _checks.oneOf.bind(_checks),
    });
  },
  payments: async ({ _hs, _merchant, connector, _checks, _overrides }, use) => {
    await use(
      new PaymentOperations(_hs, _merchant, connector, _checks, _overrides),
    );
  },
});

// Publish only the supported scenario API. This assignment is checked by TypeScript.
export const test: TestType<ScenarioFixtures, {}> = fixtures;
